#![feature(test)]


use std::collections::HashMap;
use std::sync::Arc;

use std::fmt::Formatter;
use std::fmt;

use lasso::{ThreadedRodeo, Key};

pub(crate) type NameIdx = lasso::Spur;

#[derive(Clone, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct QualifiedName(Vec<NameIdx>);

impl QualifiedName {
    pub fn new(x: Vec<NameIdx>) -> Self {
        Self(x)
    }
    pub fn empty() -> Self {
        QualifiedName::new(vec![])
    }
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
    pub fn name(&self) -> Option<NameIdx> {
        self.0.last().copied()
    }
    pub fn push(&mut self, name: NameIdx) {
        self.0.push(name);
    }
    pub fn pop(&mut self) -> Option<NameIdx> {
        self.0.pop()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Enum)]
pub(crate) enum NameReferenceType {
    ParameterType,
    Parameter,
    SequenceContainer,
    Algorithm,
}

impl std::fmt::Debug for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for idx in &self.0 {
            write!(f, "/{:?}", unsafe { idx.into_usize() })?;
        }
        Ok(())
    }
}


#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct Index(std::num::NonZeroU32);

impl Index {
    pub fn new(index: usize) -> Self {
        Self(std::num::NonZeroU32::new(index as u32 + 1).unwrap())
    }
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

type SpaceSystemIdx = Index;
type ParameterTypeIdx = Index;

#[derive(Clone, Debug, Default)]
pub struct NameDescription {
    pub(crate) name: NameIdx,
    pub(crate) short_description: Option<String>,
    pub(crate) long_description: Option<String>,
}

impl NameDescription {
    pub fn new(name: NameIdx) -> NameDescription {
        NameDescription { name, short_description: None, long_description: None }
    }
}


pub enum DataSource {
    /**
     * used for data acquired from outside, parameters of this type cannot be changed
     */
    Telemetered,
    /**
     * parameters set by the algorithm manager
     */
    Derived,
    /**
     * constants in the XtceDb - cannot be changed
     */
    Constant,
    /**
     * software parameters maintained by Yamcs and that can be set by client
     */
    Local,
    /**
     * parameters giving internal yamcs state -created on the fly
     */
    System,
    /**
     * special parameters created on the fly and instantiated in the context of command verifiers
     */
    Command,
    /**
     * special parameters created on the fly and instantiated in the context of command verifiers
     */
    CommandHistory,
    /**
     * external parameters are like local parameters (can be set by the client) but maintained outside Yamcs.
     * These are project specific and require a <code>SoftwareParameterManager</code> to be defined in the Yamcs
     * processor configuration.
     *
     */
    External1,
    External2,
    External3,
}

#[derive(Debug)]
pub struct UnitType {
    pub description: Option<String>,
    pub power: f64,
    pub factor: String,
    pub unit: String,
}

#[derive(Debug)]
pub struct BinaryDataEncoding {}

#[derive(Debug)]
pub struct BooleanDataEncoding {}

#[derive(Debug)]
pub struct FloatDataEncoding {
    pub size_in_bits: u8,
    pub encoding: FloatEncodingType,
}

#[derive(Debug, Copy, Clone)]
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
    Custom
}

#[derive(Debug)]
pub struct StringDataEncoding {
    pub sizeType: StringSizeType,
    pub size_in_bits: u32,
    pub sizeInBitsOfSizeTag: u8,
    pub encoding: String,
    pub termination_char: u8
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
pub struct BinaryParameterType {
    name: NameDescription,
    size_in_bits: u32,
    encoding: DataEncoding,
}


pub struct EnumeratedValue {
    pub(crate) value: i64,
    pub(crate) max_value: i64,
    //equal to value if not configured
    pub(crate) label: String,
    pub(crate) description: Option<String>,
}

impl std::fmt::Debug for EnumeratedValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if (self.value != self.max_value) {
            write!(f, "[{}-{}]", self.value, self.max_value)?;
        } else {
            write!(f, "{}", self.value)?;
        }
        write!(f, ": {}", self.label)
    }
}

