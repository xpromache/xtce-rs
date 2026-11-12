//! Command definitions: MetaCommands, their arguments and the command containers describing the binary layout.

use super::{
    DataTypeIdx, LocationInContainerInBits, NameDescription, NameIdx, NamedItem, ParameterIdx,
    RepeatEntry,
};

pub type MetaCommandIdx = super::Index;
pub type CommandContainerIdx = super::Index;

/// A command definition.
///
/// A command can inherit from a base command: it inherits the arguments, some of which may be assigned
/// a value by the inheriting command.
#[derive(Debug)]
pub struct MetaCommand {
    pub ndescr: NameDescription,
    /// abstract commands cannot be sent, they are used only as base for other commands
    pub abstract_: bool,
    pub base_command: Option<MetaCommandIdx>,
    /// values assigned to the arguments of the base commands (argument name, engineering value as string)
    pub argument_assignments: Vec<(NameIdx, String)>,
    pub arguments: Vec<Argument>,
    pub command_container: Option<CommandContainerIdx>,
}

impl NamedItem for MetaCommand {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

#[derive(Debug)]
pub struct Argument {
    pub ndescr: NameDescription,
    pub atype: DataTypeIdx,
    /// the engineering value used if no value is provided when the command is issued.
    /// If not set, the initial value of the type (if any) is used.
    pub initial_value: Option<String>,
}

impl NamedItem for Argument {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

/// The binary layout of a command. It can extend another command container whose entries come first.
#[derive(Debug)]
pub struct CommandContainer {
    pub ndescr: NameDescription,
    pub base_container: Option<BaseContainer>,
    pub entries: Vec<CommandContainerEntry>,
    /// the size of the container if specified (BinaryEncoding/SizeInBits/FixedValue)
    pub size_in_bits: Option<u32>,
}

impl NamedItem for CommandContainer {
    fn name_descr(&self) -> &NameDescription {
        &self.ndescr
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BaseContainer {
    Command(CommandContainerIdx),
    /// telemetry container used as base (not supported for encoding)
    Sequence(super::ContainerIdx),
}

#[derive(Debug)]
pub struct CommandContainerEntry {
    pub location_in_container: Option<LocationInContainerInBits>,
    pub repeat: Option<RepeatEntry>,
    /// set if the entry has an IncludeCondition; these are not supported for encoding
    pub has_include_condition: bool,
    pub data: CommandEntryData,
}

#[derive(Debug)]
pub enum CommandEntryData {
    /// reference by name to an argument of the command (or of one of its base commands)
    ArgumentRef(NameIdx),
    /// fixed binary value; the size_in_bits rightmost bits of the value are written
    FixedValue { value: Vec<u8>, size_in_bits: u32 },
    ParameterRef(ParameterIdx),
    ContainerRef(CommandContainerIdx),
    /// entry not supported for encoding (e.g. ArrayArgumentRefEntry); contains the element name
    Unsupported(String),
}
