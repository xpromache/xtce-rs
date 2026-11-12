use std::collections::HashMap;

use crate::{
    mdb::{
        types::{
            AggregateDataType, ArrayDataType, Calibrator, DataEncoding, DataType,
            EnumeratedDataType, TypeData,
        },
        IntegerValue, NameIdx, NamedItem,
    },
    proc::criteria_evaluator::MatchResult,
    value::{
        AggregateValue, ArrayValue, ContainerPosition, ContainerPositionDetails, EnumeratedValue,
        Value,
    },
};

use super::{calibrators, encodings::extract_encoding, ProcCtx, Result, ProcError};

/// maximum number of elements in an array (same default as in Yamcs)
pub const MAX_ARRAY_SIZE: usize = 10000;

pub(crate) fn extract(ptype: &DataType, ctx: &mut ProcCtx) -> Result<(Value, ContainerPosition)> {
    let mdb = ctx.mdb();
    if let DataEncoding::None = ptype.encoding {
        match &ptype.type_data {
            TypeData::Aggregate(atype) => extract_aggregate(atype, ctx),
            TypeData::Array(atype) => {
                if atype.dim.is_empty() {
                    return Err(ctx.decoding_error(
                        "the array size is not specified in the type (it has to be specified in an ArrayParameterRefEntry)",
                    ));
                }
                extract_array(atype, &atype.dim, ctx)
            }
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
            bit_offset: bit_offset0 as u32,
            bit_size: (bit_offset1 - bit_offset0) as u32,
            details: ContainerPositionDetails::Aggregate(posm)
        };
    

    Ok((rv, cpos))
}

/// extracts an array with the given dimensions (which may be different than the ones from the type)
pub(crate) fn extract_array(
    atype: &ArrayDataType,
    dims: &[IntegerValue],
    ctx: &mut ProcCtx,
) -> Result<(Value, ContainerPosition)> {
    let mdb = ctx.mdb();
    let mut sizes = Vec::with_capacity(dims.len());
    let mut total: usize = 1;
    for iv in dims {
        let s = ctx.get_integer_value(iv)?;
        if s < 0 {
            return Err(ctx.decoding_error(&format!("negative array size {}", s)));
        }
        let s = s as usize;
        total = total.saturating_mul(s);
        if s > MAX_ARRAY_SIZE || total > MAX_ARRAY_SIZE {
            return Err(ctx.decoding_error(&format!(
                "array size exceeds the maximum allowed {}",
                MAX_ARRAY_SIZE
            )));
        }
        sizes.push(s);
    }

    let start_offset = ctx.cbuf.start_offset;
    let bit_offset0 = ctx.cbuf.get_position();
    let etype = mdb.get_data_type(atype.dtype);
    let mut values = Vec::with_capacity(total);
    let mut positions = Vec::with_capacity(total);
    for _ in 0..total {
        let (v, cpos) = extract(etype, ctx)?;
        values.push(v);
        positions.push(cpos);
    }
    let bit_offset1 = ctx.cbuf.get_position();

    let cpos = ContainerPosition {
        start_offset,
        bit_offset: bit_offset0 as u32,
        bit_size: (bit_offset1 - bit_offset0) as u32,
        details: ContainerPositionDetails::Array(positions),
    };
    Ok((Value::Array(Box::new(ArrayValue { dims: sizes, values })), cpos))
}

/// returns the calibrator to be used: the first context calibrator whose context matches or the default one
fn select_calibrator<'a>(dtype: &'a DataType, ctx: &ProcCtx) -> Option<&'a Calibrator> {
    for cc in &dtype.context_calibrators {
        if ctx.pdata.get_criteria_evaluator(cc.context).evaluate(ctx) == MatchResult::OK {
            return Some(&cc.calibrator);
        }
    }
    dtype.calibrator.as_ref()
}

/// applies the calibrator for integer and float types; returns None if there is no calibrator
fn apply_calibrator(rawv: &Value, dtype: &DataType, ctx: &ProcCtx) -> Result<Option<Value>> {
    let cal = match &dtype.type_data {
        TypeData::Integer(_) | TypeData::Float(_) => match select_calibrator(dtype, ctx) {
            Some(cal) => cal,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    let x = match rawv {
        Value::Int64(v) => *v as f64,
        Value::Uint64(v) => *v as f64,
        Value::Double(v) => *v,
        Value::StringValue(v) => v.trim().parse::<f64>().map_err(|_| {
            ProcError::DecodingError(format!("Cannot convert raw value '{}' to number for calibration", v))
        })?,
        _ => {
            return Err(ProcError::DecodingError(format!(
                "Cannot calibrate raw value {:?}",
                rawv
            )))
        }
    };
    let y = calibrators::calibrate(cal, x)?;

    // conversion to the engineering type like in Yamcs (rounding for integers)
    let v = match &dtype.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                Value::int_value(bitsize, y.round() as i64)
            } else if y >= 0.0 {
                Value::uint_value(bitsize, y.round() as u64)
            } else {
                Value::uint_value(bitsize, 0)
            }
        }
        _ => Value::Double(y),
    };
    Ok(Some(v))
}

