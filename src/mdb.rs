use std::sync::Arc;
use std::{collections::HashMap, fmt::Debug};

use std::fmt;
use std::fmt::Formatter;

use lasso::{Key, ThreadedRodeo};

use crate::value::ValueUnion;
use crate::{
    bitbuffer::ByteOrder,
};

pub(crate) type NameIdx = lasso::Spur;

pub type NameDb = Arc<ThreadedRodeo<NameIdx>>;

pub type SpaceSystemIdx = Index;
pub type DataTypeIdx = Index;
pub type ParameterIdx = Index;
pub type ContainerIdx = Index;
pub type MatchCriteriaIdx = Index;

/// The Mission Database contains all Parameters, Parameter Types, Containers, etc.
/// Unlike the Java version, because Rust doesn't like items pointing to randomly at eachother,
/// we have them all stored in vectors at the top of this structure.
/// The definition of each item uses then indices in these vectors 
/// (e.g. a parameter definition contains the index of its parameter type in the parameter_types vector).
/// 
/// Similaryly for names - we use some numeric identifiers for each name and the String has to be retrieved from the NameDb
pub struct MissionDatabase {
    name_db: NameDb,
    pub space_systems: Vec<SpaceSystem>,
    /// qualified space system names
    /// to lookup an item (parameter, type, etc) by fully qualified name, 
    /// the space system is taken from the map and then in the space system there is a map with all the items 
    space_systems_qn: HashMap<QualifiedName, SpaceSystemIdx>,

    /// vectors with definitions
    pub parameter_types: Vec<DataType>,
    pub parameters: Vec<Parameter>,
    pub containers: Vec<SequenceContainer>,
    pub match_criteria: Vec<MatchCriteria>,
}

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

    /// return the last part of the qualified name
    /// for the root / it returns None
    pub fn name(&self) -> Option<NameIdx> {
        self.0.last().copied()
    }

    /// return the qualified name without the last component
    pub fn parent(self) -> QualifiedName {
        let mut v = self.0.clone();
        v.pop();
        QualifiedName(v)
    }

    pub fn push(&mut self, name: NameIdx) {
        self.0.push(name);
    }
    pub fn pop(&mut self) -> Option<NameIdx> {
        self.0.pop()
    }

    pub fn to_string(&self, name_db: &NameDb) -> String {
        let v = &self.0;
        if v.len() == 0 {
            String::from("/")
        } else {
            let mut r: String = String::new();
            for idx in v {
                r = r
                    + "/"
                    + match name_db.try_resolve(idx) {
                        Some(name) => name,
                        None => "[unknown]",
                    }
            }
            r
        }
    }

    /// get a qualified name from a string.
    /// It splits the name by "/" separator - if any part is not found in the NameDb, None is returned
    ///
    pub fn from_str(name_db: &NameDb, qnstr: &str) -> Option<QualifiedName> {
        let mut qn = QualifiedName::empty();

        for p in qnstr.split("/") {
            if !p.is_empty() {
                if let Some(idx) = name_db.get(p) {
                    qn.push(idx);
                } else {
                    return None;
                }
            }
        }

        Some(qn)
    }

    /// parse string to (space_system_qn, name).
    /// The string is split by "/",
    /// If the path contains any name not found in the NameDb or if the path is emty, None is returned
    pub fn parse_ss_name(name_db: &NameDb, qnstr: &str) -> Option<(QualifiedName, NameIdx)> {
        let mut v = Vec::new();

        for p in qnstr.split("/").skip_while(|x| x.is_empty()) {
            if let Some(idx) = name_db.get(p) {
                v.push(idx);
            } else {
                return None;
            }
        }

        let name = v.pop()?;
        Some((QualifiedName(v), name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Enum)]
pub enum NameReferenceType {
    ParameterType,
    Parameter,
    SequenceContainer,
    Algorithm,
}

impl std::fmt::Debug for QualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for idx in &self.0 {
            write!(f, "/{:?}", idx.into_usize())?;
        }
        Ok(())
    }
}

/// non zero U32 which has the advantage of not consuming extra space inside an Option<>
/// it is used to index parameters, containers, matchcrietrias...
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

#[derive(Clone, Debug, Default)]
pub struct NameDescription {
    pub name: NameIdx,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
}

impl NameDescription {
    pub fn new(name: NameIdx) -> NameDescription {
        NameDescription { name, short_description: None, long_description: None }
    }
}

