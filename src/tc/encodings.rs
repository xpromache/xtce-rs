//! Encoding of raw values into binary, the counterpart of [crate::proc::encodings].

use crate::{
    bitbuffer::{BitBuffer, BitWriter, ByteOrder},
    mdb::types::{
        BinaryDataEncoding, BinarySize, BooleanDataEncoding, DataEncoding, FloatDataEncoding,
        FloatEncodingType, IntegerDataEncoding, IntegerEncodingType, StringBoxSize,
        StringDataEncoding, StringSize,
    },
    value::Value,
};

use super::{Result, TcError};

fn err<T>(msg: String) -> Result<T> {
    Err(TcError::EncodingError(msg))
}

/// writes the raw value using the given encoding
pub(crate) fn encode_raw(enc: &DataEncoding, raw: &Value, w: &mut BitWriter) -> Result<()> {
    match enc {
        DataEncoding::Integer(ide) => encode_integer(ide, raw, w),
        DataEncoding::Float(fde) => encode_float(fde, raw, w),
        DataEncoding::Boolean(bde) => encode_boolean(bde, raw, w),
        DataEncoding::String(sde) => match raw {
            Value::StringValue(s) => encode_string(sde, s, w),
            _ => err(format!("cannot encode {:?} as string", raw)),
        },
        DataEncoding::Binary(bde) => match raw {
            Value::Binary(b) => encode_binary(bde, b, w),
            _ => err(format!("cannot encode {:?} as binary", raw)),
        },
        DataEncoding::None => err("data type without encoding".to_owned()),
    }
}

/// returns the integer value of the raw value (integral doubles are accepted)
fn integer_of(raw: &Value) -> Result<i128> {
    match raw {
        Value::Int64(v) => Ok(*v as i128),
        Value::Uint64(v) => Ok(*v as i128),
        Value::Boolean(b) => Ok(*b as i128),
        Value::Double(d) if d.fract() == 0.0 && d.abs() < 1.9e19 => Ok(*d as i128),
        _ => err(format!("cannot encode {:?} as integer", raw)),
    }
}

fn encode_integer(ide: &IntegerDataEncoding, raw: &Value, w: &mut BitWriter) -> Result<()> {
    let n = ide.size_in_bits as usize;
    let bits = integer_bits(ide.encoding, n, integer_of(raw)?)?;
    w.set_byte_order(ide.byte_order);
    w.put_bits(bits, n)?;
    Ok(())
}

/// computes the bits representing v in the given encoding; returns an error if v does not fit in numbits
pub(crate) fn integer_bits(encoding: IntegerEncodingType, numbits: usize, v: i128) -> Result<u64> {
    if numbits == 0 || numbits > 64 {
        return err(format!("invalid integer size in bits {}", numbits));
    }
    let n = numbits as u32;
    let mask: u64 = u64::MAX >> (64 - n);
    let (min, max): (i128, i128) = match encoding {
        IntegerEncodingType::Unsigned => (0, (1i128 << n) - 1),
        IntegerEncodingType::TwosComplement => (-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1),
        IntegerEncodingType::SignMagnitude | IntegerEncodingType::OnesComplement => {
            (-((1i128 << (n - 1)) - 1), (1i128 << (n - 1)) - 1)
        }
    };
    if v < min || v > max {
        return err(format!(
            "value {} does not fit in {} bits {:?} encoding (range [{}, {}])",
            v, numbits, encoding, min, max
        ));
    }
    Ok(match encoding {
        IntegerEncodingType::Unsigned => v as u64,
        IntegerEncodingType::TwosComplement => (v as i64 as u64) & mask,
        IntegerEncodingType::SignMagnitude => {
            if v < 0 {
                (1u64 << (n - 1)) | (-v) as u64
            } else {
                v as u64
            }
        }
        IntegerEncodingType::OnesComplement => {
            if v < 0 {
                !((-v) as u64) & mask
            } else {
                v as u64
            }
        }
    })
}

