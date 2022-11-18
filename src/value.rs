use core::num;
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
};

use hex::ToHex;

use crate::mdb::{MissionDatabase, NameIdx, NamedItem, ParameterIdx};

#[derive(Debug)]
pub struct ParameterValue {
    pub pidx: ParameterIdx,
    pub raw_value: RawValue,
    pub eng_value: EngValue,
}


pub type RawValue = Value<ContainerPosition>;
pub type EngValue = Value<()>;


#[derive(Debug)]
pub struct Value<T> {
    pub v: ValueUnion<T>,
    pub extra: T,
}

/// Unlike the Java Yamcs, we do not support the 32 bits integers or floats.
/// It simplifies the code and no extra space is consumed becuase the enum is taking 16 bytes anyway.
/// Note that the integer parameter extraction will shrink the numbers to fit into the size in bits specified in the type
#[derive(Debug)]
pub enum ValueUnion<T> {
    Int64(i64),
    Uint64(u64),
    Double(f64),
    Boolean(bool),
    //box larger than 8 bytes variants to limit the size of the Value to 16 bytes
    StringValue(Box<String>),
    Enumerated(Box<EnumeratedValue>),
    Binary(Box<Vec<u8>>),
    Aggregate(Box<AggregateValue<T>>),
}

#[derive(Debug)]
pub struct EnumeratedValue {
    pub key: i64,
    pub value: String,
}

#[derive(Debug)]
pub struct AggregateValue<T>(pub HashMap<NameIdx, Value<T>>);


impl<T> ValueUnion<T> {
    pub fn int_value(num_bits: usize, x: i64) -> ValueUnion<T> {
        if num_bits >= 64 {
            return ValueUnion::Int64(x);
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

        ValueUnion::Int64(y)
    }

    pub fn uint_value(num_bits: usize, x: u64) -> ValueUnion<T> {
        if num_bits >= 64 {
            return ValueUnion::Uint64(x);
        }

        let max: u64 = (1 << num_bits) - 1;
        let mut y = x;
        if y > max {
            y = max
        }

        ValueUnion::Uint64(y)
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
        f.write_str("}, raw_value: {")?;
        
        write_value(f, mdb, &pv.eng_value)?;
        f.write_str("}")?;

        Ok(())
    }
}

fn write_value<T>(f: &mut Formatter<'_>, mdb: &MissionDatabase, v: &Value<T>) -> fmt::Result {
    match &v.v {
        ValueUnion::Int64(v) => write!(f, "{}", v)?,
        ValueUnion::Uint64(v) => write!(f, "{}", v)?,
        ValueUnion::Double(v) => write!(f, "{}", v)?,
        ValueUnion::Boolean(v) => write!(f, "{}", v)?,
        ValueUnion::StringValue(v) => write!(f, "{}", v)?,
        ValueUnion::Enumerated(v) => write_enumerated(f, mdb, v)?,
        ValueUnion::Binary(v) => write!(f, "{}", v.encode_hex::<String>())?,
        ValueUnion::Aggregate(v) => write_aggregate(f, mdb, v)?,
    }

    Ok(())
}

fn write_aggregate<T>(
    f: &mut Formatter<'_>,
    mdb: &MissionDatabase,
    v: &AggregateValue<T>,
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
    mdb: &MissionDatabase,
    v: &EnumeratedValue,
) -> fmt::Result {
    write!(f, "{{{}={}}}", v.key, v.value)
}

#[cfg(test)]
mod tests {

    use super::*;

    struct Bum {
        idx: u32,
        pv: ParameterValue,
    }
    #[test]
    fn test_value() {
        println!("size of Vec<u32>: {}", std::mem::size_of::<Vec<u32>>());
        println!("size of String: {}", std::mem::size_of::<String>());
        println!("size of Value: {}", std::mem::size_of::<EngValue>());
        println!("size of RawValue: {}", std::mem::size_of::<RawValue>());
        println!("size of Bum: {}", std::mem::size_of::<Bum>());
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
