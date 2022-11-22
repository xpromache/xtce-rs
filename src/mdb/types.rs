use std::fmt::{self, Formatter};

use smallvec::SmallVec;

use crate::{bitbuffer::ByteOrder, error::MdbError, value::ValueUnion};

use super::{DataTypeIdx, IntegerValue, NameDescription, NameIdx, NamedItem, UnitType, MissionDatabase};

#[derive(Debug)]
pub struct BinaryDataEncoding {}

#[derive(Debug)]
pub struct BooleanDataEncoding {}

#[derive(Debug)]
pub struct FloatDataEncoding {
    pub size_in_bits: u8,
    pub encoding: FloatEncodingType,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IntegerEncodingType {
    Unsigned,
    TwosComplement,
    SignMagnitude,
    OnesComplement,
}

#[derive(Debug, Copy, Clone)]
pub struct IntegerDataEncoding {
    pub size_in_bits: u8,
    pub encoding: IntegerEncodingType,
    pub byte_order: ByteOrder,
}

#[derive(Debug, Copy, Clone)]
pub enum FloatEncodingType {
    IEEE754_1985,
    Milstd1750a,
}

#[derive(Debug)]
pub enum StringSizeType {
    /**
     * fixed size has to be specified in the {@link #getSizeInBits}
     */
    Fixed,
    /**
     * Like C strings, they are terminated with a special string, usually a null character.
     */
    TerminationChar,
    /**
     * Like PASCAL strings, the size of the string is given as an integer at the start of the string. SizeTag must
     * be an unsigned Integer
     */
    LeadingSize,
    /**
     * {@link #getFromBinaryTransformAlgorithm} will be used to decode the data
     */
    Custom,
}

#[derive(Debug)]
pub struct StringDataEncoding {
    pub sizeType: StringSizeType,
    pub size_in_bits: u32,
    pub sizeInBitsOfSizeTag: u8,
    pub encoding: String,
    pub termination_char: u8,
}

#[derive(Debug)]
pub enum DataEncoding {
    None,
    Binary(BinaryDataEncoding),
    Boolean(BooleanDataEncoding),
    Float(FloatDataEncoding),
    Integer(IntegerDataEncoding),
    String(StringDataEncoding),
}

#[derive(Debug)]
pub struct NumericAlarm {}

#[derive(Debug)]
pub struct NumericContextAlarm {}

#[derive(Debug)]
pub struct EnumerationAlarm {}

#[derive(Debug)]
pub struct EnumerationContextAlarm {}

#[derive(Debug)]
pub struct BinaryDataType {
    pub size_in_bits: u32,
}

#[derive(Debug)]
pub enum Calibrator {}

pub struct ValueEnumeration {
    pub value: i64,
    /// If max value is given, the label maps to a range where value is less than or equal to maxValue.
    /// The range is inclusive.
    pub max_value: i64,
    pub label: String,
    pub description: Option<String>,
}

impl std::fmt::Debug for ValueEnumeration {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.value != self.max_value {
            write!(f, "[{}-{}]", self.value, self.max_value)?;
        } else {
            write!(f, "{}", self.value)?;
        }
        write!(f, ": {}", self.label)
    }
}

#[derive(Debug)]
pub struct DataType {
    pub ndescr: NameDescription,
    pub encoding: DataEncoding,
    pub type_data: TypeData,
    pub units: Vec<UnitType>,
    pub calibrator: Option<Calibrator>,
}