#[derive(Debug)]
pub struct EnumeratedParameterType {
    pub(crate) name: NameDescription,
    pub(crate) encoding: DataEncoding,
    pub(crate) enumeration: Vec<EnumeratedValue>,
    pub(crate) default_alarm: Option<EnumerationAlarm>,
    pub(crate) context_alarm: Vec<EnumerationContextAlarm>,
    pub(crate) units: Vec<UnitType>,
}

#[derive(Debug)]
pub struct FloatParameterType {
    pub(crate) name: NameDescription,
    pub(crate) size_in_bits: u32,
    pub(crate) encoding: DataEncoding,
    pub(crate) default_alarm: Option<NumericAlarm>,
    pub(crate) context_alarm: Vec<NumericContextAlarm>,
    pub(crate) units: Vec<UnitType>,
}

#[derive(Debug)]
pub struct IntegerParameterType {
    pub(crate) name: NameDescription,
    pub(crate) size_in_bits: u32,
    pub(crate) signed: bool,
    pub(crate) encoding: DataEncoding,
    pub(crate) default_alarm: Option<NumericAlarm>,
    pub(crate) context_alarm: Vec<NumericContextAlarm>,
    pub(crate) units: Vec<UnitType>,
}

#[derive(Debug)]
pub struct StringParameterType {
    pub(crate) name: NameDescription,
    pub(crate) encoding: DataEncoding,
}

#[derive(Debug)]
pub struct BooleanParameterType {
    pub(crate) name: NameDescription,
    pub(crate) encoding: DataEncoding,
    pub(crate) one_string_value: String,
    pub(crate) zero_string_value: String,
    pub(crate) units: Vec<UnitType>,
}

#[derive(Debug)]
pub enum ParameterType {
    None,
    Integer(IntegerParameterType),
    Float(FloatParameterType),
    String(StringParameterType),
    Binary(BinaryParameterType),
    Boolean(BooleanParameterType),
    Enumerated(EnumeratedParameterType),
}

impl ParameterType {
    fn name(&self) -> Option<NameIdx> {
        match self { //TODO make this smarter
            ParameterType::Integer(pt) => Some(pt.name.name),
            ParameterType::Float(pt) => Some(pt.name.name),
            ParameterType::String(pt) => Some(pt.name.name),
            ParameterType::Binary(pt) => Some(pt.name.name),
            ParameterType::Boolean(pt) => Some(pt.name.name),
            ParameterType::Enumerated(pt) => Some(pt.name.name),
            ParameterType::None => Option::None
        }
    }

    pub(crate) fn encoding(&self) -> &DataEncoding {
        match self { //TODO make this smarter
            ParameterType::Integer(pt) => &pt.encoding,
            ParameterType::Float(pt) => &pt.encoding,
            ParameterType::String(pt) => &pt.encoding,
            ParameterType::Binary(pt) => &pt.encoding,
            ParameterType::Boolean(pt) => &pt.encoding,
            ParameterType::Enumerated(pt) => &pt.encoding,
            ParameterType::None => &DataEncoding::None
        }
    }
}

pub struct Parameter {
    name: NameDescription,
    ptype: Arc<ParameterType>,
    data_source: DataSource,
}

pub struct SequenceContainer {}

pub struct SpaceSystem {
    id: SpaceSystemIdx,
    pub(crate) fqn: QualifiedName,
    pub(crate) name: NameDescription,
    pub(crate) parameters: HashMap<NameIdx, Parameter>,
    pub(crate) parameter_types: HashMap<NameIdx, ParameterTypeIdx>,
    pub(crate) sequence_containers: HashMap<NameIdx, SequenceContainer>,
}

