use crate::{
    mdb::types::{
        BinaryDataEncoding, DataEncoding, IntegerDataEncoding, IntegerEncodingType, StringBoxSize,
        StringDataEncoding, StringSize,
    },
    value::{ContainerPosition, ContainerPositionDetails, Value},
};

use super::{MdbError, ProcCtx};

pub(crate) fn extract_encoding(
    encoding: &DataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition), MdbError> {
    match encoding {
        DataEncoding::Integer(ide) => extract_integer(ide, ctx),
        DataEncoding::Binary(bde) => extract_binary(bde, ctx),
        DataEncoding::Boolean(bde) => todo!(),
        DataEncoding::Float(fde) => todo!(),
        DataEncoding::String(sde) => extract_string(sde, ctx),
        DataEncoding::None => panic!("shouldn't be here"),
    }
}

fn extract_integer(
    ide: &IntegerDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition), MdbError> {
    let cctx = &mut ctx.cbuf;
    let bitbuf = &mut cctx.buf;

    bitbuf.set_byte_order(ide.byte_order);
    let numbits = ide.size_in_bits as usize;
    let bit_offset = bitbuf.get_position() as u32;

    let start_offset = cctx.start_offset;

    let mut bv = bitbuf.get_bits(numbits);

    let v = match ide.encoding {
        IntegerEncodingType::Unsigned => Value::uint_value(numbits, bv),
        IntegerEncodingType::TwosComplement => {
            let n = 64 - numbits;
            // shift left to get the sign and back again
            let x = bv as i64;
            Value::int_value(numbits, (x << n) >> n)
        }
        IntegerEncodingType::SignMagnitude => {
            let negative = (bv >> (numbits - 1) & 1) == 1;

            if negative {
                let x = (bv & ((1 << (numbits - 1)) - 1)) as i64; // remove the sign bit
                Value::int_value(numbits, -x)
            } else {
                Value::int_value(numbits, bv as i64)
            }
        }
        IntegerEncodingType::OnesComplement => {
            let negative = (bv >> (numbits - 1) & 1) == 1;
            if negative {
                let n = 64 - numbits;
                let mut x = bv as i64;
                x = (x << n) >> n;
                x = !x;
                Value::int_value(numbits, -x)
            } else {
                Value::int_value(numbits, bv as i64)
            }
        }
    };
    Ok((
        v,
        ContainerPosition {
            start_offset,
            bit_offset,
            bit_size: numbits as u32,
            details: ContainerPositionDetails::None,
        },
    ))
}

fn extract_binary(
    bde: &BinaryDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition), MdbError> {
    todo!()
}

fn extract_string(
    sde: &StringDataEncoding,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition), MdbError> {
    let position = ctx.cbuf.get_position();
    let start_offset = ctx.cbuf.start_offset;
    let bit_offset = position as u32;

    if position & 7 != 0 {
        return Err(
            ctx.decoding_error("the string data that does not start at byte boundary not supported")
        );
    }

    let remaining = ctx.cbuf.remaining_bytes() as u32;

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
                return Err(MdbError::DecodingError(format!(
                    "the fixed size of string exceeds the box or remaining size: {}>{}",
                    strsize, bmr
                )));
            }
            strsize
        }
        StringSize::LeadingSize(tag_size) => {
            if tag_size > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the size in bytes of the size tag {} exceeds the box size {}",
                    tag_size, bmr
                )));
            }
            let size = ctx.cbuf.get_bits((tag_size * 8) as usize) as u32;
            if tag_size + size > bmr {
                return Err(ctx.decoding_error(&format!(
                    "the size in bytes of the string {} exceeds the box size {}",
                    (tag_size + size),
                    bmr
                )));
            }
            box_size.get_or_insert(tag_size + size);
            size
        }
        StringSize::TerminationChar(termination_char) => {
            let mut strsize = 0;

            while strsize < bmr && ctx.cbuf.get_byte() != termination_char {
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
        StringSize::Custom => todo!(),
    };
    assert!(box_size.is_some());

    // extract the string
    let b = ctx.cbuf.get_bytes_ref(string_size_in_bytes as usize);

    let v = match sde.encoding.as_str() {
        "UTF-8" => String::from_utf8_lossy(b).into_owned(),
        // "UTF-16" => String::from_utf16_lossy(b),
        _ => todo!(),
    };

    //set the buffer position at the end of the box
    let bit_size = 8 * box_size.unwrap();
    ctx.cbuf.set_position(position + bit_size as usize);

    let cp = ContainerPosition {
        start_offset,
        bit_offset,
        bit_size,
        details: ContainerPositionDetails::None,
    };
    Ok((Value::StringValue(Box::new(v)), cp))
}
