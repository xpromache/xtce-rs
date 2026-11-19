use std::fmt::{self, Formatter};

use smallvec::SmallVec;

use crate::{
    bitbuffer::ByteOrder,
    value::{Value, ValueType},
};

use super::{
    DataTypeIdx, DynamicValueType, IntegerValue, MatchCriteriaIdx, MissionDatabase,
    NameDescription, NameIdx, NamedItem, ParameterInstanceRef, UnitType, Result, MdbError,
};


#[derive(Debug, Clone)]
pub struct DataType {
    pub ndescr: NameDescription,
    pub encoding: DataEncoding,
    pub type_data: TypeData,
    pub units: Vec<UnitType>,
    /// default calibrator, used if none of the context calibrators match
    pub calibrator: Option<Calibrator>,
    /// calibrators applied if their context matches; the first matching one is used
    pub context_calibrators: Vec<ContextCalibrator>,
    pub valid_range: Option<ValidRange>,
    /// initial (default) engineering value as string; used for the command arguments
    pub initial_value: Option<String>,
}

impl DataType {
    /// the type of the engineering values as in Yamcs
    pub fn eng_value_type(&self) -> ValueType {
        match &self.type_data {
            TypeData::Integer(idt) => match (idt.size_in_bits <= 32, idt.signed) {
                (true, true) => ValueType::Sint32,
                (true, false) => ValueType::Uint32,
                (false, true) => ValueType::Sint64,
                (false, false) => ValueType::Uint64,
            },
            TypeData::Float(fdt) => {
                if fdt.size_in_bits == 32 {
                    ValueType::Float
                } else {
                    ValueType::Double
                }
            }
            TypeData::String(_) => ValueType::String,
            TypeData::Binary(_) => ValueType::Binary,
            TypeData::Boolean(_) => ValueType::Boolean,
            TypeData::Enumerated(_) => ValueType::Enumerated,
            TypeData::Aggregate(_) => ValueType::Aggregate,
            TypeData::Array(_) => ValueType::Array,
            TypeData::AbsoluteTime(_) => ValueType::Timestamp,
        }
    }