impl SpaceSystem {
    pub fn new(id: SpaceSystemIdx, name: NameIdx, fqn: QualifiedName) -> SpaceSystem {
        SpaceSystem {
            id,
            name: NameDescription::new(name),
            fqn,
            parameters: HashMap::new(),
            parameter_types: HashMap::new(),
            sequence_containers: HashMap::new(),
        }
    }

    pub fn name(&self) -> NameIdx {
        self.name.name
    }
}



pub struct MissionDatabase {
    name_db: Arc<ThreadedRodeo::<NameIdx>>,
    pub(crate) space_systems: Vec<SpaceSystem>,
    space_systems_qn: HashMap<QualifiedName, SpaceSystemIdx>,
    parameter_types: Vec<ParameterType>,
}

impl MissionDatabase {
    pub fn new() -> Self {
        let mut mdb = MissionDatabase { name_db: Arc::new(ThreadedRodeo::<NameIdx>::new()), space_systems: Vec::new(), space_systems_qn: HashMap::new(), parameter_types: Vec::new() };
        //create the root space system - it has "" name and an empty qualified name
        let ss_idx = SpaceSystemIdx::new(0);
        let ss = SpaceSystem::new(ss_idx, mdb.name_db.get_or_intern(""), QualifiedName::empty());
        mdb.space_systems.push(ss);
        mdb.space_systems_qn.insert(QualifiedName::empty(), ss_idx);

        mdb
    }

    pub fn new_space_system(&mut self, fqn: QualifiedName) -> Result<SpaceSystemIdx, String> {
        let ss_id = SpaceSystemIdx::new(self.space_systems.len());

        if (self.space_systems_qn.contains_key(&fqn)) {
            return Err("A spacesystem with the given fqn already exists".to_string());
        }

        match fqn.name() {
            Some(name) => {
                let ss = SpaceSystem::new(ss_id, name, fqn.clone());
                self.space_systems.push(ss);
                self.space_systems_qn.insert(fqn, ss_id);
                Ok(ss_id)
            }
            None => Err("Empty names are not allowed".to_owned())
        }
    }

    pub fn add_parameter_type(&mut self, fqn: &QualifiedName, ptype: ParameterType) -> Option<ParameterTypeIdx> {
        let ptype_name = ptype.name()?;

        let idx = ParameterTypeIdx::new(self.parameter_types.len());
        self.parameter_types.push(ptype);

        let ss = self.get_space_system_mut(fqn).unwrap();
        ss.parameter_types.insert(ptype_name, idx);
        Some(idx)
    }

    pub(crate) fn resolve_reference(&self, tref: NameReferenceType, nref: &str, ss_qn: &QualifiedName) -> Option<NameIdx> {
        Option::None
    }

    pub fn get_space_system_mut(&mut self, fqn: &QualifiedName) -> Option<&mut SpaceSystem> {
        match self.space_systems_qn.get_mut(fqn) {
            None => None,
            Some(idx) => self.space_systems.get_mut(idx.index())
        }
    }

    pub fn get_parameter_type(&self, idx: ParameterTypeIdx) -> Option<&ParameterType> {
        self.parameter_types.get(idx.index())
    }

    pub fn qn_to_string(&self, fqn: &QualifiedName) -> String {
        let v = &fqn.0;
        if v.len() == 0 {
            String::from("/")
        } else {
            let mut r: String = String::new();
            for idx in v {
                r = r + "/" + match self.name_db.try_resolve(idx) {
                    Some(name) => name,
                    None => "[unknown]"
                }
            }
            r
        }
    }
    pub fn name_to_string(&self, idx: NameIdx) -> Option<&str> {
        self.name_db.try_resolve(&idx)
    }


    pub fn get_or_intern(&mut self, name_str: &str) -> NameIdx {
        self.name_db.get_or_intern(name_str)
    }
    pub fn name_db(&mut self) -> Arc<ThreadedRodeo::<NameIdx>> {
        Arc::clone(&self.name_db)
    }
}
