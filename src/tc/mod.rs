//! Command (telecommand) encoding, similar to the Yamcs MetaCommandProcessor.
//!
//! The steps are:
//! 1. collect the argument values: from the user, from the argument assignments of the inheriting commands,
//!    from the initial values of the arguments or of their types.
//! 2. convert the values to engineering values, decalibrate them into raw values and check them against the valid ranges.
//! 3. encode the command container (with its base containers first) into binary.
//!
//! Example:
//! ```no_run
//! # use std::collections::HashMap;
//! # use xtce_rs::{mdb::MissionDatabase, value::Value, tc::encode_command};
//! # let mdb = MissionDatabase::new();
//! let cmd = mdb.search_command("/YSS/SIMULATOR/SWITCH_VOLTAGE_ON").unwrap();
//! let args = HashMap::from([("voltage_num".to_owned(), Value::StringValue(Box::new("1".to_owned())))]);
//! let encoded = encode_command(&mdb, cmd, &args).unwrap();
//! println!("{}", hex::encode(&encoded.binary));
//! ```

pub mod args;
mod json;
pub mod encodings;

use std::collections::HashMap;

use thiserror::Error;

use crate::{
    bitbuffer::{BitBufferError, BitWriter},
    mdb::{
        commands::{
            Argument, BaseContainer, CommandContainerIdx, CommandEntryData, MetaCommandIdx,
        },
        types::{DataType, TypeData},
        DataTypeIdx, IntegerValue, MissionDatabase, NameIdx, NamedItem, ParameterIdx, ReferenceLocationType,
    },
    value::Value,
};

#[derive(Error, Debug, Clone, PartialEq)]
pub enum TcError {
    #[error("the command {0} is abstract and cannot be encoded")]
    AbstractCommand(String),
    #[error("unknown argument {0}")]
    UnknownArgument(String),
    #[error("no value provided for argument {0} (and it has no initial value)")]
    MissingArgument(String),
    #[error("cannot set the argument {0} because it is assigned by the command definition")]
    AssignedArgument(String),
    #[error("argument {name}: {msg}")]
    InvalidArgument { name: String, msg: String },
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("out of range: {0}")]
    OutOfRange(String),
    #[error("encoding error: {0}")]
    EncodingError(String),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("the command exceeds the maximum size: {0}")]
    TooLarge(String),
}

impl TcError {
    /// prefixes the message with the aggregate member or array index
    pub(crate) fn in_member(self, member: &str) -> TcError {
        match self {
            TcError::InvalidValue(m) => TcError::InvalidValue(format!("{}: {}", member, m)),
            TcError::OutOfRange(m) => TcError::OutOfRange(format!("{}: {}", member, m)),
            e => e,
        }
    }

    fn for_argument(self, name: &str) -> TcError {
        match self {
            TcError::InvalidValue(msg) | TcError::OutOfRange(msg) | TcError::EncodingError(msg) => {
                TcError::InvalidArgument { name: name.to_owned(), msg }
            }
            e => e,
        }
    }
}

