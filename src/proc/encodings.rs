use crate::{
    bitbuffer::ByteOrder,
    mdb::types::{
        BinaryDataEncoding, BinarySize, BooleanDataEncoding, DataEncoding, FloatDataEncoding,
        FloatEncodingType, IntegerDataEncoding, IntegerEncodingType, StringBoxSize,
        StringDataEncoding, StringSize,
    },
    value::{ContainerPosition, ContainerPositionDetails, Value},
};

use super::{ProcCtx, Result};

/// Extracts the raw value from the packet using the given encoding
pub(crate) fn extract_encoding(
    encoding: &DataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    match encoding {
        DataEncoding::Integer(ide) => extract_integer(ide, ctx),
        DataEncoding::Binary(bde) => extract_binary(bde, ctx),
        DataEncoding::Boolean(bde) => extract_boolean(bde, ctx),
        DataEncoding::Float(fde) => extract_float(fde, ctx),
        DataEncoding::String(sde) => extract_string(sde, ctx),
        DataEncoding::None => Err(ctx.decoding_error("data type without encoding")),
    }
}

fn simple_position(start_offset: u32, bit_offset: u32, bit_size: u32) -> ContainerPosition {
    ContainerPosition { start_offset, bit_offset, bit_size, details: ContainerPositionDetails::None }
}

fn extract_integer(
    ide: &IntegerDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let numbits = ide.size_in_bits as usize;
    if numbits == 0 || numbits > 64 {
        return Err(ctx.decoding_error(&format!("invalid integer size in bits {}", numbits)));
    }
    let cctx = &mut ctx.cbuf;
    let start_offset = cctx.start_offset;
    let bitbuf = &mut cctx.buf;
    bitbuf.set_byte_order(ide.byte_order);
    let bit_offset = bitbuf.get_position() as u32;
    let bv = bitbuf.get_bits(numbits)?;

    let v = decode_integer(ide.encoding, numbits, bv);

    Ok((v, simple_position(start_offset, bit_offset, numbits as u32)))
}

/// decodes the raw bits (right aligned in bv) according to the integer encoding
pub(crate) fn decode_integer(encoding: IntegerEncodingType, numbits: usize, bv: u64) -> Value {
    debug_assert!(numbits > 0 && numbits <= 64);
    let shift = 64 - numbits;
    let sign_bit = (bv >> (numbits - 1)) & 1 == 1;
    // mask of the bits excluding the sign bit
    let mag_mask = if numbits == 1 { 0 } else { u64::MAX >> (65 - numbits) };

    match encoding {
        IntegerEncodingType::Unsigned => Value::uint_value(numbits, bv),
        IntegerEncodingType::TwosComplement => {
            // shift left to get the sign and back again
            Value::int_value(numbits, ((bv << shift) as i64) >> shift)
        }
        IntegerEncodingType::SignMagnitude => {
            let x = (bv & mag_mask) as i64;
            Value::int_value(numbits, if sign_bit { -x } else { x })
        }
        IntegerEncodingType::OnesComplement => {
            if sign_bit {
                // the magnitude is the bitwise complement of the value
                let x = (!bv & mag_mask) as i64;
                Value::int_value(numbits, -x)
            } else {
                Value::int_value(numbits, bv as i64)
            }
        }
    }
}

fn extract_boolean(
    bde: &BooleanDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let numbits = match bde.size_in_bits {
        BinarySize::Fixed(n) if n > 0 && n <= 64 => n as usize,
        _ => {
            return Err(ctx.decoding_error("only fixed size (1-64 bits) boolean encoding is supported"))
        }
    };
    let start_offset = ctx.cbuf.start_offset;
    let bit_offset = ctx.cbuf.get_position() as u32;
    ctx.cbuf.buf.set_byte_order(ByteOrder::BigEndian);
    let bv = ctx.cbuf.get_bits(numbits)?;
    Ok((Value::Boolean(bv != 0), simple_position(start_offset, bit_offset, numbits as u32)))
}