#[derive(Debug)]
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

pub trait NamedItem {
    fn name_descr(&self) -> &NameDescription;
    fn name(&self) -> NameIdx {
        self.name_descr().name
    }
}
pub struct Parameter {
    pub ndescr: NameDescription,
    pub ptype: Option<DataTypeIdx>,
    pub data_source: DataSource,
}

impl NamedItem for Parameter {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

pub struct SequenceContainer {
    pub ndescr: NameDescription,
    pub base_container: Option<(ContainerIdx, Option<MatchCriteria>)>,
    //abstract is a reserved word in Rust
    pub abstract_: bool,
    pub entries: Vec<ContainerEntry>,
}

impl NamedItem for SequenceContainer {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

pub struct ContainerEntry {
    pub location_in_container: Option<LocationInContainerInBits>,
    pub include_condition: Option<MatchCriteria>,
    pub data: ContainerEntryData,
}

pub enum ContainerEntryData {
    ParameterRef(ParameterIdx),
    ContainerRef(ContainerIdx),
    IndirectParameterRef(IndirectParameterRefEntry),
    ArrayParameterRef(ArrayParameterRefEntry),
}

#[derive(Debug)]
pub struct LocationInContainerInBits {
    pub reference_location: ReferenceLocationType,
    pub location_in_bits: i32,
}

/// The location may be relative to the start of the container (containerStart),
/// or relative to the end of the previous entry (previousEntry)
#[derive(Debug)]
pub enum ReferenceLocationType {
    ContainerStart,
    PreviousEntry,
}

pub enum MatchCriteria {
    Comparison(Comparison),
}


pub struct Comparison {
    pub param_instance: ParameterInstanceRef,
    pub comparison_operator: ComparisonOperator,
    pub value: ValueUnion<()>,
}

#[derive(Debug)]
pub enum ComparisonOperator {
    Equality,
    Inequality,
    LargerThan,
    LargerOrEqualThan,
    SmallerThan,
    SmallerOrEqualThan
}

#[derive(Debug)]
pub struct ParameterInstanceRef {
    pub pidx: ParameterIdx,
    pub instance: i32,
    pub use_calibrated_value: bool
}


pub struct IndirectParameterRefEntry {}

pub struct ArrayParameterRefEntry {}

#[derive(Debug)]
pub struct AbsoluteTimeDataType {}

#[derive(Debug)]
pub enum IntegerValue {
    FixedValue(i64),
    DynamicValue(DynamicValueType),
}

#[derive(Debug)]
pub struct DynamicValueType {}

pub struct SpaceSystem {
    id: SpaceSystemIdx,
    pub fqn: QualifiedName,
    pub name: NameDescription,
    pub parameters: HashMap<NameIdx, ParameterIdx>,
    pub parameter_types: HashMap<NameIdx, DataTypeIdx>,
    pub containers: HashMap<NameIdx, ContainerIdx>,
}

impl SpaceSystem {
    pub fn new(id: SpaceSystemIdx, name: NameIdx, fqn: QualifiedName) -> SpaceSystem {
        SpaceSystem {
            id,
            name: NameDescription::new(name),
            fqn,
            parameters: HashMap::new(),
            parameter_types: HashMap::new(),
            containers: HashMap::new(),
        }
    }

    pub fn name(&self) -> NameIdx {
        self.name.name
    }
}

impl MissionDatabase {
    pub fn new() -> Self {
        let mut mdb = MissionDatabase {
            name_db: Arc::new(ThreadedRodeo::<NameIdx>::new()),
            space_systems: Vec::new(),
            space_systems_qn: HashMap::new(),
            parameter_types: Vec::new(),
            parameters: Vec::new(),
            containers: Vec::new(),
            match_criteria: Vec::new(),
        };
        //create the root space system - it has "" name and an empty qualified name
        let ss_idx = SpaceSystemIdx::new(0);
        let ss = SpaceSystem::new(ss_idx, mdb.name_db.get_or_intern(""), QualifiedName::empty());
        mdb.space_systems.push(ss);
        mdb.space_systems_qn.insert(QualifiedName::empty(), ss_idx);

        mdb
    }

