use std::collections::HashMap;

use crate::{
    mdb::{
        types::{DataEncoding, DataType, TypeData, AggregateDataType, EnumeratedDataType},
        NameIdx, NamedItem,
    },
    value::{AggregateValue, ContainerPosition, EngValue, EnumeratedValue, RawValue, ValueUnion}, error::MdbError,
};

use super::{encodings::extract_encoding, ProcCtx};

pub(crate) fn extract(ptype: &DataType, ctx: &mut ProcCtx) -> Result<RawValue, MdbError> {
    let mdb = ctx.mdb();
    if let DataEncoding::None = ptype.encoding {
        match &ptype.type_data {
            TypeData::Aggregate(atype) => extract_aggregate(atype, ctx),
            TypeData::Array(_) => todo!(),
            _ => {
                return Err(MdbError::InvalidMdb(format!(
                    "base data type without encoding: {}",
                    mdb.name2str(ptype.name())
                )));
            }
        }
    } else {
        return extract_encoding(&ptype.encoding, ctx);
    }
}

fn extract_aggregate(
    atype: &AggregateDataType,
    ctx: &mut ProcCtx,
) -> Result<RawValue, MdbError> {
    let mdb = ctx.mdb();

    let mut aggrm = HashMap::<NameIdx, RawValue>::new();
    let bit_offset0 = ctx.cbuf.buf.get_position();
    let start_offset = ctx.cbuf.start_offset;

    for m in &atype.members {
        let dtype = mdb.get_data_type(m.dtype);
        let member_rv = extract(dtype, ctx)?;
        println!("Inserting for member {} value {:?}", mdb.name2str(m.name()), member_rv);
        aggrm.insert(m.name(), member_rv);
    }
    let aggrv = AggregateValue(aggrm);

    let bit_offset1 = ctx.cbuf.buf.get_position();
    let rv = RawValue {
        v: ValueUnion::Aggregate(Box::new(aggrv)),
        extra: ContainerPosition {
            start_offset,
            bit_offset: bit_offset1 as u32,
            bit_size: (bit_offset1 - bit_offset0) as u32,
        },
    };

    Ok(rv)
}

pub(crate) fn calibrate(
    rawv: &RawValue,
    dtype: &DataType,
    ctx: &mut ProcCtx,
) -> Result<EngValue, MdbError> {
    match &rawv.v {
        ValueUnion::Int64(v) => from_signed_integer(*v, dtype, ctx),
        ValueUnion::Uint64(v) => from_unsigned_integer(*v, dtype, ctx),
        ValueUnion::Double(v) => todo!(),
        ValueUnion::Boolean(_) => todo!(),
        ValueUnion::StringValue(v) => todo!(),
        ValueUnion::Binary(v) => todo!(),
        ValueUnion::Aggregate(v) => from_aggregate(v, dtype, ctx),
        _ => panic!("Unexpected raw data type {:?}", rawv.v),
    }
}

fn from_signed_integer(v: i64, dt: &DataType, _ctx: &ProcCtx) -> Result<EngValue, MdbError> {
    if let Some(cal) = &dt.calibrator {
        todo!()
    }

    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                ValueUnion::int_value(bitsize, v)
            } else {
                let v1 = if v < 0 { 0 } else { v as u64 };
                ValueUnion::uint_value(bitsize, v1)
            }
        }
        TypeData::Float(_) => ValueUnion::Double(v as f64),
        TypeData::String(_) => ValueUnion::StringValue(Box::new(v.to_string())),
        TypeData::Boolean(_) => ValueUnion::Boolean(v != 0),
        TypeData::Enumerated(edt) => ValueUnion::Enumerated(get_enumeration(edt, v)),
        TypeData::AbsoluteTime(_) => todo!(),
        _ => {
            return Err(MdbError::InvalidValue(format!(
                "cannot convert integer to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(EngValue { v: x, extra: () })
}

fn from_unsigned_integer(v: u64, dt: &DataType, _ctx: &ProcCtx) -> Result<EngValue, MdbError> {
    if let Some(cal) = &dt.calibrator {
        todo!()
    }
    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                if v > i64::MAX as u64 {
                    ValueUnion::uint_value(bitsize, i64::MAX as u64)
                } else {
                    ValueUnion::uint_value(bitsize, v)
                }
            } else {
                println!("----------------- bitsize: {}, v: {}", bitsize, v);
                ValueUnion::uint_value(bitsize, v)
            }
        }
        TypeData::Float(_) => ValueUnion::Double(v as f64),
        TypeData::String(_) => ValueUnion::StringValue(Box::new(v.to_string())),
        TypeData::Boolean(_) => ValueUnion::Boolean(v != 0),
        TypeData::Enumerated(edt) => ValueUnion::Enumerated(get_enumeration(edt, v as i64)),
        TypeData::AbsoluteTime(_) => todo!(),
        _ => {
            return Err(MdbError::InvalidValue(format!(
                "cannot convert unsigned integer to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(EngValue { v: x, extra: () })
}

fn from_aggregate(
    aggr_rv: &Box<AggregateValue<ContainerPosition>>,
    dt: &DataType,
    ctx: &mut ProcCtx,
) -> Result<EngValue, MdbError> {
    let mdb = ctx.mdb();
    let mut aggrm = HashMap::<NameIdx, EngValue>::new();

    if let TypeData::Aggregate(atype) = &dt.type_data {
        for m in &atype.members {
            let member_rv = aggr_rv.0.get(&m.name()).ok_or(MdbError::InvalidValue(format!(
                "Error when calibrating aggregate value for type:
            aggregate raw value does not contain value for member {}.
            Got value: {:?} )",
                mdb.name2str(m.name()),
                *aggr_rv
            )))?;

            let dtype = mdb.get_data_type(m.dtype);
            let member_ev = calibrate(member_rv, dtype, ctx)?;
            aggrm.insert(m.name(), member_ev);
        }
    } else {
        let serr = format!("Got aggregate value for type {:?})", dt);
        return Err(MdbError::InvalidValue(serr));
    }

    let ev = EngValue { v: ValueUnion::Aggregate(Box::new(AggregateValue(aggrm))), extra: () };

    Ok(ev)
}

fn get_enumeration(edt: &EnumeratedDataType, v: i64) -> Box<EnumeratedValue> {
    for e in &edt.enumeration {
        if e.value <= v && v <= e.max_value {
            return Box::new(EnumeratedValue { key: v, value: e.label.clone() });
        }
    }

    return Box::new(EnumeratedValue { key: v, value: String::from("UNDEF") });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proc::containers::process;
    use std::path::Path;
    // use lasso::{Rodeo, Spur};

    #[test]
    fn test_bogus2() {
        let x: i64 = i32::MIN as i64 - 100;
        let y = x as i32;
        let z: f64 = x as f64;
        let a: i32 = z as i32;

        println!("x: {}, y: {}, z: {}", x, y, z);
    }

    fn integer_convert<F, T>(x: F) -> T
    where
        T: TryFrom<F>,
        <T as TryFrom<F>>::Error: std::fmt::Debug,
    {
        x.try_into().unwrap()
    }
}