fn extract_binary(
    bde: &BinaryDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let start_offset = ctx.cbuf.start_offset;
    let position = ctx.cbuf.get_position();

    let size_in_bits = match &bde.size_in_bits {
        BinarySize::Fixed(n) => *n as usize,
        BinarySize::Dynamic(dv) => ctx.get_dynamic_uint_value(dv)? as usize,
        BinarySize::LeadingSize(tag_bits) => {
            ctx.cbuf.buf.set_byte_order(ByteOrder::BigEndian);
            let size_in_bytes = ctx.cbuf.get_bits(*tag_bits as usize)? as usize;
            size_in_bytes.checked_mul(8).ok_or_else(|| {
                ctx.decoding_error(&format!("invalid binary leading size {}", size_in_bytes))
            })?
        }
        BinarySize::Custom(algo) => {
            return Err(ctx.decoding_error(&format!(
                "custom binary decoding algorithm '{}' is not supported",
                algo
            )))
        }
    };

    let remaining = ctx.cbuf.buf.remaining_bits();
    if size_in_bits > remaining {
        return Err(ctx.decoding_error(&format!(
            "binary data size {} bits exceeds the remaining packet size {} bits",
            size_in_bits, remaining
        )));
    }

    let data_pos = ctx.cbuf.get_position();
    let v = if data_pos & 7 == 0 && size_in_bits & 7 == 0 {
        ctx.cbuf.get_bytes_ref(size_in_bits / 8)?.to_vec()
    } else {
        // not aligned: read full bytes, the remaining bits (if any) are placed right aligned in the last byte
        ctx.cbuf.buf.set_byte_order(ByteOrder::BigEndian);
        let mut v = Vec::with_capacity((size_in_bits + 7) / 8);
        let mut n = size_in_bits;
        while n > 0 {
            let k = n.min(8);
            v.push(ctx.cbuf.get_bits(k)? as u8);
            n -= k;
        }
        v
    };

    let bit_size = (ctx.cbuf.get_position() - position) as u32;
    Ok((Value::Binary(Box::new(v)), simple_position(start_offset, position as u32, bit_size)))
}

fn extract_string(
    sde: &StringDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let position = ctx.cbuf.get_position();
    let start_offset = ctx.cbuf.start_offset;
    let bit_offset = position as u32;

    if position & 7 != 0 {
        return Err(
            ctx.decoding_error("the string data that does not start at byte boundary not supported")
        );
    }
    let remaining = ctx.cbuf.remaining_bytes()? as u32;

    // bmr = max box size  or remaining packet size
    let mut bmr = sde.max_box_size_in_bytes.filter(|m| *m < remaining).unwrap_or(remaining);

    // first determine the box size
    let mut box_size = match &sde.box_size_in_bits {
        StringBoxSize::Undefined => None,
        StringBoxSize::Fixed(x) => {
            let bsize = x / 8;
            if bsize > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the fixed size of string buffer exceeds the remaining size in bytes: {} > {}",
                    bsize, bmr
                )));
            }
            bmr = bsize;
            Some(bmr)
        }
        StringBoxSize::Dynamic(x) => {
            let x = ctx.get_dynamic_uint_value(x)?;
            let bsize = (x / 8) as u32;
            if bsize > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the dynamic size of string buffer exceeds the remaining size in bytes: {}>{}",
                    bsize, bmr
                )));
            }
            bmr = bsize;
            Some(bmr)
        }
    };

    // find the string size
    let string_size_in_bytes = match sde.size_in_bits {
        StringSize::Fixed(x) => {
            let strsize = x / 8;
            if strsize > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the fixed size of string exceeds the box or remaining size: {}>{}",
                    strsize, bmr
                )));
            }
            box_size.get_or_insert(strsize);
            strsize
        }
        StringSize::LeadingSize(tag_size) => {
            if tag_size > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the size in bytes of the size tag {} exceeds the box size {}",
                    tag_size, bmr
                )));
            }
            ctx.cbuf.buf.set_byte_order(ByteOrder::BigEndian);
            let size = ctx.cbuf.get_bits((tag_size * 8) as usize)?;
            if tag_size as u64 + size > bmr as u64 {
                return Err(ctx.decoding_error(&format!(
                    "the size in bytes of the string {} exceeds the box size {}",
                    (tag_size as u64 + size),
                    bmr
                )));
            }
            let size = size as u32;
            box_size.get_or_insert(tag_size + size);
            size
        }
        StringSize::TerminationChar(termination_char) => {
            let mut strsize = 0;
            while strsize < bmr && ctx.cbuf.get_byte()? != termination_char {
                strsize += 1;
            }

            if box_size.is_none() {
                if strsize == bmr {
                    // if the box size is not set we do not want to just eat the remaining of the packet
                    return Err(ctx.decoding_error(&format!(
                        "cannot find string terminator 0x{:x}",
                        termination_char
                    )));
                }
                box_size.get_or_insert(strsize + 1);
            }
            //put back the position at the beginning of the string
            ctx.cbuf.set_position(position);
            strsize
        }
        StringSize::Custom => {
            return Err(ctx.decoding_error("custom string size algorithms are not supported"))
        }
    };
    // box_size is always set by one of the branches above
    let box_size = box_size.unwrap_or(string_size_in_bytes);

    // extract the string
    let b = ctx.cbuf.get_bytes_ref(string_size_in_bytes as usize)?;
    let v = decode_string(&sde.encoding, b)
        .ok_or_else(|| ctx.decoding_error(&format!("unsupported string encoding '{}'", sde.encoding)))?;

    //set the buffer position at the end of the box
    let bit_size = 8 * box_size;
    ctx.cbuf.set_position(position + bit_size as usize);

    Ok((Value::StringValue(Box::new(v)), simple_position(start_offset, bit_offset, bit_size)))
}