    pub fn new(ndescr: NameDescription, type_data: TypeData) -> DataType {
        DataType {
            ndescr,
            encoding: DataEncoding::None,
            type_data,
            units: Vec::new(),
            calibrator: None,
            context_calibrators: Vec::new(),
            valid_range: None,
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone)]
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


impl DataEncoding {
    /// the type of the raw values as in Yamcs; None for types without encoding (aggregates, arrays)
    pub fn raw_value_type(&self) -> Option<ValueType> {
        match self {
            DataEncoding::None => None,
            DataEncoding::Integer(ide) => Some(match (ide.size_in_bits <= 32, ide.encoding) {
                (true, IntegerEncodingType::Unsigned) => ValueType::Uint32,
                (true, _) => ValueType::Sint32,
                (false, IntegerEncodingType::Unsigned) => ValueType::Uint64,
                (false, _) => ValueType::Sint64,
            }),
            DataEncoding::Float(fde) => {
                Some(if fde.size_in_bits <= 32 { ValueType::Float } else { ValueType::Double })
            }
            DataEncoding::Boolean(_) => Some(ValueType::Boolean),
            DataEncoding::Binary(_) => Some(ValueType::Binary),
            DataEncoding::String(_) => Some(ValueType::String),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DataEncoding {
    None,
    Binary(BinaryDataEncoding),
    Boolean(BooleanDataEncoding),
    Float(FloatDataEncoding),
    Integer(IntegerDataEncoding),
    String(StringDataEncoding),
}


#[derive(Debug, Clone)]
pub struct BooleanDataEncoding {
    pub size_in_bits: BinarySize,
}

#[derive(Debug, Clone)]
pub struct FloatDataEncoding {
    pub size_in_bits: u8,
    pub encoding: FloatEncodingType,
    pub byte_order: ByteOrder,
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

#[derive(Debug, Clone)]
pub enum StringSize {
    ///
    /// fixed size in bits    
    Fixed(u32),
    ///
    /// Like C strings, they are terminated with a special string, usually a null character.
    TerminationChar(u8),
    ///
    /// Like PASCAL strings, the size of the string is given as an integer at the start of the string. SizeTag must
    /// be an unsigned Integer.
    /// The tag size is given in bytes (not bits!)
    LeadingSize(u32),
    ///
    ///  an alogirthm will be used to decode the data - not yet supported
    ///
    Custom,
}

#[derive(Debug, Clone)]
pub struct StringDataEncoding {
    pub size_in_bits: StringSize,
    pub box_size_in_bits: StringBoxSize,
    pub encoding: String,
    pub max_box_size_in_bytes: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum StringBoxSize {
    Undefined,
    Fixed(u32),
    Dynamic(DynamicValueType),
}


#[derive(Debug, Clone)]
pub struct BinaryDataEncoding {
    pub size_in_bits: BinarySize
}

#[derive(Debug, Clone)]
pub enum BinarySize {
    /// fixed size in bits
    Fixed(u32),
    /// the size in bytes is given by an unsigned integer (big endian) preceding the data;
    /// the value is the size in bits of the size tag
    LeadingSize(u32),
    /// the size in bits is given by the value of another parameter
    Dynamic(DynamicValueType),
    /// size determined by a custom algorithm (not supported for processing); contains the algorithm text
    Custom(String),
}



/// A numeric range; all limits are optional
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NumericRange {
    pub min_inclusive: Option<f64>,
    pub min_exclusive: Option<f64>,
    pub max_inclusive: Option<f64>,
    pub max_exclusive: Option<f64>,
}

impl NumericRange {
    /// returns true if the value is within the range
    pub fn contains(&self, v: f64) -> bool {
        self.min_inclusive.is_none_or(|m| v >= m)
            && self.min_exclusive.is_none_or(|m| v > m)
            && self.max_inclusive.is_none_or(|m| v <= m)
            && self.max_exclusive.is_none_or(|m| v < m)
    }
}

#[derive(Debug, Clone)]
pub struct ValidRange {
    pub range: NumericRange,
    /// if true, the range applies to the engineering value, otherwise to the raw value
    pub applies_to_calibrated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlarmLevel {
    Normal,
    Watch,
    Warning,
    Distress,
    Critical,
    Severe,
}

/// Static alarm ranges: a value outside of the range triggers the corresponding alarm
#[derive(Debug, Clone, Default)]
pub struct AlarmRanges {
    pub watch: Option<NumericRange>,
    pub warning: Option<NumericRange>,
    pub distress: Option<NumericRange>,
    pub critical: Option<NumericRange>,
    pub severe: Option<NumericRange>,
}

#[derive(Debug, Clone)]
pub struct NumericAlarm {
    /// number of successive values that have to be out of limits before the alarm is triggered
    pub min_violations: u32,
    pub static_alarm_ranges: AlarmRanges,
}

#[derive(Debug, Clone)]
pub struct NumericContextAlarm {
    pub context: MatchCriteriaIdx,
    pub alarm: NumericAlarm,
}

#[derive(Debug, Clone)]
pub struct EnumerationAlarm {
    pub min_violations: u32,
    /// the alarm level for the labels not explicitly listed
    pub default_level: AlarmLevel,
    /// (label, level)
    pub alarms: Vec<(String, AlarmLevel)>,
}

#[derive(Debug, Clone)]
pub struct EnumerationContextAlarm {
    pub context: MatchCriteriaIdx,
    pub alarm: EnumerationAlarm,
}

#[derive(Debug, Clone)]
pub struct BinaryDataType {
    pub size_in_bits: u32,
    /// allowed size of the value in bytes (SizeRangeInBytes), used for command arguments
    pub min_bytes: Option<u32>,
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum Calibrator {
    /// polynomial calibrator; the coefficient at index i corresponds to x^i
    Polynomial(Vec<f64>),
    Spline(SplineCalibrator),
    /// math operation in reverse polish notation
    MathOperation(Vec<MathOpElement>),
    /// calibrator not supported (e.g. Java expression); contains a description.
    /// The parameters using it cannot be calibrated.
    Unsupported(String),
}

#[derive(Debug, Clone)]
pub struct ContextCalibrator {
    pub context: MatchCriteriaIdx,
    pub calibrator: Calibrator,
}

#[derive(Debug, Clone)]
pub struct SplineCalibrator {
    /// the order and extrapolate are read from the XTCE but like in Yamcs,
    /// the calibration is always linear with extrapolation outside the points
    pub order: u32,
    pub extrapolate: bool,
    /// sorted by raw value, at least two points
    pub points: Vec<SplinePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplinePoint {
    pub raw: f64,
    pub calibrated: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MathOpElement {
    Value(f64),
    ThisParameter,
    /// input of a math algorithm (index in the algorithm input list)
    Input(usize),
    Operator(MathOperator),
}

/// Operators supported in math operations (same set as Yamcs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathOperator {
    Plus,
    Minus,
    Star,
    Div,
    Modulo,
    Pow,
    RevPow,
    Ln,
    Log,
    Exp,
    Inv,
    Fact,
    Tan,
    Cos,
    Sin,
    Atan,
    Acos,
    Asin,
    Tanh,
    Cosh,
    Sinh,
    Atanh,
    Acosh,
    Asinh,
    Abs,
    LeftShift,
    RightShift,
    BitwiseOr,
    BitwiseAnd,
    Swap,
    Drop,
    Dup,
    Over,
}

impl MathOperator {
    pub fn from_xtce_name(s: &str) -> Option<MathOperator> {
        use MathOperator::*;
        Some(match s {
            "+" => Plus,
            "-" => Minus,
            "*" => Star,
            "/" => Div,
            "%" => Modulo,
            "^" => Pow,
            "y^x" => RevPow,
            "ln" => Ln,
            "log" => Log,
            "e^x" => Exp,
            "1/x" => Inv,
            "x!" => Fact,
            "tan" => Tan,
            "cos" => Cos,
            "sin" => Sin,
            "atan" => Atan,
            "acos" => Acos,
            "asin" => Asin,
            "tanh" => Tanh,
            "cosh" => Cosh,
            "sinh" => Sinh,
            "atanh" => Atanh,
            "acosh" => Acosh,
            "asinh" => Asinh,
            "abs" => Abs,
            "<<" => LeftShift,
            ">>" => RightShift,
            "|" => BitwiseOr,
            "&" => BitwiseAnd,
            "swap" => Swap,
            "drop" => Drop,
            "dup" => Dup,
            "over" => Over,
            _ => return None,
        })
    }

    /// the name of the operator in XTCE (inverse of [MathOperator::from_xtce_name])
    pub fn xtce_name(&self) -> &'static str {
        use MathOperator::*;
        match self {
            Plus => "+",
            Minus => "-",
            Star => "*",
            Div => "/",
            Modulo => "%",
            Pow => "^",
            RevPow => "y^x",
            Ln => "ln",
            Log => "log",
            Exp => "e^x",
            Inv => "1/x",
            Fact => "x!",
            Tan => "tan",
            Cos => "cos",
            Sin => "sin",
            Atan => "atan",
            Acos => "acos",
            Asin => "asin",
            Tanh => "tanh",
            Cosh => "cosh",
            Sinh => "sinh",
            Atanh => "atanh",
            Acosh => "acosh",
            Asinh => "asinh",
            Abs => "abs",
            LeftShift => "<<",
            RightShift => ">>",
            BitwiseOr => "|",
            BitwiseAnd => "&",
            Swap => "swap",
            Drop => "drop",
            Dup => "dup",
            Over => "over",
        }
    }

    /// returns (number of stack elements consumed, number of stack elements produced)
    pub fn stack_effect(&self) -> (usize, usize) {
        use MathOperator::*;
        match self {
            Swap => (2, 2),
            Drop => (1, 0),
            Dup => (1, 2),
            Over => (2, 3),
            Plus | Minus | Star | Div | Modulo | Pow | RevPow | LeftShift | RightShift
            | BitwiseOr | BitwiseAnd => (2, 1),
            _ => (1, 1),
        }
    }
}

#[derive(Clone)]
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

impl NamedItem for DataType {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

impl DataType {
    /// Converts a string to a value corresponding to the given data type
    ///
    pub fn from_str(&self, value: &str, calibrated: bool) -> Result<Value> {
        if calibrated {
            match &self.type_data {
                TypeData::Integer(idt) => parse_integer(value, idt.signed, idt.size_in_bits),
                TypeData::Float(_) => parse_float(value),
                TypeData::String(_) => Ok(Value::StringValue(Box::new(value.to_owned()))),
                TypeData::Binary(_) => parse_binary(value),
                TypeData::Boolean(bdt) => parse_eng_boolean(value, bdt),
                TypeData::Enumerated(edt) => parse_eng_enumerated(value, edt),
                TypeData::Aggregate(_) | TypeData::Array(_) | TypeData::AbsoluteTime(_) => {
                    Err(MdbError::InvalidValue(format!(
                        "Conversion from string to {:?} is not supported",
                        self.type_data
                    )))
                }
            }
        } else {
            match self.encoding {
                DataEncoding::Integer(ide) => parse_integer(
                    value,
                    ide.encoding != IntegerEncodingType::Unsigned,
                    ide.size_in_bits as u32,
                ),
                DataEncoding::Float(_) => parse_float(value),
                DataEncoding::Binary(_) => parse_binary(value),
                DataEncoding::Boolean(_) => match value.trim().to_lowercase().as_str() {
                    "true" | "1" => Ok(Value::Boolean(true)),
                    "false" | "0" => Ok(Value::Boolean(false)),
                    _ => Err(MdbError::InvalidValue(format!("Invalid boolean value '{}'", value))),
                },
                DataEncoding::String(_) => Ok(Value::StringValue(Box::new(value.to_owned()))),
                DataEncoding::None => Err(MdbError::InvalidValue(
                    "Cannot convert a raw value for a type without encoding".to_owned(),
                )),
            }
        }
    }
}

fn parse_float(value: &str) -> Result<Value> {
    value
        .trim()
        .parse::<f64>()
        .map(Value::Double)
        .map_err(|e| MdbError::InvalidValue(format!("Invalid float value '{}': {}", value, e)))
}

/// Hexadecimal to bytes like Yamcs (HexUtils.unhex): an odd number of digits gets a leading 0 ("0" is 0x00)
pub fn unhex(s: &str) -> std::result::Result<Vec<u8>, hex::FromHexError> {
    let s = s.trim();
    if s.len() % 2 == 1 {
        hex::decode(format!("0{s}"))
    } else {
        hex::decode(s)
    }
}

fn parse_binary(value: &str) -> Result<Value> {
    unhex(value)
        .map(|b| Value::Binary(Box::new(b)))
        .map_err(|e| MdbError::InvalidValue(format!("Invalid hex binary value '{}': {}", value, e)))
}

fn parse_integer(value: &str, signed: bool, size_in_bits: u32) -> Result<Value> {
    if size_in_bits == 0 || size_in_bits > 64 {
        return Err(MdbError::InvalidValue(format!("Invalid integer size in bits {}", size_in_bits)));
    }
    let x = value.trim().parse::<i128>()?;
    let max = if signed { (1i128 << (size_in_bits - 1)) - 1 } else { (1i128 << size_in_bits) - 1 };
    let min = if signed { -(1i128 << (size_in_bits - 1)) } else { 0 };

    if x < min || x > max {
        return Err(MdbError::OutOfRange(format!(
            "Value {} out of range [{}, {}] required by the type",
            x, min, max
        )));
    };

    if signed {
        Ok(Value::Int64(x as i64))
    } else {
        Ok(Value::Uint64(x as u64))
    }
}

fn parse_eng_boolean(value: &str, bdt: &BooleanDataType) -> Result<Value> {
    if value == bdt.zero_string_value {
        Ok(Value::Boolean(false))
    } else if value == bdt.one_string_value {
        Ok(Value::Boolean(true))
    } else {
        Err(MdbError::InvalidValue(format!(
            "Invalid value '{}' for boolean type. Expected {} or {}",
            value, bdt.one_string_value, bdt.zero_string_value
        )))
    }
}

fn parse_eng_enumerated(value: &str, edt: &EnumeratedDataType) -> Result<Value> {
    edt.enumeration
        .iter()
        .find(|ev| ev.label == value)
        .map(|v| Value::StringValue(Box::new(v.label.clone())))
        .ok_or_else(|| MdbError::InvalidValue(format!("Value {} not valid for type", value)))
}

#[derive(Debug, Clone)]
pub struct AbsoluteTimeDataType {
    /// the raw value is multiplied by scale and the offset is added to obtain the number of seconds since the reference
    pub scale: f64,
    pub offset: f64,
    pub reference: TimeReference,
}

#[derive(Debug, Clone)]
pub enum TimeReference {
    None,
    Epoch(TimeEpoch),
    /// the time is an offset from the value of another (absolute time) parameter
    OffsetFrom(ParameterInstanceRef),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeEpoch {
    /// 1958-01-01T00:00:00 TAI
    Tai,
    /// 2000-01-01T11:58:55.816 UTC
    J2000,
    /// 1970-01-01T00:00:00 UTC
    Unix,
    /// 1980-01-06T00:00:00 UTC
    Gps,
    /// an explicit date or date time (xs:date or xs:dateTime), converted to instant
    DateTime(i64),
}

#[derive(Debug, Clone)]
pub struct EnumeratedDataType {
    pub enumeration: Vec<ValueEnumeration>,
    pub default_alarm: Option<EnumerationAlarm>,
    pub context_alarm: Vec<EnumerationContextAlarm>,
}

#[derive(Debug, Clone)]
pub struct FloatDataType {
    pub size_in_bits: u32,
    pub default_alarm: Option<NumericAlarm>,
    pub context_alarm: Vec<NumericContextAlarm>,
}

#[derive(Debug, Clone)]
pub struct IntegerDataType {
    pub size_in_bits: u32,
    pub signed: bool,
    pub default_alarm: Option<NumericAlarm>,
    pub context_alarm: Vec<NumericContextAlarm>,
}

#[derive(Debug, Clone)]
pub struct StringDataType {
    /// allowed length of the string in characters (SizeRangeInCharacters), used for command arguments
    pub min_chars: Option<u32>,
    pub max_chars: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct BooleanDataType {
    pub one_string_value: String,
    pub zero_string_value: String,
}

#[derive(Debug, Clone)]
pub struct AggregateDataType {
    pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct Member {
    pub ndescr: NameDescription,
    pub dtype: DataTypeIdx,
    /// initial value (used for command arguments); overrides the initial value of the member type
    pub initial_value: Option<String>,
}

impl NamedItem for Member {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

#[derive(Debug, Clone)]
pub struct ArrayDataType {
    pub dtype: DataTypeIdx,
    pub number_of_dimensions: usize,
    /// the size of each dimension; empty if the sizes are not specified in the type
    /// (and have to be specified in the ArrayParameterRefEntry)
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
    pub fn to_string(&self, mdb: &MissionDatabase) -> String {
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

#[cfg(test)]
mod unhex_tests {
    #[test]
    fn unhex() {
        assert_eq!(vec![0], super::unhex("0").unwrap());
        assert_eq!(vec![0x0a, 0xbc], super::unhex(" abc ").unwrap());
        assert_eq!(vec![0x12], super::unhex("12").unwrap());
        assert!(super::unhex("x").is_err());
    }
}
