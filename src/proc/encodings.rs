use crate::{mdb::{DataEncoding, IntegerDataEncoding, BinaryDataEncoding}, value::RawValue};

use super::{ProcCtx, MdbProcError};

pub(crate) fn extract_encoding(encoding: &DataEncoding, ctx: &mut ProcCtx) ->  Result<RawValue, MdbProcError>  {
    match encoding {
        DataEncoding::Integer(ide) => extract_integer(ide, ctx),
        DataEncoding::Binary(bde) => extract_binary(bde, ctx),
        DataEncoding::Boolean(bde) => todo!(),
        DataEncoding::Float(fde) => todo!(),
        DataEncoding::String(sde) => todo!(),
        DataEncoding::None => panic!("shouldn't be here"),
        
    };
    todo!()
}

fn extract_integer(ide: &IntegerDataEncoding, ctx: &mut ProcCtx) ->  Result<RawValue, MdbProcError>  {
/*
    buffer.setByteOrder(ide.getByteOrder());
    int numBits = ide.getSizeInBits();

    long rv = buffer.getBits(numBits);
    switch (ide.getEncoding()) {
    case UNSIGNED:
        // nothing to do
        break;
    case TWOS_COMPLEMENT:
        int n = 64 - numBits;
        // shift left to get the sign and back again
        rv = (rv << n) >> n;
        break;

    case SIGN_MAGNITUDE:
        boolean negative = ((rv >>> (numBits - 1) & 1L) == 1L);

        if (negative) {
            rv = rv & ((1 << (numBits - 1)) - 1); // remove the sign bit
            rv = -rv;
        }
        break;
    case ONES_COMPLEMENT:
        negative = ((rv >>> (numBits - 1) & 1L) == 1L);
        if (negative) {
            n = 64 - numBits;
            rv = (rv << n) >> n;
            rv = ~rv;
            rv = -rv;
        }
        break;
    default: // shouldn't happen
        throw new IllegalStateException();
    }
    return getRawValue(ide, rv);
    */
    todo!()
}
fn extract_binary(bde: &BinaryDataEncoding, ctx: &mut ProcCtx) ->  Result<RawValue, MdbProcError>  {
    todo!()
}