/// returns true if the character set is supported by the string decoder
pub fn is_supported_charset(encoding: &str) -> bool {
    decode_string(encoding, &[]).is_some()
}

/// decodes the bytes into a string using the given character set.
/// Invalid characters are replaced with the unicode replacement character.
/// Returns None if the character set is not supported.
pub(crate) fn decode_string(encoding: &str, b: &[u8]) -> Option<String> {
    let s = match encoding.to_ascii_uppercase().as_str() {
        "UTF-8" | "UTF8" => String::from_utf8_lossy(b).into_owned(),
        "US-ASCII" | "ASCII" => {
            b.iter().map(|&c| if c < 0x80 { c as char } else { char::REPLACEMENT_CHARACTER }).collect()
        }
        "ISO-8859-1" | "LATIN1" | "ISO_8859_1" => b.iter().map(|&c| c as char).collect(),
        "UTF-16" => {
            // like Java: big endian unless a byte order mark says otherwise; the BOM is not part of the string
            if b.len() >= 2 && b[0] == 0xFF && b[1] == 0xFE {
                decode_utf16(&b[2..], false)
            } else if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
                decode_utf16(&b[2..], true)
            } else {
                decode_utf16(b, true)
            }
        }
        "UTF-16BE" => decode_utf16(b, true),
        "UTF-16LE" => decode_utf16(b, false),
        _ => return None,
    };
    Some(s)
}

fn decode_utf16(b: &[u8], big_endian: bool) -> String {
    let units = b.chunks_exact(2).map(|c| {
        if big_endian {
            u16::from_be_bytes([c[0], c[1]])
        } else {
            u16::from_le_bytes([c[0], c[1]])
        }
    });
    let mut s: String =
        char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect();
    if b.len() % 2 == 1 {
        s.push(char::REPLACEMENT_CHARACTER);
    }
    s
}

fn extract_float(
    fde: &FloatDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let numbits = fde.size_in_bits as usize;
    let cctx = &mut ctx.cbuf;
    let start_offset = cctx.start_offset;
    let bitbuf = &mut cctx.buf;
    bitbuf.set_byte_order(fde.byte_order);
    let bit_offset = bitbuf.get_position() as u32;

    let bv = bitbuf.get_bits(numbits)?;

    let v = match (fde.encoding, numbits) {
        (FloatEncodingType::IEEE754_1985, 16) => Value::Double(half_to_f64(bv as u16)),
        (FloatEncodingType::IEEE754_1985, 32) => Value::Double(f32::from_bits(bv as u32) as f64),
        (FloatEncodingType::IEEE754_1985, 64) => Value::Double(f64::from_bits(bv)),
        (FloatEncodingType::Milstd1750a, 32) => Value::Double(milstd1750a32_to_f64(bv as u32)),
        (FloatEncodingType::Milstd1750a, 48) => Value::Double(milstd1750a48_to_f64(bv)),
        _ => {
            return Err(ctx.decoding_error(&format!(
                "unsupported float encoding {:?} with size in bits {}",
                fde.encoding, numbits
            )))
        }
    };

    Ok((v, simple_position(start_offset, bit_offset, numbits as u32)))
}

