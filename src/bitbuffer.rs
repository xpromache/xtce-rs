/// Allows to read and write bits from a byte array (byte[]) keeps a bit position and the extractions are relative to the
/// position. It allows also to provide an offset (in bytes) inside the byte array and then the bit position is relative
/// to the offset.
/// <p>
/// Supported operations are
/// <ul>
/// <li>extract up to 64 bits into a long
/// <li>big endian or little endian
/// <li>extract a byte array (throws exception if the position is not at the beginning of a byte)
/// <li>extract a byte (throws exception if the position is not at the beginning of a byte)
/// </ul>
///
/// Note on the Little Endian: it is designed to work on x86 architecture which uses internally little endian byte _and_
/// bit ordering but when accessing memory, full bytes are transferred in big endian order.
/// <p>
/// For example when in C you have a 32 bit structure:
///
/// <pre>
/// struct S {
///    unsigned int a: 3;
///    unsigned int b: 12;
///    unsigned int c: 17;
/// }
/// </pre>
///
/// and you pack that in a packet by just reading the corresponding 4 bytes memory, you will get the following
/// representation (0 is the most significant bit):
///
/// <pre>
/// b7  b8 b9  b10 b11 a0  a1  a2
/// c16 b0 b1  b2  b3  b4  b5  b6
/// c8  c9 c10 c11 c12 c13 c14 c15
/// c0 c1  c2  c3  c4  c5  c6  c7
/// </pre>
///
/// To read this with this BitBuffer you would naturally do like this:
///
/// <pre>
/// BitBuffer bb = new BitBuffer(..., 0);
/// bb.setOrder(LITTLE_ENDIAN);
///
/// a = bb.getBits(3);
/// b = bb.getBits(12);
/// c = bb.getBits(17);
/// </pre>
///
/// Note how the first call (when the bb.position=0) reads the 3 bits at position 5 instead of those at position 0

pub struct BitBuffer<'a> {
    b: &'a [u8],
    position: usize,
    byte_order: ByteOrder,
}

impl BitBuffer<'_> {
    pub fn wrap<'a>(b: &'a [u8]) -> BitBuffer<'a> {
        BitBuffer { b, position: 0, byte_order: ByteOrder::BigEndian }
    }

    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    pub fn set_byte_order(&mut self, byte_order: ByteOrder) {
        self.byte_order = byte_order;
    }

    pub fn get_position(&self) -> usize {
        self.position
    }

    /// total size in bits
    pub fn bitsize(&self) -> usize {
        self.b.len() * 8
    }

    pub fn slice<'a>(&'a self) -> BitBuffer<'a> {
        if (self.position & 0x7) != 0 {
            panic!("Can only slice at byte boundaries")
        }
        let pos = self.position / 8;

        BitBuffer { b: &self.b[pos..], position: 0, byte_order: self.byte_order }
    }
    /**
     * reads numBits from the buffer and returns them into a long on the rightmost position.
     *
     * @param numBits
     *            has to be max 64.
     */
    pub fn get_bits(&mut self, num_bits: usize) -> u64 {
        if num_bits > 64 {
            panic!("Invalid numBits {}, max value: 64", num_bits);
        }

        if self.byte_order == ByteOrder::LittleEndian {
            return self.get_bits_le(num_bits);
        }
        let mut r: u64 = 0;
        let mut pos = self.position;

        let mut byte_pos = pos / 8;
        let mut n = num_bits;
        let fbb = (-(pos as i32) & 0x7) as usize; // how many bits are from position until the end of the byte

        if fbb > 0 {
            if n <= fbb {
                // the value fits entirely within the first byte
                pos += num_bits;
                self.position = pos;
                return ((self.b[byte_pos] >> (fbb - n)) & ((1 << n) - 1)) as u64;
            } else {
                r = (self.b[byte_pos] & ((1 << fbb) - 1)) as u64;
                n -= fbb;
                byte_pos += 1;
            }
        }

        while n > 8 {
            r = (r << 8) | (self.b[byte_pos] as u64);
            n -= 8;
            byte_pos += 1;
        }
        r = (r << n) | ((self.b[byte_pos] >> (8 - n)) as u64);

        pos += num_bits;

        self.position = pos;

        r
    }
    fn get_bits_le(&mut self, num_bits: usize) -> u64 {
        let mut r: u64 = 0;
        let mut pos = self.position;

        let mut byte_pos = (pos + num_bits - 1) / 8;
        let mut n = num_bits;
        let lbb = (pos + num_bits) & 0x7; // how many bits are to be read from the last byte (which is the most
                                          // significant)
        if lbb > 0 {
            if lbb >= n {
                // the value fits entirely within one byte
                pos += num_bits;
                self.position = pos;
                return ((self.b[byte_pos] >> (lbb - n)) & ((1 << n) - 1)) as u64;
            } else {
                r = (self.b[byte_pos] & ((1 << lbb) - 1)) as u64;
                n -= lbb;
                byte_pos -= 1;
            }
        }
        while n > 8 {
            r = (r << 8) | (self.b[byte_pos] as u64);
            n -= 8;
            byte_pos -= 1;
        }

        r = (r << n) | ((self.b[byte_pos] >> (8 - n)) as u64);

        pos += num_bits;
        self.position = pos;

        r
    }

    pub fn get_byte(&mut self) -> u8 {
        self.ensure_byte_boundary();
        let r = self.b[self.position / 8];
        self.position += 8;
        r
    }

    /// copy from the buffer into slice b
    /// panics if there is not enough data in the buffer
    pub fn get_bytes(&mut self, b: &mut [u8]) {
        self.ensure_byte_boundary();
        let pos = self.position;

        b.clone_from_slice(&self.b[pos..pos + b.len()]);
        self.position = pos + b.len();
    }

    /// gets a reference to a slice inside the buffer
    /// advances the position
    /// panics if there is not enough data in the buffer
    pub fn get_bytes_ref(&mut self, len: usize) -> &[u8] {
        let pos = self.position / 8;
        self.position = self.position + 8 * len;
        &self.b[pos..pos + len]
    }

    pub fn remaining_bytes(&self) -> usize {
        self.ensure_byte_boundary();

        return self.b.len() - (self.position >> 3);
    }

    fn ensure_byte_boundary(&self) {
        if self.position & 0x7 != 0 {
            panic!("bit position not at byte boundary");
        }
    }
}