    pub fn new_space_system(&mut self, fqn: QualifiedName) -> Result<SpaceSystemIdx, String> {
        let ss_id = SpaceSystemIdx::new(self.space_systems.len());

        if self.space_systems_qn.contains_key(&fqn) {
            return Err("A spacesystem with the given fqn already exists".to_string());
        }

        match fqn.name() {
            Some(name) => {
                let ss = SpaceSystem::new(ss_id, name, fqn.clone());
                self.space_systems.push(ss);
                self.space_systems_qn.insert(fqn, ss_id);
                Ok(ss_id)
            }
            None => Err("Empty names are not allowed".to_owned()),
        }
    }

    pub fn add_parameter_type(
        &mut self,
        space_system: &QualifiedName,
        ptype: DataType,
    ) -> DataTypeIdx {
        let ptype_name = ptype.name();

        let idx = DataTypeIdx::new(self.parameter_types.len());
        self.parameter_types.push(ptype);

        let ss = self.get_space_system_mut(space_system).unwrap();
        ss.parameter_types.insert(ptype_name, idx);
        idx
    }

    pub fn add_parameter(
        &mut self,
        space_system: &QualifiedName,
        param: Parameter,
    ) -> ParameterIdx {
        let param_name = param.name();

        let idx = ParameterIdx::new(self.parameters.len());
        self.parameters.push(param);

        let ss = self.get_space_system_mut(space_system).unwrap();
        ss.parameters.insert(param_name, idx);
        idx
    }

    pub fn add_container(
        &mut self,
        space_system: &QualifiedName,
        container: SequenceContainer,
    ) -> ParameterIdx {
        let name = container.name();

        let idx = ContainerIdx::new(self.containers.len());
        self.containers.push(container);

        let ss = self.get_space_system_mut(space_system).unwrap();
        ss.containers.insert(name, idx);
        idx
    }

    pub fn get_space_system_mut(&mut self, fqn: &QualifiedName) -> Option<&mut SpaceSystem> {
        match self.space_systems_qn.get_mut(fqn) {
            None => None,
            Some(idx) => self.space_systems.get_mut(idx.index()),
        }
    }

    pub fn get_space_system(&self, fqn: &QualifiedName) -> Option<&SpaceSystem> {
        match self.space_systems_qn.get(fqn) {
            None => None,
            Some(idx) => self.space_systems.get(idx.index()),
        }
    }

    pub fn get_container(&self, idx: ContainerIdx) -> &SequenceContainer {
        &self.containers[idx.index()]
    }

    pub fn get_container_idx(
        &self,
        space_system: &QualifiedName,
        name: NameIdx,
    ) -> Option<ContainerIdx> {
        self.get_space_system(space_system).and_then(|ss| ss.containers.get(&name)).map(|idx| *idx)
    }

    pub fn get_data_type(&self, idx: DataTypeIdx) -> &DataType {
        &self.parameter_types[idx.index()]
    }

    pub fn get_parameter_type_idx(
        &self,
        space_system: &QualifiedName,
        name: NameIdx,
    ) -> Option<DataTypeIdx> {
        self.get_space_system(space_system)
            .and_then(|ss| ss.parameter_types.get(&name))
            .map(|idx| *idx)
    }

    pub fn get_parameter(&self, idx: DataTypeIdx) -> &Parameter {
        &self.parameters[idx.index()]
    }

    pub fn get_parameter_idx(
        &self,
        space_system: &QualifiedName,
        name: NameIdx,
    ) -> Option<DataTypeIdx> {
        self.get_space_system(space_system).and_then(|ss| ss.parameters.get(&name)).map(|idx| *idx)
    }

    pub fn name2str(&self, idx: NameIdx) -> &str {
        self.name_db.try_resolve(&idx).unwrap_or("<none>")
    }

    pub fn qn_to_string(&self, qn: &QualifiedName) -> String {
        qn.to_string(&self.name_db)
    }

    pub fn get_or_intern(&mut self, name_str: &str) -> NameIdx {
        self.name_db.get_or_intern(name_str)
    }
    pub fn name_db(&mut self) -> NameDb {
        Arc::clone(&self.name_db)
    }

    pub fn name_db_ref(&self) -> &NameDb {
        &self.name_db
    }

    /// searches a container by fully qualified name
    pub fn search_container(&self, qnstr: &str) -> Option<ContainerIdx> {
        let (ssqn, name) = QualifiedName::parse_ss_name(&self.name_db, qnstr)?;

        let ss = self.get_space_system(&ssqn)?;
        ss.containers.get(&name).copied()
    }
}
