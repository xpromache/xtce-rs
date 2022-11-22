use crate::{
    mdb::types::{BinaryDataEncoding, DataEncoding, IntegerDataEncoding, IntegerEncodingType},
    value::{RawValue, ValueUnion, ContainerPosition},
};

use super::{MdbError, ProcCtx};

pub(crate) fn extract_encoding(
    encoding: &DataEncoding,
    ctx: &mut ProcCtx,
) -> Result<RawValue, MdbError> {
    match encoding {
        DataEncoding::Integer(ide) => extract_integer(ide, ctx),
        DataEncoding::Binary(bde) => extract_binary(bde, ctx),
        DataEncoding::Boolean(bde) => todo!(),
        DataEncoding::Float(fde) => todo!(),
        DataEncoding::String(sde) => todo!(),
        DataEncoding::None => panic!("shouldn't be here"),
    }
}

fn extract_integer(ide: &IntegerDataEncoding, ctx: &mut ProcCtx) -> Result<RawValue, MdbError> {
    let cctx = &mut ctx.cbuf;
    let bitbuf = &mut cctx.buf;

    bitbuf.set_byte_order(ide.byte_order);
    let numbits = ide.size_in_bits as usize;
    let bit_offset = bitbuf.get_position() as u32;

    let start_offset = cctx.start_offset;

    let mut bv = bitbuf.get_bits(numbits);

    let v = match ide.encoding {
        IntegerEncodingType::Unsigned => ValueUnion::uint_value(numbits, bv),
        IntegerEncodingType::TwosComplement => {
            let n = 64 - numbits;
            // shift left to get the sign and back again
            let x = bv as i64;
            ValueUnion::int_value(numbits, (x << n) >> n)
        }
        IntegerEncodingType::SignMagnitude => {
            let negative = (bv >> (numbits - 1) & 1) == 1;

            if negative {
                let x = (bv & ((1 << (numbits - 1)) - 1)) as i64; // remove the sign bit
                ValueUnion::int_value(numbits, -x)
            } else {
                ValueUnion::int_value(numbits, bv as i64)
            }
        }
        IntegerEncodingType::OnesComplement => {
            let negative = (bv >> (numbits - 1) & 1) == 1;
            if negative {
                let n = 64 - numbits;
                let mut x = bv as i64;
                x = (x << n) >> n;
                x = !x;
                ValueUnion::int_value(numbits, -x)
            } else {
                ValueUnion::int_value(numbits, bv as i64)
            }
        }
    };
    Ok(RawValue{v, extra: ContainerPosition { start_offset, bit_offset, bit_size: numbits as u32 }})

}

fn extract_binary(bde: &BinaryDataEncoding, ctx: &mut ProcCtx) -> Result<RawValue, MdbError> {
    todo!()
}
