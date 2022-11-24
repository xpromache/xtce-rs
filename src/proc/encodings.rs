use crate::{
    mdb::types::{BinaryDataEncoding, DataEncoding, IntegerDataEncoding, IntegerEncodingType},
    value::{Value, ContainerPosition, ContainerPositionDetails},
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
        DataEncoding::String(sde) => todo!(),
        DataEncoding::None => panic!("shouldn't be here"),
    }
}

fn extract_integer(ide: &IntegerDataEncoding, ctx: &mut ProcCtx) -> Result<(Value, ContainerPosition), MdbError> {
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
    Ok((v, ContainerPosition { start_offset, bit_offset, bit_size: numbits as u32 , details: ContainerPositionDetails::None}))

}

fn extract_binary(bde: &BinaryDataEncoding, ctx: &mut ProcCtx) -> Result<(Value, ContainerPosition), MdbError> {
    todo!()
}