#[derive(Debug)]
pub enum TypeData {
    Integer(IntegerDataType),
    Float(FloatDataType),
    String(StringDataType),
    Binary(BinaryDataType),
    Boolean(BooleanDataType),
    Enumerated(EnumeratedDataType),
    Aggregate(AggregateDataType),
    Array(ArrayDataType),
    AbsoluteTime(AbsoluteTimeDataType),
}
impl NamedItem for DataType {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

impl DataType {
    /// Converts a string to a value corresponding to the given data type
    ///
    pub fn from_str(&self, value: &str, calibrated: bool) -> Result<ValueUnion<()>, MdbError> {
        if calibrated {
            match &self.type_data {
                TypeData::Integer(idt) => parse_integer(value, idt.signed, idt.size_in_bits),
                TypeData::Float(_) => todo!(),
                TypeData::String(_) => todo!(),
                TypeData::Binary(_) => todo!(),
                TypeData::Boolean(bdt) => parse_eng_boolean(value, bdt),
                TypeData::Enumerated(edt) => parse_eng_enumerated(value, edt),
                TypeData::Aggregate(_) => todo!(),
                TypeData::Array(_) => todo!(),
                TypeData::AbsoluteTime(_) => todo!(),
            }
        } else {
            match self.encoding {
                DataEncoding::Integer(ide) => parse_integer(
                    value,
                    ide.encoding != IntegerEncodingType::Unsigned,
                    ide.size_in_bits as u32,
                ),
                DataEncoding::Float(_) => todo!(),
                DataEncoding::Binary(_) => todo!(),
                DataEncoding::Boolean(_) => todo!(),
                DataEncoding::String(_) => todo!(),
                DataEncoding::None => todo!(),
            }
        }
    }
}

fn parse_integer(value: &str, signed: bool, size_in_bits: u32) -> Result<ValueUnion<()>, MdbError> {
    let x = value.parse::<i128>()?;
    let max = if signed { (1i128 << (size_in_bits - 1)) - 1 } else { (1i128 << size_in_bits) - 1 };
    let min = if signed { -(1i128 << (size_in_bits - 1)) } else { 0 };

    if x < min || x > max {
        return Err(MdbError::OutOfRange(format!(
            "Value {} out of range [{}, {}] required by the type",
            x, min, max
        )));
    };

    if signed {
        Ok(ValueUnion::Int64(x as i64))
    } else {
        Ok(ValueUnion::Uint64(x as u64))
    }
}

fn parse_eng_boolean(value: &str, bdt: &BooleanDataType) -> Result<ValueUnion<()>, MdbError> {
    if value == bdt.zero_string_value {
        Ok(ValueUnion::Boolean(false))
    } else if value == bdt.one_string_value {
        Ok(ValueUnion::Boolean(true))
    } else {
        Err(MdbError::InvalidValue(format!(
            "Invalid value '{}' for boolean type. Expected {} or {}",
            value, bdt.one_string_value, bdt.zero_string_value
        )))
    }
}

fn parse_eng_enumerated(value: &str, edt: &EnumeratedDataType) -> Result<ValueUnion<()>, MdbError> {
    edt.enumeration
        .iter()
        .find(|ev| ev.label == value)
        .map(|v| ValueUnion::StringValue(Box::new(v.label.clone())))
        .ok_or(MdbError::InvalidValue(format!("Value {} not valid for type", value)))
}

#[derive(Debug)]
pub struct AbsoluteTimeDataType {}

#[derive(Debug)]
pub struct EnumeratedDataType {
    pub enumeration: Vec<ValueEnumeration>,
    pub default_alarm: Option<EnumerationAlarm>,
    pub context_alarm: Vec<EnumerationContextAlarm>,
}

#[derive(Debug)]
pub struct FloatDataType {
    pub size_in_bits: u32,
    pub default_alarm: Option<NumericAlarm>,
    pub context_alarm: Vec<NumericContextAlarm>,
}

#[derive(Debug)]
pub struct IntegerDataType {
    pub size_in_bits: u32,
    pub signed: bool,
    pub default_alarm: Option<NumericAlarm>,
    pub context_alarm: Vec<NumericContextAlarm>,
}

impl IntegerDataType {
    fn max_value(&self) -> i128 {
        if self.signed {
            (1i128 << (self.size_in_bits - 1)) - 1
        } else {
            (1i128 << self.size_in_bits) - 1
        }
    }

    fn min_value(&self) -> i128 {
        if self.signed {
            -(1i128 << (self.size_in_bits - 1))
        } else {
            0
        }
    }
}

#[derive(Debug)]
pub struct StringDataType {}

#[derive(Debug)]
pub struct BooleanDataType {
    pub one_string_value: String,
    pub zero_string_value: String,
}

#[derive(Debug)]
pub struct AggregateDataType {
    pub members: Vec<Member>,
}

#[derive(Debug)]
pub struct Member {
    pub ndescr: NameDescription,
    pub dtype: DataTypeIdx,
}

impl NamedItem for Member {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

#[derive(Debug)]
pub struct ArrayDataType {
    pub dtype: DataTypeIdx,
    pub dim: Vec<IntegerValue>,
}

///
/// Describes an element from an aggregate/array member access path For example, the third element from this path :
/// a/c[2]/d[0][5]/x
/// is:
///  name = "d"
///  index = [0, 5]
///
/// name can be None and index can be empty
#[derive(Debug, Clone)]
pub struct PathElement {
    pub name: Option<NameIdx>,
    //SmallVec of size 4 will occupy on a 64 bits machine the same amont of memory (24 bytes) as an empty Vec.
    //The advantage is that it can store up to four u32s without any heap allocation.
    // Most references will be either unary arrays or zero size empty (i.e. paths like a.b[1].c[2] or a.b.c)
    pub index: SmallVec<[u32; 4]>,
}

impl PathElement {
   pub fn to_string(&self, mdb: &MissionDatabase) -> String{
        let mut r = String::new();
        if let Some(name) = self.name {
            r.push_str(mdb.name2str(name));
        }
        for idx in &self.index {
            r.push_str(&format!("[{}]", idx));
        }
        r
    }
}
pub type MemberPath = Vec<PathElement>;
