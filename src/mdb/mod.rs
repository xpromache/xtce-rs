pub mod debug;
pub mod types;
pub mod utils;

use std::sync::Arc;
use std::{collections::HashMap, fmt::Debug};

use std::fmt;
use std::fmt::Formatter;

use lasso::{Key, ThreadedRodeo};

use self::types::{DataType, MemberPath};

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

    //this is the reverse of the base containers relation
    pub child_containers: HashMap<ContainerIdx, Vec<ContainerIdx>>,
}

pub trait NamedItem {
    fn name_descr(&self) -> &NameDescription;
    fn name(&self) -> NameIdx {
        self.name_descr().name
    }
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

    pub fn invalid() -> Self {
        Self(std::num::NonZeroU32::new(u32::MAX).unwrap())
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
    pub base_container: Option<(ContainerIdx, Option<MatchCriteriaIdx>)>,
    //abstract is a reserved word in Rust
    pub abstract_: bool,
    pub entries: Vec<ContainerEntry>,
    pub idx: ContainerIdx
}

impl NamedItem for SequenceContainer {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

pub struct ContainerEntry {
    pub location_in_container: Option<LocationInContainerInBits>,
    pub include_condition: Option<MatchCriteriaIdx>,
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
    ComparisonList(Vec<Comparison>),
}

#[derive(Debug)]
pub struct Comparison {
    pub param_instance: ParameterInstanceRef,
    pub comparison_operator: ComparisonOperator,
    pub value: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ComparisonOperator {
    Equality,
    Inequality,
    LargerThan,
    LargerOrEqualThan,
    SmallerThan,
    SmallerOrEqualThan,
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            ComparisonOperator::Equality => "==",
            ComparisonOperator::Inequality => "!=",
            ComparisonOperator::LargerThan => ">",
            ComparisonOperator::LargerOrEqualThan => ">=",
            ComparisonOperator::SmallerThan => "<",
            ComparisonOperator::SmallerOrEqualThan => "<=",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub struct ParameterInstanceRef {
    pub pidx: ParameterIdx,
    pub member_path: Option<MemberPath>,
    pub instance: i32,
    pub use_calibrated_value: bool,
}

impl ParameterInstanceRef {
    pub fn to_string(&self, mdb: &MissionDatabase) -> String {
        let p = mdb.get_parameter(self.pidx);
        let mut r = mdb.name2str(p.name()).to_string();

        if let Some(path) = &self.member_path {
            r.push('.');
            let path_str = path.iter().map(|pe| pe.to_string(mdb)).collect::<Vec<String>>().join(".");
            r.push_str(&path_str);
        }
        if self.instance != 0 {
            r.push_str(&format!("[inst: {}]", self.instance));
        }
        if self.use_calibrated_value {
            r.push_str(".eng");
        } else {
            r.push_str(".raw");
        }

        r
    }
}

pub struct IndirectParameterRefEntry {}

pub struct ArrayParameterRefEntry {}

#[derive(Debug)]
pub enum IntegerValue {
    FixedValue(i64),
    DynamicValue(DynamicValueType),
}

#[derive(Debug)]
pub struct DynamicValueType {}



pub struct SpaceSystem {
    pub id: SpaceSystemIdx,
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
            child_containers: HashMap::new()
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
        mut container: SequenceContainer,
    ) -> ParameterIdx {
        let name = container.name();

        let idx = ContainerIdx::new(self.containers.len());
        container.idx = idx;
        let base_idx = container.base_container.map(|(idx, _)| idx);

        self.containers.push(container);

      

        let ss = self.get_space_system_mut(space_system).unwrap();
        ss.containers.insert(name, idx);

        if let Some(base_idx) = base_idx {
            self.child_containers.entry(base_idx).or_insert(Vec::new()).push(idx);
        }

        idx
    }

    pub fn add_match_criteria(&mut self, macth_criteria: MatchCriteria) -> MatchCriteriaIdx {
        let idx = MatchCriteriaIdx::new(self.match_criteria.len());
        self.match_criteria.push(macth_criteria);

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

    pub fn get_match_criteria(&self, idx: MatchCriteriaIdx) -> &MatchCriteria {
        &self.match_criteria[idx.index()]
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
