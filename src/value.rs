use std::collections::HashMap;

use crate::mdb::ParameterIdx;

pub struct ParameterValue {
    pub pidx: ParameterIdx,
    pub raw_value: RawValue,
    pub eng_value: Value,
}

pub struct EnumeratedValue {
    m: Box<HashMap<i64, String>>,
}
pub enum Value {
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    Float(f32),
    Double(f64),
    StringValue(Box<String>),
    Enumerated(EnumeratedValue),
    Binary(),
}

pub struct RawValue {
    v: Value,

    // the start of the container in the packet in bytes
    // this is the start of the top container in the hierarchy
    // this means it is normally 0 unless we have container composition (not inheritance!) and then it is the
    // byte offset where the sub-container appears in the containing container
    start_offset: u32,
    // bit offset relative to the startOffset
    bit_offset: u32,
    bit_size: u32
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_value() {
        println!("size of Vec<u32>:{}", std::mem::size_of::<Vec<u32>>());
        println!("size of String:{}", std::mem::size_of::<String>());
        println!("size of value:{}", std::mem::size_of::<Value>());

        println!("size of ParameterValue:{}", std::mem::size_of::<ParameterValue>());

    }
}