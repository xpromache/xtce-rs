use std::collections::HashMap;

use crate::{
    mdb::{
        types::{DataEncoding, DataType, TypeData, AggregateDataType, EnumeratedDataType},
        NameIdx, NamedItem,
    },
    value::{AggregateValue, ContainerPosition, EnumeratedValue, Value, ContainerPositionDetails}};

use super::{encodings::extract_encoding, ProcCtx, Result, ProcError};

pub(crate) fn extract(ptype: &DataType, ctx: &mut ProcCtx) -> Result<(Value, ContainerPosition)> {
    let mdb = ctx.mdb();
    if let DataEncoding::None = ptype.encoding {
        match &ptype.type_data {
            TypeData::Aggregate(atype) => extract_aggregate(atype, ctx),
            TypeData::Array(_) => todo!(),
            _ => {
                return Err(ProcError::InvalidMdb(format!(
                    "base data type without encoding: {}",
                    mdb.name2str(ptype.name())
                )));
            }
        }
    } else {
        return extract_encoding(&ptype.encoding, ctx);
    }
}

// extracts an aggregate from a packet by extracting all members in order
fn extract_aggregate(
    atype: &AggregateDataType,
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let mdb = ctx.mdb();

    let mut aggrm = HashMap::<NameIdx, Value>::new();
    let mut posm = HashMap::<NameIdx, ContainerPosition>::new();
    
    let bit_offset0 = ctx.cbuf.buf.get_position();
    let start_offset = ctx.cbuf.start_offset;

    for m in &atype.members {
        let dtype = mdb.get_data_type(m.dtype);
        let (member_rv, cpos) = extract(dtype, ctx)?;
        aggrm.insert(m.name(), member_rv);
        posm.insert(m.name(), cpos);
    }
    let aggrv = AggregateValue(aggrm);

    let bit_offset1 = ctx.cbuf.buf.get_position();
    let rv = Value::Aggregate(Box::new(aggrv));
    let cpos = ContainerPosition {
            start_offset,
            bit_offset: bit_offset1 as u32,
            bit_size: (bit_offset1 - bit_offset0) as u32,
            details: ContainerPositionDetails::Aggregate(posm)
        };
    

    Ok((rv, cpos))
}

// transforms the raw value into an egineering value
pub(crate) fn calibrate(
    rawv: &Value,
    dtype: &DataType,
    ctx: &mut ProcCtx,
) -> Result<Value> {
    match &rawv {
        Value::Int64(v) => from_signed_integer(*v, dtype, ctx),
        Value::Uint64(v) => from_unsigned_integer(*v, dtype, ctx),
        Value::Double(v) => from_double(*v, dtype, ctx),
        Value::Boolean(_) => todo!(),
        Value::StringValue(v) => from_string(v, dtype, ctx),
        Value::Binary(v) => todo!(),
        Value::Aggregate(v) => from_aggregate(v, dtype, ctx),
        _ => panic!("Unexpected raw data type {:?}", rawv),
    }
}

fn from_signed_integer(v: i64, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {
    if let Some(cal) = &dt.calibrator {
        todo!()
    }

    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                Value::int_value(bitsize, v)
            } else {
                let v1 = if v < 0 { 0 } else { v as u64 };
                Value::uint_value(bitsize, v1)
            }
        }
        TypeData::Float(_) => Value::Double(v as f64),
        TypeData::String(_) => Value::StringValue(Box::new(v.to_string())),
        TypeData::Boolean(_) => Value::Boolean(v != 0),
        TypeData::Enumerated(edt) => Value::Enumerated(get_enumeration(edt, v)),
        TypeData::AbsoluteTime(_) => todo!(),
        _ => {
            return Err(ProcError::InvalidValue(format!(
                "cannot convert integer to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(x)
}

// computes the engineering value from a unsigned integer raw value
fn from_unsigned_integer(rv: u64, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {
    if let Some(cal) = &dt.calibrator {
        todo!()
    }
    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                if rv > i64::MAX as u64 {
                    Value::uint_value(bitsize, i64::MAX as u64)
                } else {
                    Value::uint_value(bitsize, rv)
                }
            } else {
                Value::uint_value(bitsize, rv)
            }
        }
        TypeData::Float(_) => Value::Double(rv as f64),
        TypeData::String(_) => Value::StringValue(Box::new(rv.to_string())),
        TypeData::Boolean(_) => Value::Boolean(rv != 0),
        TypeData::Enumerated(edt) => Value::Enumerated(get_enumeration(edt, rv as i64)),
        TypeData::AbsoluteTime(_) => todo!(),
        _ => {
            return Err(ProcError::InvalidValue(format!(
                "cannot convert unsigned integer to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(x)
}



// computes the engineering value from a double value
fn from_double(rv: f64, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {
    if let Some(cal) = &dt.calibrator {
        todo!()
    }
    
    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {                                   
                Value::int_value(bitsize, rv as i64)                
            } else {
                Value::uint_value(bitsize, rv as u64)
            }
        }
        TypeData::Float(_) => Value::Double(rv),
        TypeData::String(_) => Value::StringValue(Box::new(rv.to_string())),
        TypeData::Boolean(_) => Value::Boolean(rv != 0.0),
        TypeData::Enumerated(edt) => Value::Enumerated(get_enumeration(edt, rv as i64)),
        TypeData::AbsoluteTime(_) => todo!(),
        _ => {
            return Err(ProcError::InvalidValue(format!(
                "cannot convert unsigned integer to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(x)
}
// computes an aggregate engineering value from an aggregate raw value
fn from_aggregate(
    aggr_rv: &Box<AggregateValue>,
    dt: &DataType,
    ctx: &mut ProcCtx,
) -> Result<Value> {
    let mdb = ctx.mdb();
    let mut aggrm = HashMap::<NameIdx, Value>::new();

    if let TypeData::Aggregate(atype) = &dt.type_data {
        for m in &atype.members {
            let member_rv = aggr_rv.0.get(&m.name()).ok_or_else(|| ProcError::InvalidValue(format!(
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
        return Err(ProcError::InvalidValue(serr));
    }

    let ev = Value::Aggregate(Box::new(AggregateValue(aggrm)));

    Ok(ev)
}

// computes an enumerated engineering value from a signed integer raw values
fn get_enumeration(edt: &EnumeratedDataType, rv: i64) -> Box<EnumeratedValue> {
    for e in &edt.enumeration {
        if e.value <= rv && rv <= e.max_value {
            return Box::new(EnumeratedValue { key: rv, value: e.label.clone() });
        }
    }

    return Box::new(EnumeratedValue { key: rv, value: String::from("UNDEF") });
}



// computes the engineering value from a string raw value
fn from_string(rv: &str, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {    
    let x = match &dt.type_data {
        TypeData::String(_) => Value::StringValue(Box::new(rv.to_owned())),
        TypeData::Integer(_) => todo!(),
        TypeData::Float(_) => todo!(),
        TypeData::Binary(_) => todo!(),
        TypeData::Boolean(_) => todo!(),
        TypeData::Enumerated(_) => todo!(),
        TypeData::Aggregate(_) => todo!(),
        TypeData::Array(_) => todo!(),
        TypeData::AbsoluteTime(_) => todo!(),

    };

    Ok(x)
}