fn encode_float(fde: &FloatDataEncoding, raw: &Value, w: &mut BitWriter) -> Result<()> {
    let x = match raw {
        Value::Double(d) => *d,
        Value::Int64(v) => *v as f64,
        Value::Uint64(v) => *v as f64,
        _ => return err(format!("cannot encode {:?} as float", raw)),
    };
    let n = fde.size_in_bits as usize;
    let bits = match (fde.encoding, n) {
        (FloatEncodingType::IEEE754_1985, 16) => f64_to_half(x) as u64,
        (FloatEncodingType::IEEE754_1985, 32) => (x as f32).to_bits() as u64,
        (FloatEncodingType::IEEE754_1985, 64) => x.to_bits(),
        (FloatEncodingType::Milstd1750a, 32) => f64_to_milstd1750a32(x)? as u64,
        (FloatEncodingType::Milstd1750a, 48) => f64_to_milstd1750a48(x)?,
        _ => {
            return err(format!("unsupported float encoding {:?} with size in bits {}", fde.encoding, n))
        }
    };
    w.set_byte_order(fde.byte_order);
    w.put_bits(bits, n)?;
    Ok(())
}

/// converts to IEEE 754 half precision (rounding to nearest even through single precision)
pub(crate) fn f64_to_half(x: f64) -> u16 {
    let bits = (x as f32).to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mant = bits & 0x7F_FFFF;
    if exp == 0xFF {
        return sign | 0x7C00 | if mant != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1F {
        return sign | 0x7C00;
    }
    if e <= 0 {
        // subnormal half
        if e < -10 {
            return sign;
        }
        let m = mant | 0x80_0000;
        let shift = (14 - e) as u32;
        let hm = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        let r = if rem > halfway || (rem == halfway && hm & 1 == 1) { hm + 1 } else { hm };
        return sign | r as u16;
    }
    let hm = mant >> 13;
    let rem = mant & 0x1FFF;
    let mut r = ((e as u32) << 10) | hm;
    if rem > 0x1000 || (rem == 0x1000 && r & 1 == 1) {
        // the carry may propagate into the exponent which gives the correct result (including infinity)
        r += 1;
    }
    sign | r as u16
}

/// splits x into mantissa f and exponent e such that x = f * 2^e and 0.5 <= |f| < 1
fn frexp(x: f64) -> (f64, i32) {
    let mut e = x.abs().log2().floor() as i32 + 1;
    let mut f = x / 2f64.powi(e);
    while f.abs() >= 1.0 {
        f /= 2.0;
        e += 1;
    }
    while f.abs() < 0.5 {
        f *= 2.0;
        e -= 1;
    }
    (f, e)
}

/// computes the (mantissa, exponent) of the MIL-STD-1750A representation with a mantissa of mbits bits
fn milstd1750a(x: f64, mbits: i32) -> Result<(i64, i32)> {
    if !x.is_finite() {
        return err(format!("cannot encode {} as MIL-STD-1750A", x));
    }
    if x == 0.0 {
        return Ok((0, 0));
    }
    let (f, mut e) = frexp(x);
    let scale = 2f64.powi(mbits - 1);
    let mut m = (f * scale).round() as i64;
    if m == scale as i64 {
        // rounding overflow
        m /= 2;
        e += 1;
    }
    if e > 127 {
        return err(format!("value {} too large for MIL-STD-1750A", x));
    }
    if e < -128 {
        // underflow
        return Ok((0, 0));
    }
    Ok((m, e))
}

pub(crate) fn f64_to_milstd1750a32(x: f64) -> Result<u32> {
    let (m, e) = milstd1750a(x, 24)?;
    Ok((((m as u32) & 0xFF_FFFF) << 8) | (e as i8 as u8 as u32))
}

pub(crate) fn f64_to_milstd1750a48(x: f64) -> Result<u64> {
    let (m, e) = milstd1750a(x, 40)?;
    let m = (m as u64) & 0xFF_FFFF_FFFF;
    Ok(((m >> 16) << 24) | ((e as i8 as u8 as u64) << 16) | (m & 0xFFFF))
}

fn encode_boolean(bde: &BooleanDataEncoding, raw: &Value, w: &mut BitWriter) -> Result<()> {
    let n = match bde.size_in_bits {
        BinarySize::Fixed(n) if n > 0 && n <= 64 => n as usize,
        _ => return err("only fixed size (1-64 bits) boolean encoding is supported".to_owned()),
    };
    let v = match raw {
        Value::Boolean(b) => *b as u64,
        _ => return err(format!("cannot encode {:?} as boolean", raw)),
    };
    w.set_byte_order(ByteOrder::BigEndian);
    w.put_bits(v, n)?;
    Ok(())
}

/// encodes the string into bytes using the given character set (like Java, the unmappable characters are replaced by '?').
/// Returns None if the character set is not supported.
pub(crate) fn encode_charset(charset: &str, s: &str) -> Option<Vec<u8>> {
    let utf16 = |be: bool| -> Vec<u8> {
        s.encode_utf16().flat_map(|u| if be { u.to_be_bytes() } else { u.to_le_bytes() }).collect()
    };
    Some(match charset.to_ascii_uppercase().as_str() {
        "UTF-8" | "UTF8" => s.as_bytes().to_vec(),
        "US-ASCII" | "ASCII" => s.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).collect(),
        "ISO-8859-1" | "LATIN1" | "ISO_8859_1" => {
            s.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
        }
        // like Java, UTF-16 is written big endian with a byte order mark
        "UTF-16" => {
            let mut v = vec![0xFE, 0xFF];
            v.extend(utf16(true));
            v
        }
        "UTF-16BE" => utf16(true),
        "UTF-16LE" => utf16(false),
        _ => return None,
    })
}