/// converts IEEE 754 half precision (binary16) to f64
pub(crate) fn half_to_f64(h: u16) -> f64 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1F) as i32;
    let frac = (h & 0x3FF) as f64;
    let v = match exp {
        0 => frac * 2f64.powi(-24),
        0x1F => {
            if frac == 0.0 {
                f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => (1.0 + frac / 1024.0) * 2f64.powi(exp - 15),
    };
    sign * v
}

/// converts a MIL-STD-1750A 32 bits float
/// (24 bits two's complement mantissa followed by 8 bits two's complement exponent) to f64
pub(crate) fn milstd1750a32_to_f64(x: u32) -> f64 {
    let mantissa = ((x as i32) >> 8) as f64; // arithmetic shift keeps the sign
    let exp = (x & 0xFF) as u8 as i8 as i32;
    mantissa * 2f64.powi(exp - 23)
}

/// converts a MIL-STD-1750A 48 bits extended float
/// (24 bits MSB of the mantissa, 8 bits exponent, 16 bits LSB of the mantissa) to f64
pub(crate) fn milstd1750a48_to_f64(x: u64) -> f64 {
    let m_hi = (x >> 24) & 0xFF_FFFF;
    let exp = ((x >> 16) & 0xFF) as u8 as i8 as i32;
    let m_lo = x & 0xFFFF;
    // 40 bits two's complement mantissa, sign extend to 64 bits
    let m = (((m_hi << 16) | m_lo) << 24) as i64 >> 24;
    (m as f64) * 2f64.powi(exp - 39)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_integer() {
        use IntegerEncodingType::*;
        assert_eq!(Value::Int64(-1), decode_integer(TwosComplement, 8, 0xFF));
        assert_eq!(Value::Int64(-1), decode_integer(TwosComplement, 64, u64::MAX));
        assert_eq!(Value::Int64(-1), decode_integer(TwosComplement, 1, 1));
        assert_eq!(Value::Int64(-5), decode_integer(SignMagnitude, 8, 0x85));
        assert_eq!(Value::Int64(5), decode_integer(SignMagnitude, 8, 0x05));
        assert_eq!(Value::Int64(0), decode_integer(SignMagnitude, 1, 1));
        assert_eq!(Value::Int64(-5), decode_integer(OnesComplement, 8, 0xFA));
        assert_eq!(Value::Int64(5), decode_integer(OnesComplement, 8, 0x05));
        assert_eq!(Value::Uint64(0xFF), decode_integer(Unsigned, 8, 0xFF));
    }

    #[test]
    fn test_half() {
        assert_eq!(1.0, half_to_f64(0x3C00));
        assert_eq!(-2.0, half_to_f64(0xC000));
        assert_eq!(65504.0, half_to_f64(0x7BFF));
        assert_eq!(f64::INFINITY, half_to_f64(0x7C00));
        assert!(half_to_f64(0x7E00).is_nan());
        assert_eq!(2f64.powi(-24), half_to_f64(0x0001));
    }

    #[test]
    fn test_milstd1750a() {
        // examples from the MIL-STD-1750A specification
        assert_eq!((1.0 - 2f64.powi(-23)) * 2f64.powi(127), milstd1750a32_to_f64(0x7FFFFF7F));
        assert_eq!(0.5, milstd1750a32_to_f64(0x40000000));
        assert_eq!(-1.0, milstd1750a32_to_f64(0x80000000));
        assert_eq!(0.0, milstd1750a32_to_f64(0x00000000));
        assert_eq!(0.75 * 2f64.powi(-128), milstd1750a32_to_f64(0x60000080));
        assert_eq!(-0.75 * 2f64.powi(4), milstd1750a32_to_f64(0xA0000004));

        assert_eq!(0.5, milstd1750a48_to_f64(0x400000_00_0000));
        assert_eq!(-1.0, milstd1750a48_to_f64(0x800000_00_0000));
        assert_eq!(0.5 * 2f64.powi(127), milstd1750a48_to_f64(0x400000_7F_0000));
    }

    #[test]
    fn test_strings() {
        assert_eq!(Some("ab".to_owned()), decode_string("UTF-8", b"ab"));
        assert_eq!(Some("é".to_owned()), decode_string("ISO-8859-1", &[0xE9]));
        assert_eq!(Some("ab".to_owned()), decode_string("UTF-16", &[0, b'a', 0, b'b']));
        assert_eq!(Some("ab".to_owned()), decode_string("UTF-16", &[0xFF, 0xFE, b'a', 0, b'b', 0]));
        assert_eq!(Some("ab".to_owned()), decode_string("UTF-16LE", &[b'a', 0, b'b', 0]));
        assert_eq!(None, decode_string("EBCDIC", b"ab"));
    }
}
