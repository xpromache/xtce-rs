use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
};

use hex::ToHex;

use crate::mdb::{MissionDatabase, NameIdx, NamedItem, ParameterIdx};

#[derive(Debug, Clone)]
pub struct ParameterValue {
    pub pidx: ParameterIdx,
    /// the raw value; None for the parameters which do not have one (e.g. computed by algorithms)
    pub raw_value: Option<Value>,
    pub eng_value: Value,
    /// generation time (instant, see [crate::time]); INVALID_INSTANT if not known
    pub generation_time: i64,
    /// acquisition (reception) time (instant); INVALID_INSTANT if not known
    pub acquisition_time: i64,
    pub status: ParameterStatus,
    /// result of the alarm checking; None if the parameter has no alarm defined
    pub monitoring_result: Option<MonitoringResult>,
    /// set if the value is outside an alarm range
    pub range_condition: Option<RangeCondition>,
}

impl ParameterValue {
    pub fn new(pidx: ParameterIdx, raw_value: Value, eng_value: Value) -> ParameterValue {
        Self::with_raw(pidx, Some(raw_value), eng_value)
    }

    pub fn with_raw(pidx: ParameterIdx, raw_value: Option<Value>, eng_value: Value) -> ParameterValue {
        ParameterValue {
            pidx,
            raw_value,
            eng_value,
            generation_time: crate::time::INVALID_INSTANT,
            acquisition_time: crate::time::INVALID_INSTANT,
            status: ParameterStatus::ACQUIRED,
            monitoring_result: None,
            range_condition: None,
        }
    }
}

/// Acquisition status as a bit field like in Yamcs; 0 means acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParameterStatus(pub u8);

impl ParameterStatus {
    pub const ACQUIRED: ParameterStatus = ParameterStatus(0);
    /// the value is outside the valid range or could not be computed
    pub const INVALID: ParameterStatus = ParameterStatus(1);
    pub const NOT_RECEIVED: ParameterStatus = ParameterStatus(2);
    pub const EXPIRED: ParameterStatus = ParameterStatus(4);

    pub fn is_invalid(&self) -> bool {
        self.0 & Self::INVALID.0 != 0
    }

    pub fn set_invalid(&mut self) {
        self.0 |= Self::INVALID.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MonitoringResult {
    InLimits,
    Watch,
    Warning,
    Distress,
    Critical,
    Severe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeCondition {
    Low,
    High,
}

/// The type of a value as in Yamcs.
///
/// The values are stored in 64 bits (see [Value]) but the conversions follow the Yamcs rules for the given type
/// (e.g. a SINT32 engineering value is obtained with a 32 bits cast).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Sint32,
    Uint32,
    Sint64,
    Uint64,
    Float,
    Double,
    Boolean,
    String,
    Binary,
    Enumerated,
    Timestamp,
    Aggregate,
    Array,
}

/// A parameter value.
///
/// Unlike the Java Yamcs, there are no separate variants for 32 bits integers or floats.
/// It simplifies the code and no extra space is consumed because the enum is taking 16 bytes anyway.
/// The Yamcs type of a value can be obtained from the data type (see `DataType::eng_value_type`)
/// and the conversions reproduce the Yamcs results (e.g. a FLOAT value is rounded to 32 bits precision).
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Int64(i64),
    Uint64(u64),
    Double(f64),
    Boolean(bool),
    //box larger than 8 bytes variants to limit the size of the Value to 16 bytes
    StringValue(Box<String>),
    Enumerated(Box<EnumeratedValue>),
    Binary(Box<Vec<u8>>),
    Aggregate(Box<AggregateValue>),
    Array(Box<ArrayValue>),
    /// instant: milliseconds since 1970-01-01T00:00:00 TAI (see [crate::time])
    Timestamp(i64),
}

/// Multidimensional array stored in row-major order (the last index varies fastest)
#[derive(Debug, PartialEq, Clone)]
pub struct ArrayValue {
    /// size of each dimension
    pub dims: Vec<usize>,
    pub values: Vec<Value>,
}

impl ArrayValue {
    /// returns the element at the given (multidimensional) index
    pub fn get(&self, index: &[u32]) -> Option<&Value> {
        if index.len() != self.dims.len() {
            return None;
        }
        let mut flat = 0usize;
        for (&i, &d) in index.iter().zip(&self.dims) {
            let i = i as usize;
            if i >= d {
                return None;
            }
            flat = flat * d + i;
        }
        self.values.get(flat)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumeratedValue {
    pub key: i64,
    pub value: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AggregateValue(pub HashMap<NameIdx, Value>);


impl Value {
    pub fn int_value(num_bits: usize, x: i64) -> Value {
        if num_bits >= 64 {
            return Value::Int64(x);
        }
        let max: i64 = (1 << (num_bits - 1)) - 1;
        let min: i64 = -max - 1;
        let mut y = x as i64;

        if y >= max {
            y = max
        }
        if y < min {
            y = min;
        }

        Value::Int64(y)
    }

    pub fn uint_value(num_bits: usize, x: u64) -> Value {
        if num_bits >= 64 {
            return Value::Uint64(x);
        }

        let max: u64 = (1 << num_bits) - 1;
        let mut y = x;
        if y > max {
            y = max
        }

        Value::Uint64(y)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int64(x) => write!(f, "{}", x),
            Value::Uint64(x) => write!(f, "{}", x),
            Value::Double(x) => write!(f, "{}", x),
            Value::Boolean(x) => write!(f, "{}", x),
            Value::StringValue(x) => write!(f, "{}", x),
            Value::Enumerated(x) => write!(f, "{}", x.value),
            Value::Binary(x) => write!(f, "{}", x.encode_hex::<String>()),
            Value::Aggregate(x) => write!(f, "{:?}", x),
            Value::Array(x) => {
                write!(f, "[")?;
                for (i, v) in x.values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Timestamp(t) => write!(f, "{}", crate::time::to_string(*t)),
        }        
    }
}

impl TryFrom<Value> for i64 {
    type Error = ();

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match &value {
            Value::Int64(x) => Ok(*x),
            _ => Err(())
        } 
    }
}


impl TryFrom<Value> for u64 {
    type Error = ();

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Uint64(x) => Ok(x),
            _ => Err(())
        } 
    }
}

impl TryFrom<&Value> for u64 {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Uint64(x) => Ok(*x),
            _ => Err(())
        } 
    }
}

