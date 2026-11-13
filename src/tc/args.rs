//! Conversion of the user provided argument values to engineering values, validation and
//! decalibration (engineering to raw value conversion).

use std::collections::HashMap;

use crate::{
    mdb::{
        types::{
            AbsoluteTimeDataType, AggregateDataType, ArrayDataType, DataEncoding, DataType, EnumeratedDataType,
            TimeEpoch, TimeReference, TypeData,
        },
        IntegerValue, MissionDatabase, NamedItem,
    },
    proc::types::{java_round, narrow_float},
    time,
    value::{AggregateValue, ArrayValue, EnumeratedValue, Value},
};

use super::{
    json::{self, Json},
    Result, TcError,
};

fn invalid<T>(msg: String) -> Result<T> {
    Err(TcError::InvalidValue(msg))
}

/// Converts a user provided value to the engineering value of the given type.
///
/// String values are parsed according to the type (e.g. "12" for an integer, an enumeration label,
/// "2024-01-01T00:00:00Z" for an absolute time, hexadecimal for binary).
pub(crate) fn to_eng_value(mdb: &MissionDatabase, dtype: &DataType, v: &Value) -> Result<Value> {
    let s = match v {
        Value::StringValue(s) => Some(s.trim()),
        _ => None,
    };
    match &dtype.type_data {
        TypeData::Integer(idt) => {
            let x: i128 = match (v, s) {
                (Value::Int64(x), _) => *x as i128,
                (Value::Uint64(x), _) => *x as i128,
                (Value::Double(d), _) if d.fract() == 0.0 && d.abs() < 1.9e19 => *d as i128,
                (_, Some(s)) => parse_int(s).ok_or_else(|| TcError::InvalidValue(format!("invalid integer '{}'", s)))?,
                _ => return invalid(format!("cannot convert {:?} to integer", v)),
            };
            let n = idt.size_in_bits;
            let (min, max) = if idt.signed {
                (-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1)
            } else {
                (0, (1i128 << n) - 1)
            };
            if x < min || x > max {
                return Err(TcError::OutOfRange(format!(
                    "value {} out of the range [{}, {}] of the {} bits {} integer type",
                    x,
                    min,
                    max,
                    n,
                    if idt.signed { "signed" } else { "unsigned" }
                )));
            }
            Ok(if idt.signed { Value::Int64(x as i64) } else { Value::Uint64(x as u64) })
        }
        TypeData::Float(_) => {
            let x = match (v, s) {
                (Value::Double(d), _) => *d,
                (Value::Int64(x), _) => *x as f64,
                (Value::Uint64(x), _) => *x as f64,
                (_, Some(s)) => s.parse::<f64>().map_err(|_| TcError::InvalidValue(format!("invalid float '{}'", s)))?,
                _ => return invalid(format!("cannot convert {:?} to float", v)),
            };
            Ok(narrow_float(dtype.eng_value_type(), x))
        }
        TypeData::String(_) => match v {
            Value::StringValue(s) => Ok(Value::StringValue(s.clone())),
            _ => invalid(format!("cannot convert {:?} to string", v)),
        },
        TypeData::Binary(_) => match (v, s) {
            (Value::Binary(b), _) => Ok(Value::Binary(b.clone())),
            (_, Some(s)) => hex::decode(s)
                .map(|b| Value::Binary(Box::new(b)))
                .map_err(|_| TcError::InvalidValue(format!("invalid hexadecimal binary '{}'", s))),
            _ => invalid(format!("cannot convert {:?} to binary", v)),
        },
        TypeData::Boolean(bdt) => match (v, s) {
            (Value::Boolean(b), _) => Ok(Value::Boolean(*b)),
            (_, Some(s)) => {
                if s == bdt.one_string_value || s.eq_ignore_ascii_case("true") {
                    Ok(Value::Boolean(true))
                } else if s == bdt.zero_string_value || s.eq_ignore_ascii_case("false") {
                    Ok(Value::Boolean(false))
                } else {
                    invalid(format!(
                        "invalid boolean '{}'; expected {} or {}",
                        s, bdt.one_string_value, bdt.zero_string_value
                    ))
                }
            }
            _ => invalid(format!("cannot convert {:?} to boolean", v)),
        },
        TypeData::Enumerated(edt) => {
            let label = match (v, s) {
                (Value::Enumerated(ev), _) => ev.value.as_str(),
                (_, Some(s)) => s,
                _ => return invalid(format!("cannot convert {:?} to enumerated (a label is required)", v)),
            };
            enumerated(edt, label).map(|ev| Value::Enumerated(Box::new(ev)))
        }
        TypeData::AbsoluteTime(_) => match (v, s) {
            (Value::Timestamp(t), _) => Ok(Value::Timestamp(*t)),
            (_, Some(s)) => time::parse(s)
                .map(Value::Timestamp)
                .ok_or_else(|| TcError::InvalidValue(format!("invalid time '{}'", s))),
            _ => invalid(format!("cannot convert {:?} to time", v)),
        },
        TypeData::Aggregate(atype) => {
            if let Some(s) = s {
                return to_eng_value(mdb, dtype, &aggregate_from_json(mdb, atype, s)?);
            }
            let Value::Aggregate(av) = v else {
                return invalid(format!("cannot convert {:?} to aggregate", v));
            };
            let mut m = HashMap::new();
            for member in &atype.members {
                let mtype = mdb.get_data_type(member.dtype);
                let mv = match av.0.get(&member.name()) {
                    Some(mv) => to_eng_value(mdb, mtype, mv),
                    None => match member.initial_value.as_ref().or(mtype.initial_value.as_ref()) {
                        Some(iv) => to_eng_value(mdb, mtype, &Value::StringValue(Box::new(iv.clone()))),
                        None => invalid(format!("no value for the aggregate member {}", mdb.name2str(member.name()))),
                    },
                }
                .map_err(|e| e.in_member(mdb.name2str(member.name())))?;
                m.insert(member.name(), mv);
            }
            if let Some(k) = av.0.keys().find(|k| !atype.members.iter().any(|m| m.name() == **k)) {
                return invalid(format!("unknown aggregate member {}", mdb.name2str(*k)));
            }
            Ok(Value::Aggregate(Box::new(AggregateValue(m))))
        }
        TypeData::Array(atype) => {
            if let Some(s) = s {
                return to_eng_value(mdb, dtype, &array_from_json(atype, s)?);
            }
            let Value::Array(av) = v else {
                return invalid(format!("cannot convert {:?} to array", v));
            };
            // if the dimensions are fixed in the type, the value has to match them
            let fixed_dims: Option<Vec<usize>> = atype
                .dim
                .iter()
                .map(|d| match d {
                    IntegerValue::FixedValue(x) if *x >= 0 => Some(*x as usize),
                    _ => None,
                })
                .collect();
            let dims = match fixed_dims {
                Some(fd) if !fd.is_empty() => {
                    if fd.iter().product::<usize>() != av.values.len() {
                        return invalid(format!(
                            "the array has {} elements but the type requires dimensions {:?}",
                            av.values.len(),
                            fd
                        ));
                    }
                    fd
                }
                _ if av.dims.iter().product::<usize>() == av.values.len() => av.dims.clone(),
                _ => vec![av.values.len()],
            };
            let etype = mdb.get_data_type(atype.dtype);
            let values = av
                .values
                .iter()
                .enumerate()
                .map(|(i, x)| to_eng_value(mdb, etype, x).map_err(|e| e.in_member(&format!("[{}]", i))))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::Array(Box::new(ArrayValue { dims, values })))
        }
    }
}

