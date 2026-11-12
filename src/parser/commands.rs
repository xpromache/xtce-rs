//! Parsing of the MetaCommands and the CommandContainers

use roxmltree::Node;

use super::{
    containers::{read_container_size, read_location_in_container, read_repeat_entry},
    misc::{resolve_para_ref, resolve_ref},
    utils::*,
    ParseContext, Result, XtceError,
};
use crate::mdb::{
    commands::{
        Argument, BaseContainer, CommandContainer, CommandContainerEntry, CommandEntryData, MetaCommand,
        MetaCommandIdx,
    },
    MissionDatabase, NameIdx, NameReferenceType, NamedItem,
};

pub(super) fn add_command(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<()> {
    let node = &ctx.node;
    let abstract_ = read_attribute::<bool>(node, "abstract")?.unwrap_or(false);
    let mut base_command = None;
    let mut argument_assignments = Vec::new();
    let mut arguments = Vec::new();
    let mut command_container = None;
    let mut unnamed_container = None;

    for cnode in children(node) {
        match cnode.tag_name().name() {
            "BaseMetaCommand" => {
                let bref = read_mandatory_attribute::<String>(&cnode, "metaCommandRef")?;
                base_command = Some(resolve_ref(mdb, ctx, &bref, NameReferenceType::MetaCommand)?);
                for alnode in children(&cnode).filter(|n| n.tag_name().name() == "ArgumentAssignmentList") {
                    for anode in children(&alnode) {
                        let name = read_mandatory_attribute::<String>(&anode, "argumentName")?;
                        let value = read_mandatory_attribute::<String>(&anode, "argumentValue")?;
                        argument_assignments.push((mdb.get_or_intern(&name), value));
                    }
                }
            }
            "ArgumentList" => {
                for anode in children(&cnode) {
                    arguments.push(read_argument(mdb, ctx, &anode)?);
                }
            }
            "CommandContainer" => {
                let name = mdb.get_or_intern(read_mandatory_name(&cnode)?);
                let registered = ctx.name_tree.get_node(ctx.path, name, NameReferenceType::CommandContainer);
                if registered.map(|(_, id)| id) == Some(cnode.id()) {
                    // the container is added to the MDB separately (it is registered in the name tree)
                    command_container = Some(mdb.get_command_container_idx(ctx.path, name).ok_or_else(|| {
                        XtceError::UnresolvedReference(
                            mdb.name2str(name).to_owned(),
                            NameReferenceType::CommandContainer,
                        )
                    })?);
                } else {
                    // another container with the same name is registered
                    let ctx1 = ParseContext { node: cnode, name, ..*ctx };
                    let cc = read_command_container(mdb, &ctx1)?;
                    unnamed_container = Some(cc);
                }
            }
            "LongDescription" | "AliasSet" | "AncillaryDataSet" | "DefaultSignificance"
            | "ContextSignificanceList" | "VerifierSet" | "TransmissionConstraintList" | "Interlock"
            | "ParameterToSetList" | "ParametersToSuspendAlarmsOnSet" => {}
            name => log::warn!("ignoring MetaCommand unknown property '{}'", name),
        }
    }

    for (name, _) in &argument_assignments {
        if find_argument(mdb, base_command, *name).is_none() {
            return Err(get_parse_error(
                format!("Argument assignment for unknown argument '{}'", mdb.name2str(*name)),
                node,
            ));
        }
    }

    if let Some(cc) = unnamed_container {
        command_container = Some(mdb.add_unnamed_command_container(cc));
    }
    let ndescr = read_name_description(ctx);
    mdb.add_command(
        ctx.path,
        MetaCommand { ndescr, abstract_, base_command, argument_assignments, arguments, command_container },
    );
    Ok(())
}

/// looks for the argument in the command and its base commands
fn find_argument(mdb: &MissionDatabase, mut cmd: Option<MetaCommandIdx>, name: NameIdx) -> Option<&Argument> {
    while let Some(idx) = cmd {
        let c = mdb.get_command(idx);
        if let Some(a) = c.arguments.iter().find(|a| a.name() == name) {
            return Some(a);
        }
        cmd = c.base_command;
    }
    None
}

fn read_argument(mdb: &mut MissionDatabase, ctx: &ParseContext, node: &Node) -> Result<Argument> {
    if node.tag_name().name() != "Argument" {
        return Err(unsupported("argument list element", node));
    }
    let tref = read_mandatory_attribute::<String>(node, "argumentTypeRef")?;
    let atype = resolve_ref(mdb, ctx, &tref, NameReferenceType::ArgumentType)?;
    let name = mdb.get_or_intern(read_mandatory_name(node)?);
    let ctx1 = ParseContext { node: *node, name, ..*ctx };
    let ndescr = read_name_description(&ctx1);
    let initial_value = node.attribute("initialValue").map(|s| s.to_owned());
    Ok(Argument { ndescr, atype, initial_value })
}

pub(super) fn add_command_container(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<()> {
    let cc = read_command_container(mdb, ctx)?;
    mdb.add_command_container(ctx.path, cc);
    Ok(())
}

fn read_command_container(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<CommandContainer> {
    let node = &ctx.node;
    let mut entries = Vec::new();
    let mut base_container = None;
    let mut size_in_bits = None;

    for cnode in children(node) {
        match cnode.tag_name().name() {
            "EntryList" => {
                for enode in children(&cnode) {
                    entries.push(read_entry(mdb, ctx, &enode)?);
                }
            }
            "BaseContainer" => {
                let cref = read_mandatory_attribute::<String>(&cnode, "containerRef")?;
                base_container = Some(resolve_container_ref(mdb, ctx, &cref)?);
                if children(&cnode).next().is_some() {
                    log::debug!("ignoring the restriction criteria of the command container base container");
                }
            }
            "BinaryEncoding" => size_in_bits = Some(read_container_size(mdb, ctx, &cnode)?),
            "LongDescription" | "AliasSet" | "AncillaryDataSet" => {}
            name => log::warn!("ignoring CommandContainer unknown property '{}'", name),
        }
    }
    let ndescr = read_name_description(ctx);
    Ok(CommandContainer { ndescr, base_container, entries, size_in_bits })
}

/// resolves a reference to a command container; if not found, a sequence container is looked up
fn resolve_container_ref(mdb: &MissionDatabase, ctx: &ParseContext, cref: &str) -> Result<BaseContainer> {
    match resolve_ref(mdb, ctx, cref, NameReferenceType::CommandContainer) {
        Ok(idx) => Ok(BaseContainer::Command(idx)),
        Err(XtceError::UndefinedReference(..)) => {
            Ok(BaseContainer::Sequence(resolve_ref(mdb, ctx, cref, NameReferenceType::SequenceContainer)?))
        }
        Err(e) => Err(e),
    }
}

fn read_entry(mdb: &mut MissionDatabase, ctx: &ParseContext, node: &Node) -> Result<CommandContainerEntry> {
    let data = match node.tag_name().name() {
        "ArgumentRefEntry" => {
            let aref = read_mandatory_attribute::<String>(node, "argumentRef")?;
            CommandEntryData::ArgumentRef(mdb.get_or_intern(&aref))
        }
        "FixedValueEntry" => {
            let hexv = read_mandatory_attribute::<String>(node, "binaryValue")?;
            let hexv = hexv.trim();
            // like in Yamcs, an odd number of hex digits is padded on the left
            let padded = if hexv.len() % 2 == 1 { format!("0{}", hexv) } else { hexv.to_owned() };
            let value = hex::decode(&padded).map_err(|e| {
                get_parse_error(format!("invalid binaryValue '{}': {}", hexv, e), node)
            })?;
            let size_in_bits =
                read_attribute::<u32>(node, "sizeInBits")?.unwrap_or(8 * value.len() as u32);
            CommandEntryData::FixedValue { value, size_in_bits }
        }
        "ParameterRefEntry" => {
            let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
            let (pidx, path) = resolve_para_ref(mdb, ctx, &pref)?;
            if path.is_some() {
                return Err(get_parse_error("aggregate member references not supported in command containers", node));
            }
            CommandEntryData::ParameterRef(pidx)
        }
        "ContainerRefEntry" => {
            let cref = read_mandatory_attribute::<String>(node, "containerRef")?;
            match resolve_container_ref(mdb, ctx, &cref)? {
                BaseContainer::Command(idx) => CommandEntryData::ContainerRef(idx),
                BaseContainer::Sequence(_) => {
                    CommandEntryData::Unsupported("ContainerRefEntry to telemetry container".to_owned())
                }
            }
        }
        name => {
            log::warn!("command container entry {} not supported; the commands using it cannot be encoded", name);
            CommandEntryData::Unsupported(name.to_owned())
        }
    };

    let mut entry =
        CommandContainerEntry { location_in_container: None, repeat: None, has_include_condition: false, data };
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "LocationInContainerInBits" => {
                entry.location_in_container = Some(read_location_in_container(mdb, ctx, &cnode)?)
            }
            "RepeatEntry" => entry.repeat = Some(read_repeat_entry(mdb, ctx, &cnode)?),
            "IncludeCondition" => {
                log::warn!("IncludeCondition is not supported in command containers");
                entry.has_include_condition = true;
            }
            _ => {}
        }
    }
    Ok(entry)
}