pub struct BitWriter<'a> {
    b: &'a mut [u8],
    position: usize,
    byte_order: ByteOrder,
}

#[derive(PartialEq, Debug)]
pub enum WriteError {
    BufferTooSmall,
    UnalignedWrite
}

impl<'a> BitWriter<'a> {
    pub fn wrap(b: &'a mut [u8]) -> BitWriter<'a> {
        Self { b: b, position: 0, byte_order: ByteOrder::BigEndian }
    }

    pub fn check_bounds(&self, next_position: usize) -> Result<(), WriteError> {
        if next_position.div_ceil(8) > self.b.len() {
            Err(WriteError::BufferTooSmall)
        } else {
            Ok(())
        }
    }

    pub fn write_bytes(&mut self, b: &[u8]) -> Result<(), WriteError> {
        if self.position & 0x7 != 0 {
            return Err(WriteError::UnalignedWrite);
        }

        self.check_bounds(self.position + b.len() * 8)?;

        self.b[self.position / 8..self.position / 8 + b.len()].clone_from_slice(b);
        self.position += 8 * b.len();
        Ok(())
    }

    pub fn skip_to_byte_boundary(&mut self) -> Result<(), WriteError> {
        // move to the next byte boundary
        let next_position = (self.position + 7) & !0x7;
        self.check_bounds(next_position)?;

        self.position = next_position;
        Ok(())

    }

    pub fn write_bits(&mut self, value: u64, num_bits: usize) -> Result<(), WriteError> {
        if self.byte_order == ByteOrder::LittleEndian {
            todo!(); // is this actually needed?
        }

        let mut pos = self.position;
        let mut byte_pos = pos / 8;
        let mut n = num_bits;
        let mut v = value;

        self.check_bounds(pos + n)?;

        let fbb = (-(pos as i32) & 0x7) as usize; // how many bits are from position until the end of the byte
        if fbb > 0 {
            if n <= fbb {
                // the value fits entirely within the first byte
                let mask: u8 = (1 << n) - 1;
                self.b[byte_pos] &= !(mask << (fbb - n)); // clear existing bits that may be set
                self.b[byte_pos] |= ((v as u8) & mask) << (fbb - n); // set requested bits
                pos += num_bits;
                self.position = pos;
                return Ok(());
            } else {
                let mask = (1 << fbb) - 1;
                self.b[byte_pos] &= !mask; // clear existing bits that may be set
                self.b[byte_pos] |= (v >> (n - fbb)) as u8 & mask; // set requested bits
                n -= fbb;
                byte_pos += 1;
                v &= (1 << n) - 1;
            }
        }

        while n >= 8 {
            n -= 8;
            self.b[byte_pos] = (v >> n) as u8;
            byte_pos += 1;
            v &= (1 << n) - 1;
        }

        if n > 0 {
            let mask = (1 << n) - 1;
            self.b[byte_pos] &= !(mask << (8 - n));
            self.b[byte_pos] |= (v as u8) << (8 - n);
        }

        pos += num_bits;
        self.position = pos;
        return Ok(())
    }

    pub fn set_byte_order(&mut self, byte_order: ByteOrder) {
        self.byte_order = byte_order;
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ByteOrder {
    BigEndian,
    LittleEndian,
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use rand::{rngs::SmallRng, RngCore, SeedableRng};

    use super::*;

    #[test]
    fn test_write_bigendian() {
        let mut b: [u8; 4] = [0, 0, 0, 0];
        let mut w = BitWriter::wrap(&mut b);

        assert!(w.write_bits(5, 3).is_ok());
        assert!(w.write_bits(0xabcdef, 24).is_ok());

        assert_eq!([0xb5, 0x79, 0xbd, 0xe0], b);

        let mut bb = BitBuffer::wrap(&b);

        assert_eq!(5, bb.get_bits(3));
        assert_eq!(0xabcdef, bb.get_bits(24));
    }

    #[test]
    fn test_write_too_big() {
        let mut b: [u8; 4] = [0, 0, 0, 0];
        let mut w = BitWriter::wrap(&mut b);
        assert!(w.write_bits(0x123456789, 37).is_err());
        assert_eq!(b, [0, 0, 0, 0]);
    }

    #[test]
    fn test_write_bytes() {
        let mut b: [u8; 4] = [0, 0, 0, 0];
        let mut w = BitWriter::wrap(&mut b);

        assert!(w.write_bits(0xab, 8).is_ok());
        assert!(w.write_bytes(&[0x12, 0x34, 0x56]).is_ok());
        assert_eq!(w.position, 4 * 8);

        assert!(w.write_bytes(&[0xff]).is_err());
        assert_eq!(b, [0xab, 0x12, 0x34, 0x56]);
    }

    #[test]
    fn test_unaligned_write() {
        let mut b: [u8; 4] = [0, 0, 0, 0];
        let mut w = BitWriter::wrap(&mut b);
        assert!(w.write_bits(0x1234, 12).is_ok());
        assert_eq!(w.write_bytes(&[0x12, 0x34, 0x56]).unwrap_err(), WriteError::UnalignedWrite);
    }

    #[test]
    fn test_skip_to_byte_boundary() {
        let mut b: [u8; 4] = [0, 0, 0, 0];
        let mut w = BitWriter::wrap(&mut b);
        assert!(w.write_bits(0x0b, 4).is_ok());
        assert_eq!(w.position, 4);
        assert!(w.skip_to_byte_boundary().is_ok());
        assert_eq!(w.position, 8);

        // repeated call should not change the position
        assert!(w.skip_to_byte_boundary().is_ok());
        assert_eq!(w.position, 8);

        assert_eq!(b, [0x0b << 4, 0x00, 0x00, 0x00]);

    }


    #[test]
    fn test_bigendian() {
        let b = vec![0x18, 0x7A, 0x23, 0xFF];
        let mut bitbuf = BitBuffer::wrap(&b);

        bitbuf.set_position(0);
        assert_eq!(0x18, bitbuf.get_bits(8));

        bitbuf.set_position(4);
        assert_eq!(0x87, bitbuf.get_bits(8));

        bitbuf.set_position(0);
        assert_eq!(0x187A, bitbuf.get_bits(16));

        bitbuf.set_position(4);
        assert_eq!(0x87A, bitbuf.get_bits(12));

        bitbuf.set_position(4);
        assert_eq!(0x87A2, bitbuf.get_bits(16));

        bitbuf.set_position(4);
        assert_eq!(0x87A23, bitbuf.get_bits(20));

        bitbuf.set_position(0);

        assert_eq!(0x187A23FF, bitbuf.get_bits(32));
    }
    #[test]
    fn test_bytes() {
        let b = vec![0x18, 0x7A, 0x23, 0xFF];
        let mut bitbuf = BitBuffer::wrap(&b);

        assert_eq!(0x18, bitbuf.get_byte());
        assert_eq!([0x7A, 0x23, 0xFF], bitbuf.get_bytes_ref(3));
    }

    #[test]
    fn test_little_endian() {
        let b = vec![0x18, 0x7A, 0x23, 0xFF];
        let mut bitbuf = BitBuffer::wrap(&b);
        bitbuf.set_byte_order(ByteOrder::LittleEndian);

        assert_eq!(0, bitbuf.get_bits(1));
        assert_eq!(1, bitbuf.get_position());

        assert_eq!(4, bitbuf.get_bits(3));
        assert_eq!(4, bitbuf.get_position());

        bitbuf.set_position(0);
        assert_eq!(0x18, bitbuf.get_bits(8));

        bitbuf.set_position(4);
        assert_eq!(0xA1, bitbuf.get_bits(8));

        bitbuf.set_position(0);
        assert_eq!(0x7A18, bitbuf.get_bits(16));

        bitbuf.set_position(4);
        assert_eq!(0x7A1, bitbuf.get_bits(12));

        bitbuf.set_position(4);
        assert_eq!(0x37A1, bitbuf.get_bits(16));

        bitbuf.set_position(4);
        assert_eq!(0x237A1, bitbuf.get_bits(20));

        bitbuf.set_position(0);

        assert_eq!(0xFF237A18, bitbuf.get_bits(32));
    }

    #[test]
    fn test_little_endian_read1() {
        let b = vec![0x03, 0x80, 0xFF, 0xFF];
        let mut bitbuf = BitBuffer::wrap(&b);
        bitbuf.set_byte_order(ByteOrder::LittleEndian);

        assert_eq!(3, bitbuf.get_bits(3));
        assert_eq!(0, bitbuf.get_bits(12));
        assert_eq!(0x1FFFF, bitbuf.get_bits(17));
    }

    #[test]
    fn test_double_slice() {
        let b = vec![0x01, 0x02, 0x03, 0x04];
        let mut bitbuf = BitBuffer::wrap(&b);
        assert_eq!(0x01, bitbuf.get_bits(8));
        assert_eq!(8, bitbuf.get_position());

        let mut bitbuf1 = bitbuf.slice();
        assert_eq!(0x02, bitbuf1.get_bits(8));

        let mut bitbuf2 = bitbuf1.slice();
        assert_eq!(0x03, bitbuf2.get_bits(8));

        assert_eq!(0x02, bitbuf.get_bits(8));
        assert_eq!(16, bitbuf.get_position());
    }

    // in big endian it runs in java in 10.6 seconds and in Rust release mode in about 6.6 seconds
    // in little endian it runs in java in 10.1 seconds and in Rust in 7.1 seconds
    //#[test]
    fn _test_speed() {
        const N: usize = 1000_000;
        let mut b = [0u8; N];
        let mut s = 0;
        let mut r = SmallRng::from_entropy();

        let t0 = Instant::now();

        let mut c = 0;

        for _ in 0..3000 {
            let idx = 3; //r.next_u32() as usize % N;
            b[idx] = r.next_u32() as u8;
            let mut bitbuf = BitBuffer::wrap(&b);
            // bitbuf.set_byte_order(ByteOrder::LittleEndian);

            'hopa: loop {
                for j in 1..33 {
                    if bitbuf.get_position() + 64 > N * 8 {
                        break 'hopa;
                    }
                    c += 1;

                    s += bitbuf.get_bits(j);
                }
            }
        }

        println!("s: {}, t1-t0: {} millis c: {}", s, t0.elapsed().as_millis(), c);
    }
}