/// returns the initial value of the type (for aggregates composed from the initial values of the members)
pub(crate) fn type_initial_value(mdb: &MissionDatabase, dtype: &DataType) -> Option<Result<Value>> {
    if let Some(iv) = &dtype.initial_value {
        return Some(to_eng_value(mdb, dtype, &Value::StringValue(Box::new(iv.clone()))));
    }
    if let TypeData::Aggregate(atype) = &dtype.type_data {
        let mut m = HashMap::new();
        for member in &atype.members {
            let mtype = mdb.get_data_type(member.dtype);
            let mv = match &member.initial_value {
                Some(iv) => to_eng_value(mdb, mtype, &Value::StringValue(Box::new(iv.clone()))),
                None => type_initial_value(mdb, mtype)?,
            };
            match mv {
                Ok(mv) => m.insert(member.name(), mv),
                Err(e) => return Some(Err(e.in_member(mdb.name2str(member.name())))),
            };
        }
        return Some(Ok(Value::Aggregate(Box::new(AggregateValue(m)))));
    }
    None
}

/// parses a decimal or hexadecimal (0x prefix) integer
fn parse_int(s: &str) -> Option<i128> {
    let (neg, t) = match s.strip_prefix('-') {
        Some(t) => (true, t),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let x = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => i128::from_str_radix(h, 16).ok()?,
        None => t.parse::<i128>().ok()?,
    };
    Some(if neg { -x } else { x })
}