impl From<BitBufferError> for TcError {
    fn from(e: BitBufferError) -> Self {
        match e {
            BitBufferError::OutOfBounds { .. } => TcError::TooLarge(e.to_string()),
            _ => TcError::EncodingError(e.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, TcError>;

/// Options for the command encoding
pub struct EncodeOptions {
    /// maximum size of the encoded command in bytes
    pub max_size: usize,
    /// engineering values of the parameters referenced in the command containers (ParameterRefEntry)
    pub parameter_values: HashMap<ParameterIdx, Value>,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        EncodeOptions { max_size: 65536, parameter_values: HashMap::new() }
    }
}

/// The value of one argument of an encoded command
#[derive(Debug, Clone)]
pub struct ArgumentValue {
    pub name: NameIdx,
    pub atype: DataTypeIdx,
    pub eng_value: Value,
    pub raw_value: Value,
}

/// The result of encoding a command
#[derive(Debug, Clone)]
pub struct EncodedCommand {
    pub cmd: MetaCommandIdx,
    pub binary: Vec<u8>,
    /// the values of all the arguments (including the inherited ones), the base command arguments first
    pub args: Vec<ArgumentValue>,
}

impl EncodedCommand {
    pub fn arg(&self, mdb: &MissionDatabase, name: &str) -> Option<&ArgumentValue> {
        self.args.iter().find(|a| mdb.name2str(a.name) == name)
    }
}

/// returns all the arguments of the command including the inherited ones (the base command arguments first)
/// together with the value assigned by the inheriting commands (if any)
pub fn all_arguments(mdb: &MissionDatabase, cmd: MetaCommandIdx) -> Vec<(&Argument, Option<&str>)> {
    let mut chain = Vec::new();
    let mut c = Some(cmd);
    while let Some(idx) = c {
        let mc = mdb.get_command(idx);
        chain.push(mc);
        c = mc.base_command;
    }
    let mut r = Vec::new();
    for (level, mc) in chain.iter().enumerate().rev() {
        for arg in &mc.arguments {
            // the assignments are made by the commands inheriting from mc; the most derived one wins
            let assigned = chain[..level]
                .iter()
                .find_map(|c| c.argument_assignments.iter().find(|(n, _)| *n == arg.name()))
                .map(|(_, v)| v.as_str());
            r.push((arg, assigned));
        }
    }
    r
}

/// encodes the command with the given arguments (name -> value) and the default options.
///
/// The values can be given as engineering values of the corresponding type or as strings which are parsed
/// according to the argument type.
pub fn encode_command(
    mdb: &MissionDatabase,
    cmd: MetaCommandIdx,
    args: &HashMap<String, Value>,
) -> Result<EncodedCommand> {
    encode_command_with(mdb, cmd, args, &EncodeOptions::default())
}

pub fn encode_command_with(
    mdb: &MissionDatabase,
    cmd: MetaCommandIdx,
    args: &HashMap<String, Value>,
    opts: &EncodeOptions,
) -> Result<EncodedCommand> {
    let mc = mdb.get_command(cmd);
    if mc.abstract_ {
        return Err(TcError::AbstractCommand(mdb.name2str(mc.name()).to_owned()));
    }
    let all_args = all_arguments(mdb, cmd);

    for name in args.keys() {
        match all_args.iter().find(|(a, _)| mdb.name2str(a.name()) == name) {
            None => return Err(TcError::UnknownArgument(name.clone())),
            Some((_, Some(_))) => return Err(TcError::AssignedArgument(name.clone())),
            _ => {}
        }
    }

    let mut values = Vec::with_capacity(all_args.len());
    for (arg, assigned) in &all_args {
        let name = mdb.name2str(arg.name());
        let dtype = mdb.get_data_type(arg.atype);
        let eng = match (args.get(name), *assigned, arg.initial_value.as_deref()) {
            (Some(v), _, _) => args::to_eng_value(mdb, dtype, v),
            (None, Some(s), _) | (None, None, Some(s)) => {
                args::to_eng_value(mdb, dtype, &Value::StringValue(Box::new(s.to_owned())))
            }
            (None, None, None) => match args::type_initial_value(mdb, dtype) {
                Some(v) => v,
                None => return Err(TcError::MissingArgument(name.to_owned())),
            },
        }
        .map_err(|e| e.for_argument(name))?;
        let raw = args::decalibrate(mdb, dtype, &eng).map_err(|e| e.for_argument(name))?;
        args::check_valid(mdb, dtype, &eng, &raw).map_err(|e| e.for_argument(name))?;
        values.push(ArgumentValue { name: arg.name(), atype: arg.atype, eng_value: eng, raw_value: raw });
    }

    let container = mc.command_container.ok_or_else(|| {
        TcError::Unsupported(format!("command {} without command container", mdb.name2str(mc.name())))
    })?;
    let mut enc = Encoder { mdb, opts, args: &values, w: BitWriter::new(opts.max_size) };
    enc.encode_container_with_size(container, 0)?;

    Ok(EncodedCommand { cmd, binary: enc.w.into_bytes(), args: values })
}

struct Encoder<'a> {
    mdb: &'a MissionDatabase,
    opts: &'a EncodeOptions,
    args: &'a [ArgumentValue],
    w: BitWriter,
}

impl Encoder<'_> {
    /// encodes the container hierarchy starting at the given position;
    /// if the container has a size, the position is set at the end of the container
    fn encode_container_with_size(&mut self, cidx: CommandContainerIdx, start: usize) -> Result<()> {
        self.w.set_position(start)?;
        self.encode_container(cidx, start)?;
        if let Some(size) = self.mdb.get_command_container(cidx).size_in_bits {
            let end = start + size as usize;
            if self.w.get_position() > end {
                return Err(TcError::EncodingError(format!(
                    "the content of container {} exceeds its size of {} bits",
                    self.mdb.name2str(self.mdb.get_command_container(cidx).name()),
                    size
                )));
            }
            self.w.extend_to(end)?;
            self.w.set_position(end)?;
        }
        Ok(())
    }

    /// encodes the base container and then the entries of this container;
    /// start is the position of the start of the container hierarchy (used for the containerStart locations)
    fn encode_container(&mut self, cidx: CommandContainerIdx, start: usize) -> Result<()> {
        let mdb = self.mdb;
        let cc = mdb.get_command_container(cidx);
        match cc.base_container {
            Some(BaseContainer::Command(base)) => self.encode_container(base, start)?,
            Some(BaseContainer::Sequence(_)) => {
                return Err(TcError::Unsupported(format!(
                    "command container {} based on a telemetry container",
                    mdb.name2str(cc.name())
                )))
            }
            None => {}
        }
        let cname = || mdb.name2str(cc.name());

        for entry in &cc.entries {
            if entry.has_include_condition {
                return Err(TcError::Unsupported(format!(
                    "include conditions in command containers ({})",
                    cname()
                )));
            }
            if let Some(lic) = &entry.location_in_container {
                let loc = fixed(&lic.location_in_bits, "entry location")?;
                let newpos = match lic.reference_location {
                    ReferenceLocationType::ContainerStart => start as i64 + loc,
                    ReferenceLocationType::PreviousEntry => self.w.get_position() as i64 + loc,
                };
                if newpos < 0 {
                    return Err(TcError::EncodingError(format!(
                        "container {}: negative entry location {}",
                        cname(),
                        newpos
                    )));
                }
                self.w.set_position(newpos as usize)?;
            }
            match &entry.repeat {
                Some(rep) => {
                    let count = fixed(&rep.count, "repeat count")?;
                    let offset = fixed(&rep.offset_in_bits, "repeat offset")?;
                    for _ in 0..count.max(0) {
                        self.encode_entry(&entry.data, start)?;
                        let newpos = self.w.get_position() as i64 + offset;
                        self.w.set_position(newpos.max(0) as usize)?;
                    }
                }
                None => self.encode_entry(&entry.data, start)?,
            }
        }
        Ok(())
    }

    fn encode_entry(&mut self, data: &CommandEntryData, _start: usize) -> Result<()> {
        let mdb = self.mdb;
        match data {
            CommandEntryData::ArgumentRef(name) => {
                let av = self.args.iter().find(|a| a.name == *name).ok_or_else(|| {
                    TcError::EncodingError(format!(
                        "the argument {} referenced in the container is not an argument of the command",
                        mdb.name2str(*name)
                    ))
                })?;
                let dtype = mdb.get_data_type(av.atype);
                encode_value(mdb, dtype, &av.raw_value, &mut self.w)
                    .map_err(|e| e.for_argument(mdb.name2str(*name)))
            }
            CommandEntryData::FixedValue { value, size_in_bits } => {
                encodings::encode_fixed_value(value, *size_in_bits as usize, &mut self.w)
            }
            CommandEntryData::ParameterRef(pidx) => {
                let param = mdb.get_parameter(*pidx);
                let pname = mdb.name2str(param.name());
                let v = self.opts.parameter_values.get(pidx).ok_or_else(|| {
                    TcError::Unsupported(format!(
                        "no value provided for the parameter {} used in the command container",
                        pname
                    ))
                })?;
                let dtype = param.ptype.map(|t| mdb.get_data_type(t)).ok_or_else(|| {
                    TcError::EncodingError(format!("parameter {} without type", pname))
                })?;
                let eng = args::to_eng_value(mdb, dtype, v)?;
                let raw = args::decalibrate(mdb, dtype, &eng)?;
                encode_value(mdb, dtype, &raw, &mut self.w)
            }
            CommandEntryData::ContainerRef(cidx) => {
                let pos = self.w.get_position();
                self.encode_container_with_size(*cidx, pos)
            }
            CommandEntryData::Unsupported(name) => {
                Err(TcError::Unsupported(format!("command container entry {}", name)))
            }
        }
    }
}

/// returns the value of a fixed integer value (dynamic values are not supported in commands)
fn fixed(iv: &IntegerValue, what: &str) -> Result<i64> {
    match iv {
        IntegerValue::FixedValue(v) => Ok(*v),
        IntegerValue::DynamicValue(_) => {
            Err(TcError::Unsupported(format!("dynamic {} in command containers", what)))
        }
    }
}

/// encodes the raw value of the type (aggregates member by member and arrays element by element)
fn encode_value(mdb: &MissionDatabase, dtype: &DataType, raw: &Value, w: &mut BitWriter) -> Result<()> {
    match (&dtype.type_data, raw) {
        (TypeData::Aggregate(atype), Value::Aggregate(av)) => {
            for m in &atype.members {
                let mv = av.0.get(&m.name()).ok_or_else(|| {
                    TcError::EncodingError(format!("no value for member {}", mdb.name2str(m.name())))
                })?;
                encode_value(mdb, mdb.get_data_type(m.dtype), mv, w)
                    .map_err(|e| e.in_member(mdb.name2str(m.name())))?;
            }
            Ok(())
        }
        (TypeData::Array(atype), Value::Array(av)) => {
            let etype = mdb.get_data_type(atype.dtype);
            for v in &av.values {
                encode_value(mdb, etype, v, w)?;
            }
            Ok(())
        }
        _ => encodings::encode_raw(&dtype.encoding, raw, w),
    }
}