fn encode_string(sde: &StringDataEncoding, s: &str, w: &mut BitWriter) -> Result<()> {
    let data = encode_charset(&sde.encoding, s)
        .ok_or_else(|| TcError::EncodingError(format!("unsupported string encoding '{}'", sde.encoding)))?;
    let start = w.get_position();
    match sde.size_in_bits {
        StringSize::Fixed(bits) => {
            let size = (bits / 8) as usize;
            if data.len() > size {
                return err(format!("string of {} bytes does not fit in the fixed size of {} bytes", data.len(), size));
            }
            w.put_bytes(&data)?;
            w.put_bytes(&vec![0; size - data.len()])?;
        }
        StringSize::TerminationChar(c) => {
            w.put_bytes(&data)?;
            w.put_bytes(&[c])?;
        }
        StringSize::LeadingSize(tag_bytes) => {
            let tag_bits = (tag_bytes * 8) as usize;
            if tag_bits < 64 && data.len() as u64 >= (1u64 << tag_bits) {
                return err(format!("string of {} bytes too long for a size tag of {} bits", data.len(), tag_bits));
            }
            w.set_byte_order(ByteOrder::BigEndian);
            w.put_bits(data.len() as u64, tag_bits)?;
            w.put_bytes(&data)?;
        }
        StringSize::Custom => return err("custom string size algorithms are not supported".to_owned()),
    }
    let written = (w.get_position() - start) / 8;
    if let Some(max) = sde.max_box_size_in_bytes {
        if written > max as usize {
            return err(format!("encoded string size {} bytes exceeds the maximum {} bytes", written, max));
        }
    }
    if let StringBoxSize::Fixed(bits) = sde.box_size_in_bits {
        let box_size = (bits / 8) as usize;
        if written > box_size {
            return err(format!("encoded string size {} bytes exceeds the box size {} bytes", written, box_size));
        }
        w.put_bytes(&vec![0; box_size - written])?;
    }
    Ok(())
}

fn encode_binary(bde: &BinaryDataEncoding, data: &[u8], w: &mut BitWriter) -> Result<()> {
    w.set_byte_order(ByteOrder::BigEndian);
    match &bde.size_in_bits {
        BinarySize::Fixed(bits) => {
            let bits = *bits as usize;
            let nbytes = bits.div_ceil(8);
            if data.len() > nbytes {
                return err(format!("binary value of {} bytes longer than the fixed size of {} bits", data.len(), bits));
            }
            // like the decoder, if the size is not a multiple of 8, the last byte contains the remaining bits right aligned
            let mut n = bits;
            for i in 0..nbytes {
                let k = n.min(8);
                let b = data.get(i).copied().unwrap_or(0) as u64;
                w.put_bits(b & ((1 << k) - 1), k)?;
                n -= k;
            }
        }
        BinarySize::LeadingSize(tag_bits) => {
            let tag_bits = *tag_bits as usize;
            if data.len() as u64 >= (1u64 << tag_bits) {
                return err(format!("binary value of {} bytes too long for a size tag of {} bits", data.len(), tag_bits));
            }
            w.put_bits(data.len() as u64, tag_bits)?;
            w.put_bytes(data)?;
        }
        // the size is given by another value; the data is written entirely
        BinarySize::Dynamic(_) => w.put_bytes(data)?,
        BinarySize::Custom(algo) => {
            return err(format!("custom binary encoding algorithm '{}' is not supported", algo))
        }
    }
    Ok(())
}