impl TryFrom<&Value> for f64 {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Uint64(x) => Ok(*x as f64),
            Value::Int64(x) => Ok(*x as f64),
            Value::Double(x) => Ok(*x),
            _ => Err(())
        } 
    }
}

#[derive(Debug)]
pub struct ContainerPosition {
    // the start of the container in the packet in bytes
    // this is the start of the top container in the hierarchy
    // this means it is normally 0 unless we have container composition (not inheritance!) and then it is the
    // byte offset where the sub-container appears in the containing container
    pub start_offset: u32,
    // bit offset relative to the startOffset
    pub bit_offset: u32,
    pub bit_size: u32,

    //if the extraction corresponds to an aggregate, this contains the details for the members
    pub details: ContainerPositionDetails,
}

#[derive(Debug)]
pub enum ContainerPositionDetails {
    None,
    Aggregate(HashMap<NameIdx, ContainerPosition>),
    Array(Vec<ContainerPosition>),
}
pub struct ParameterValueDebug<'a> {
    pv: &'a ParameterValue,
    mdb: &'a MissionDatabase,
}

impl ParameterValue {
    pub fn dbg<'a>(&'a self, mdb: &'a MissionDatabase) -> ParameterValueDebug<'a> {
        ParameterValueDebug { pv: self, mdb: mdb }
    }
}

impl std::fmt::Debug for ParameterValueDebug<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mdb = self.mdb;
        let pv = self.pv;

        write!(f, "{} ", mdb.name2str(mdb.get_parameter(pv.pidx).name()))?;
        f.write_str("eng_value: {")?;
        write_value(f, mdb, &pv.eng_value)?;
        if let Some(raw) = &pv.raw_value {
            f.write_str("}, raw_value: {")?;
            write_value(f, mdb, raw)?;
        }
        f.write_str("}")?;

        Ok(())
    }
}

fn write_value(f: &mut Formatter<'_>, mdb: &MissionDatabase, v: &Value) -> fmt::Result {
    match &v {
        Value::Int64(v) => write!(f, "{}", v)?,
        Value::Uint64(v) => write!(f, "{}", v)?,
        Value::Double(v) => write!(f, "{}", v)?,
        Value::Boolean(v) => write!(f, "{}", v)?,
        Value::StringValue(v) => write!(f, "{}", v)?,
        Value::Enumerated(v) => write_enumerated(f, v)?,
        Value::Binary(v) => write!(f, "{}", v.encode_hex::<String>())?,
        Value::Aggregate(v) => write_aggregate(f, mdb, v)?,
        Value::Array(v) => {
            f.write_str("[")?;
            for (i, x) in v.values.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write_value(f, mdb, x)?;
            }
            f.write_str("]")?;
        }
        Value::Timestamp(t) => write!(f, "{}", crate::time::to_string(*t))?,
    }

    Ok(())
}

fn write_aggregate(
    f: &mut Formatter<'_>,
    mdb: &MissionDatabase,
    v: &AggregateValue,
) -> fmt::Result {
    f.write_str("{")?;
    let mut first = true;
    for (member_name, member_value) in &v.0 {
        if first {
            first = false;
        } else {
            write!(f, ", ")?;
        }
        write!(f, "{}: ", mdb.name2str(*member_name))?;
        write_value(f, mdb, &member_value)?;
    }
    f.write_str("}")?;

    Ok(())
}

fn write_enumerated(
    f: &mut Formatter<'_>,
    v: &EnumeratedValue,
) -> fmt::Result {
    write!(f, "{{{}={}}}", v.key, v.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value() {
        println!("size of Vec<u32>: {}", std::mem::size_of::<Vec<u32>>());
        println!("size of String: {}", std::mem::size_of::<String>());
        println!("size of Value: {}", std::mem::size_of::<Value>());
        println!("size of RawValue: {}", std::mem::size_of::<Value>());
        println!("size of ParameterValue: {}", std::mem::size_of::<ParameterValue>());
    }

    #[test]
    fn test_i64() {
        let x: i64 = 0x3FFF_FFFF_FFFF_FFFF;
        let max: i64 = (1 << 62) - 1;
        let min: i64 = -max - 1;
        println!("x: {:x} max: {:x} min: {:x}", x, max, min);
    }
}