fn enumerated(edt: &EnumeratedDataType, label: &str) -> Result<EnumeratedValue> {
    edt.enumeration
        .iter()
        .find(|e| e.label == label)
        .map(|e| EnumeratedValue { key: e.value, value: e.label.clone() })
        .ok_or_else(|| {
            let labels: Vec<&str> = edt.enumeration.iter().map(|e| e.label.as_str()).collect();
            TcError::InvalidValue(format!("invalid enumeration label '{}'; valid labels: {:?}", label, labels))
        })
}

/// verifies the engineering value (and for numeric types the raw value) against the valid ranges of the type
pub(crate) fn check_valid(mdb: &MissionDatabase, dtype: &DataType, eng: &Value, raw: &Value) -> Result<()> {
    match (&dtype.type_data, eng) {
        (TypeData::Integer(_) | TypeData::Float(_), _) => {
            if let Some(vr) = &dtype.valid_range {
                let v = if vr.applies_to_calibrated { eng } else { raw };
                if let Ok(x) = f64::try_from(v) {
                    if !vr.range.contains(x) {
                        return Err(TcError::OutOfRange(format!(
                            "{} value {} is outside of the valid range {}",
                            if vr.applies_to_calibrated { "engineering" } else { "raw" },
                            v,
                            range_to_string(&vr.range)
                        )));
                    }
                }
            }
        }
        (TypeData::String(sdt), Value::StringValue(s)) => {
            let n = s.chars().count() as u32;
            if sdt.min_chars.is_some_and(|m| n < m) || sdt.max_chars.is_some_and(|m| n > m) {
                return Err(TcError::OutOfRange(format!(
                    "string length {} outside of the allowed range [{}, {}]",
                    n,
                    sdt.min_chars.map_or("-".to_owned(), |x| x.to_string()),
                    sdt.max_chars.map_or("-".to_owned(), |x| x.to_string())
                )));
            }
        }
        (TypeData::Binary(bdt), Value::Binary(b)) => {
            let n = b.len() as u32;
            if bdt.min_bytes.is_some_and(|m| n < m) || bdt.max_bytes.is_some_and(|m| n > m) {
                return Err(TcError::OutOfRange(format!(
                    "binary size {} bytes outside of the allowed range [{}, {}]",
                    n,
                    bdt.min_bytes.map_or("-".to_owned(), |x| x.to_string()),
                    bdt.max_bytes.map_or("-".to_owned(), |x| x.to_string())
                )));
            }
        }
        (TypeData::Aggregate(atype), Value::Aggregate(ev)) => {
            let Value::Aggregate(rv) = raw else { return Ok(()) };
            for m in &atype.members {
                if let (Some(e), Some(r)) = (ev.0.get(&m.name()), rv.0.get(&m.name())) {
                    check_valid(mdb, mdb.get_data_type(m.dtype), e, r)
                        .map_err(|err| err.in_member(mdb.name2str(m.name())))?;
                }
            }
        }
        (TypeData::Array(atype), Value::Array(ev)) => {
            let Value::Array(rv) = raw else { return Ok(()) };
            let etype = mdb.get_data_type(atype.dtype);
            for (i, (e, r)) in ev.values.iter().zip(&rv.values).enumerate() {
                check_valid(mdb, etype, e, r).map_err(|err| err.in_member(&format!("[{}]", i)))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn range_to_string(r: &crate::mdb::types::NumericRange) -> String {
    let lo = match (r.min_inclusive, r.min_exclusive) {
        (Some(x), _) => format!("[{}", x),
        (None, Some(x)) => format!("({}", x),
        _ => "(-inf".to_owned(),
    };
    let hi = match (r.max_inclusive, r.max_exclusive) {
        (Some(x), _) => format!("{}]", x),
        (None, Some(x)) => format!("{})", x),
        _ => "+inf)".to_owned(),
    };
    format!("{}, {}", lo, hi)
}

/// converts the engineering value to raw value
pub(crate) fn decalibrate(mdb: &MissionDatabase, dtype: &DataType, eng: &Value) -> Result<Value> {
    let enc = &dtype.encoding;
    match (&dtype.type_data, eng) {
        (TypeData::Integer(_) | TypeData::Float(_), _) => {
            let x = match eng {
                Value::Int64(v) => *v as f64,
                Value::Uint64(v) => *v as f64,
                Value::Double(d) => *d,
                _ => return invalid(format!("unexpected numeric value {:?}", eng)),
            };
            match &dtype.calibrator {
                // like in Yamcs, the calibrator of an argument type converts the engineering value to raw
                Some(cal) => {
                    let y = crate::proc::calibrators::calibrate(cal, x)
                        .map_err(|e| TcError::InvalidValue(e.to_string()))?;
                    calibrated_raw(enc, y)
                }
                // integers are kept exact
                None => match eng {
                    Value::Int64(_) | Value::Uint64(_) => match enc {
                        DataEncoding::Float(_) => Ok(Value::Double(x)),
                        DataEncoding::String(_) => Ok(Value::StringValue(Box::new(eng.to_string()))),
                        _ => Ok(eng.clone()),
                    },
                    _ => numeric_raw(enc, x),
                },
            }
        }
        (TypeData::Enumerated(_), Value::Enumerated(ev)) => match enc {
            DataEncoding::Float(_) => Ok(Value::Double(ev.key as f64)),
            DataEncoding::String(_) => Ok(Value::StringValue(Box::new(ev.key.to_string()))),
            _ => Ok(Value::Int64(ev.key)),
        },
        (TypeData::Boolean(bdt), Value::Boolean(b)) => match enc {
            DataEncoding::Integer(_) => Ok(Value::Uint64(*b as u64)),
            DataEncoding::String(_) => Ok(Value::StringValue(Box::new(
                if *b { &bdt.one_string_value } else { &bdt.zero_string_value }.clone(),
            ))),
            _ => Ok(Value::Boolean(*b)),
        },
        (TypeData::String(_), Value::StringValue(_)) | (TypeData::Binary(_), Value::Binary(_)) => Ok(eng.clone()),
        (TypeData::AbsoluteTime(atype), Value::Timestamp(t)) => time_raw(atype, enc, *t),
        (TypeData::Aggregate(atype), Value::Aggregate(av)) => {
            let mut m = HashMap::new();
            for member in &atype.members {
                let mv = av.0.get(&member.name()).ok_or_else(|| {
                    TcError::InvalidValue(format!("no value for member {}", mdb.name2str(member.name())))
                })?;
                let rv = decalibrate(mdb, mdb.get_data_type(member.dtype), mv)
                    .map_err(|e| e.in_member(mdb.name2str(member.name())))?;
                m.insert(member.name(), rv);
            }
            Ok(Value::Aggregate(Box::new(AggregateValue(m))))
        }
        (TypeData::Array(atype), Value::Array(av)) => {
            let etype = mdb.get_data_type(atype.dtype);
            let values = av
                .values
                .iter()
                .enumerate()
                .map(|(i, x)| decalibrate(mdb, etype, x).map_err(|e| e.in_member(&format!("[{}]", i))))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::Array(Box::new(ArrayValue { dims: av.dims.clone(), values })))
        }
        _ => invalid(format!("unexpected value {:?} for type {:?}", eng, dtype.eng_value_type())),
    }
}

fn parse_json(s: &str) -> Result<Json> {
    json::parse(s).map_err(|e| TcError::InvalidValue(format!("cannot parse '{}' as JSON: {}", s, e)))
}

fn json_string(j: &Json) -> Value {
    Value::StringValue(Box::new(j.as_arg_string()))
}

/// converts a JSON object to an aggregate value whose members are strings to be converted according to the
/// member types (the JSON strings are used without quotes, the other JSON values as JSON text)
fn aggregate_from_json(mdb: &MissionDatabase, atype: &AggregateDataType, s: &str) -> Result<Value> {
    let Json::Object(members) = parse_json(s)? else {
        return invalid(format!("expected a JSON object for the aggregate value, got '{}'", s));
    };
    let mut m = HashMap::new();
    for (k, j) in &members {
        let Some(member) = atype.members.iter().find(|m| mdb.name2str(m.name()) == k) else {
            return invalid(format!("unknown aggregate member {}", k));
        };
        m.insert(member.name(), json_string(j));
    }
    Ok(Value::Aggregate(Box::new(AggregateValue(m))))
}

/// converts a JSON array (nested for multidimensional arrays) to an array of strings to be converted according
/// to the element type
fn array_from_json(atype: &ArrayDataType, s: &str) -> Result<Value> {
    let j = parse_json(s)?;
    let ndims = atype.number_of_dimensions.max(1);
    let mut dims = Vec::new();
    let mut level = vec![&j];
    for d in 0..ndims {
        let mut next = Vec::new();
        let mut size = None;
        for x in level {
            let Json::Array(a) = x else {
                return invalid(format!("expected a JSON array with {} dimensions, got '{}'", ndims, s));
            };
            if *size.get_or_insert(a.len()) != a.len() {
                return invalid(format!("the arrays of dimension {} have different sizes in '{}'", d, s));
            }
            next.extend(a.iter());
        }
        dims.push(size.unwrap_or(0));
        level = next;
    }
    let values = level.into_iter().map(json_string).collect();
    Ok(Value::Array(Box::new(ArrayValue { dims, values })))
}

/// converts a floating point engineering value (of a type without calibrator) to the raw value required by
/// the encoding. Like in Yamcs, the conversion to integer truncates the fractional part (Java (long) cast).
fn numeric_raw(enc: &DataEncoding, x: f64) -> Result<Value> {
    match enc {
        DataEncoding::Integer(ide) => {
            if !x.is_finite() {
                return invalid(format!("cannot encode {} as integer", x));
            }
            let v = x as i64;
            if ide.encoding == crate::mdb::types::IntegerEncodingType::Unsigned && v >= 0 {
                Ok(Value::Uint64(v as u64))
            } else {
                Ok(Value::Int64(v))
            }
        }
        DataEncoding::Float(_) => Ok(Value::Double(x)),
        DataEncoding::String(_) => Ok(Value::StringValue(Box::new(x.to_string()))),
        _ => invalid(format!("cannot encode a numeric value with {:?}", enc)),
    }
}

/// converts the output of the calibrator to the raw value type of the encoding like the Yamcs
/// NumericCalibratorProc: Math.round to integers, negative values become 0 for the unsigned encodings,
/// 32 bits int cast for the encodings up to 32 bits.
fn calibrated_raw(enc: &DataEncoding, y: f64) -> Result<Value> {
    match enc {
        DataEncoding::Integer(ide) => {
            let unsigned = ide.encoding == crate::mdb::types::IntegerEncodingType::Unsigned;
            let r = if unsigned && y < 0.0 { 0 } else { java_round(y) };
            Ok(match (ide.size_in_bits <= 32, unsigned) {
                (true, true) => Value::Uint64(r as i32 as u32 as u64),
                (true, false) => Value::Int64(r as i32 as i64),
                (false, true) => Value::Uint64(r as u64),
                (false, false) => Value::Int64(r),
            })
        }
        DataEncoding::Float(fde) => {
            Ok(Value::Double(if fde.size_in_bits <= 32 { y as f32 as f64 } else { y }))
        }
        DataEncoding::String(_) => Ok(Value::StringValue(Box::new(y.to_string()))),
        _ => invalid(format!("cannot encode a numeric value with {:?}", enc)),
    }
}

/// computes the raw value of an absolute time
fn time_raw(atype: &AbsoluteTimeDataType, enc: &DataEncoding, t: i64) -> Result<Value> {
    if let DataEncoding::String(_) = enc {
        return Ok(Value::StringValue(Box::new(time::to_string(t))));
    }
    let offset_ms = match &atype.reference {
        TimeReference::Epoch(epoch) => match epoch {
            TimeEpoch::Tai => t.wrapping_sub(time::TAI_EPOCH_YAMCS_EPOCH_DELTA),
            TimeEpoch::J2000 => t.wrapping_sub(time::J2000_EPOCH_YAMCS_EPOCH_DELTA),
            TimeEpoch::Unix => time::to_unix_millis(t),
            TimeEpoch::Gps => t.wrapping_sub(time::GPS_EPOCH_YAMCS_EPOCH_DELTA),
            TimeEpoch::DateTime(t0) => t.wrapping_sub(*t0),
        },
        TimeReference::OffsetFrom(_) => {
            return Err(TcError::Unsupported("time arguments relative to a parameter (OffsetFrom)".to_owned()))
        }
        TimeReference::None => return invalid("absolute time type without reference time".to_owned()),
    };
    // computed like in Yamcs (the raw value is a 64 bits integer for the integer encodings)
    let needs_scaling = atype.scale != 1.0 || atype.offset != 0.0;
    let scaled = || (offset_ms as f64 - 1000.0 * atype.offset) / (1000.0 * atype.scale);
    match enc {
        DataEncoding::Integer(_) => {
            let x = if needs_scaling { scaled() as i64 } else { offset_ms / 1000 };
            Ok(Value::Int64(x))
        }
        DataEncoding::Float(_) => Ok(Value::Double(if needs_scaling { scaled() } else { offset_ms as f64 / 1000.0 })),
        _ => invalid(format!("cannot encode an absolute time with {:?}", enc)),
    }
}