/// writes the size_in_bits rightmost bits of value (padded on the left with zeros if needed)
pub(crate) fn encode_fixed_value(value: &[u8], size_in_bits: usize, w: &mut BitWriter) -> Result<()> {
    w.set_byte_order(ByteOrder::BigEndian);
    let total = value.len() * 8;
    let mut n = size_in_bits;
    if n > total {
        let mut pad = n - total;
        while pad > 0 {
            let k = pad.min(64);
            w.put_bits(0, k)?;
            pad -= k;
        }
        n = total;
    }
    let mut bb = BitBuffer::wrap(value);
    bb.set_position(total - n);
    while n > 0 {
        let k = n.min(64);
        w.put_bits(bb.get_bits(k)?, k)?;
        n -= k;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proc::encodings::{decode_integer, half_to_f64, milstd1750a32_to_f64, milstd1750a48_to_f64};

    #[test]
    fn test_integer_bits() {
        use IntegerEncodingType::*;
        for enc in [Unsigned, TwosComplement, SignMagnitude, OnesComplement] {
            for n in [1usize, 3, 8, 16, 33, 64] {
                for v in [-200i128, -5, -1, 0, 1, 5, 127, 200, i64::MAX as i128, u64::MAX as i128] {
                    if let Ok(bits) = integer_bits(enc, n, v) {
                        let dv = match decode_integer(enc, n, bits) {
                            Value::Int64(x) => x as i128,
                            Value::Uint64(x) => x as i128,
                            _ => unreachable!(),
                        };
                        assert_eq!(v, dv, "{:?} {} bits", enc, n);
                    }
                }
            }
        }
        assert!(integer_bits(Unsigned, 8, 256).is_err());
        assert!(integer_bits(Unsigned, 8, -1).is_err());
        assert!(integer_bits(TwosComplement, 8, -129).is_err());
        assert!(integer_bits(SignMagnitude, 8, -128).is_err());
        assert_eq!(0x85, integer_bits(SignMagnitude, 8, -5).unwrap());
        assert_eq!(0xFA, integer_bits(OnesComplement, 8, -5).unwrap());
    }

    #[test]
    fn test_floats() {
        for x in [0.0, 1.0, -2.0, 65504.0, 0.333, 2f64.powi(-24), -1e-3] {
            let h = f64_to_half(x);
            assert!((half_to_f64(h) - x).abs() <= x.abs() * 1e-3 + 1e-7, "{} -> {}", x, half_to_f64(h));
        }
        assert_eq!(0x3C00, f64_to_half(1.0));
        assert_eq!(0x7C00, f64_to_half(1e6));
        for x in [0.5, -1.0, 0.75 * 2f64.powi(-128), -0.75 * 2f64.powi(4), 2.71, -1234.5678, 1e30] {
            let y = milstd1750a32_to_f64(f64_to_milstd1750a32(x).unwrap());
            assert!((x - y).abs() <= x.abs() * 2f64.powi(-22), "{} -> {}", x, y);
            let y = milstd1750a48_to_f64(f64_to_milstd1750a48(x).unwrap());
            assert!((x - y).abs() <= x.abs() * 2f64.powi(-38), "{} -> {}", x, y);
        }
        assert_eq!(0x40000000, f64_to_milstd1750a32(0.5).unwrap());
        assert_eq!(0x7FFFFF7F, f64_to_milstd1750a32((1.0 - 2f64.powi(-23)) * 2f64.powi(127)).unwrap());
        assert!(f64_to_milstd1750a32(1e40).is_err());
    }

    #[test]
    fn test_fixed_value() {
        let mut w = BitWriter::new(10);
        encode_fixed_value(&[0x03], 2, &mut w).unwrap();
        encode_fixed_value(&[0xAB, 0xCD], 12, &mut w).unwrap();
        encode_fixed_value(&[0x01], 10, &mut w).unwrap();
        // 11 1011 1100 1101 00 0000 0001
        assert_eq!(vec![0xEF, 0x34, 0x01], w.into_bytes());
    }

    #[test]
    fn test_charsets() {
        assert_eq!(Some(vec![0xFE, 0xFF, 0, b'a']), encode_charset("UTF-16", "a"));
        assert_eq!(Some(vec![b'?']), encode_charset("US-ASCII", "é"));
        assert_eq!(Some(vec![0xE9]), encode_charset("ISO-8859-1", "é"));
        assert_eq!(None, encode_charset("EBCDIC", "a"));
    }
}