// transforms the raw value into an egineering value
pub(crate) fn calibrate(
    rawv: &Value,
    dtype: &DataType,
    ctx: &mut ProcCtx,
) -> Result<Value> {
    if let Some(v) = apply_calibrator(rawv, dtype, ctx)? {
        return Ok(v);
    }
    match &rawv {
        Value::Array(v) => from_array(v, dtype, ctx),
        Value::Int64(v) => from_signed_integer(*v, dtype, ctx),
        Value::Uint64(v) => from_unsigned_integer(*v, dtype, ctx),
        Value::Double(v) => from_double(*v, dtype, ctx),
        Value::Boolean(v) => from_boolean(*v, dtype, ctx),
        Value::StringValue(v) => from_string(v, dtype, ctx),
        Value::Binary(v) => from_binary(v, dtype, ctx),
        Value::Aggregate(v) => from_aggregate(v, dtype, ctx),
        Value::Enumerated(_) => {
            Err(ProcError::InvalidValue(format!("Unexpected raw data type {:?}", rawv)))
        }
    }
}

fn from_signed_integer(v: i64, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {

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
        TypeData::AbsoluteTime(_) => return Err(unsupported_time()),
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
    let x = match &dt.type_data {
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                // the value is clamped to the range of the signed type
                Value::int_value(bitsize, rv.min(i64::MAX as u64) as i64)
            } else {
                Value::uint_value(bitsize, rv)
            }
        }
        TypeData::Float(_) => Value::Double(rv as f64),
        TypeData::String(_) => Value::StringValue(Box::new(rv.to_string())),
        TypeData::Boolean(_) => Value::Boolean(rv != 0),
        TypeData::Enumerated(edt) => Value::Enumerated(get_enumeration(edt, rv as i64)),
        TypeData::AbsoluteTime(_) => return Err(unsupported_time()),
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
        TypeData::AbsoluteTime(_) => return Err(unsupported_time()),
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

// computes an array engineering value by calibrating each element
fn from_array(arr_rv: &ArrayValue, dt: &DataType, ctx: &mut ProcCtx) -> Result<Value> {
    let mdb = ctx.mdb();
    if let TypeData::Array(atype) = &dt.type_data {
        let etype = mdb.get_data_type(atype.dtype);
        let mut values = Vec::with_capacity(arr_rv.values.len());
        for v in &arr_rv.values {
            values.push(calibrate(v, etype, ctx)?);
        }
        Ok(Value::Array(Box::new(ArrayValue { dims: arr_rv.dims.clone(), values })))
    } else {
        Err(ProcError::InvalidValue(format!("Got array value for type {:?}", dt)))
    }
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
    let invalid = |tname: &str| {
        ProcError::DecodingError(format!("Cannot convert raw string '{}' to {}", rv, tname))
    };
    let s = rv.trim();
    let x = match &dt.type_data {
        TypeData::String(_) => Value::StringValue(Box::new(rv.to_owned())),
        TypeData::Integer(idt) => {
            let bitsize = idt.size_in_bits as usize;
            if idt.signed {
                Value::int_value(bitsize, s.parse::<i64>().map_err(|_| invalid("integer"))?)
            } else {
                Value::uint_value(bitsize, s.parse::<u64>().map_err(|_| invalid("unsigned integer"))?)
            }
        }
        TypeData::Float(_) => Value::Double(s.parse::<f64>().map_err(|_| invalid("float"))?),
        TypeData::Boolean(bdt) => {
            if s == bdt.one_string_value {
                Value::Boolean(true)
            } else if s == bdt.zero_string_value {
                Value::Boolean(false)
            } else {
                return Err(invalid("boolean"));
            }
        }
        TypeData::Enumerated(edt) => {
            let v = s.parse::<i64>().map_err(|_| invalid("enumerated"))?;
            Value::Enumerated(get_enumeration(edt, v))
        }
        TypeData::AbsoluteTime(_) => return Err(unsupported_time()),
        TypeData::Binary(_) | TypeData::Aggregate(_) | TypeData::Array(_) => {
            return Err(ProcError::InvalidValue(format!(
                "cannot convert string to {:?}",
                dt.type_data
            )))
        }
    };

    Ok(x)
}

// computes the engineering value from a boolean raw value
fn from_boolean(rv: bool, dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {
    match &dt.type_data {
        TypeData::Boolean(_) => Ok(Value::Boolean(rv)),
        _ => Err(ProcError::InvalidValue(format!("cannot convert boolean to {:?}", dt.type_data))),
    }
}

// computes the engineering value from a binary raw value
fn from_binary(rv: &[u8], dt: &DataType, _ctx: &ProcCtx) -> Result<Value> {
    match &dt.type_data {
        TypeData::Binary(_) => Ok(Value::Binary(Box::new(rv.to_vec()))),
        _ => Err(ProcError::InvalidValue(format!("cannot convert binary to {:?}", dt.type_data))),
    }
}

fn unsupported_time() -> ProcError {
    ProcError::DecodingError("absolute time parameters are not yet supported".to_owned())
}
