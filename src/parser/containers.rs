use std::str::FromStr;

use roxmltree::Node;

use crate::{
    mdb::{
        ContainerEntry, ContainerEntryData, ContainerIdx, IntegerValue,
        LocationInContainerInBits, MatchCriteriaIdx, MissionDatabase,
        NameReferenceType, ReferenceLocationType, SequenceContainer, Index,
    },
    parser::utils::{read_attribute, read_mandatory_attribute, read_name_description},
};

use super::{
    misc::{read_integer_value, read_match_criteria, resolve_para_ref, resolve_ref},
    utils::get_parse_error,
    ParseContext, XtceError,
};

pub(super) fn add_container(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<(), XtceError> {
    let abstract_ = read_attribute::<bool>(&ctx.node, "abstract")?.unwrap_or(true);
    let ndescr = read_name_description(ctx);

    let mut entry_list: Vec<ContainerEntry> = Vec::new();

    let mut base_container = None;

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "EntryList" => {
                read_entry_list(mdb, ctx, &cnode, &mut entry_list)?;
            }
            "BaseContainer" => {
                base_container.replace(read_base_container(mdb, ctx, &cnode)?);
            }
            "LongDescription" | "" => continue,
            _ => log::warn!("ignoring container unknown property '{}'", cnode.tag_name().name()),
        };
    }
    
    let sc = SequenceContainer { ndescr, base_container, abstract_, entries: entry_list, idx: Index::invalid() };
    mdb.add_container(ctx.path, sc);
    Ok(())
}

fn read_base_container(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<(ContainerIdx, Option<MatchCriteriaIdx>), XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "containerRef")?;
    let cidx = resolve_ref(mdb, ctx, &pref, NameReferenceType::SequenceContainer)?;
    let mut mcidx = None;

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "RestrictionCriteria" => mcidx = Some(read_match_criteria(mdb, ctx, &cnode)?),
            "" => continue,
            _ => {
                log::warn!("ignoring base container unknown property '{}'", cnode.tag_name().name())
            }
        }
    }

    Ok((cidx, mcidx))
}

fn read_entry_list(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    list: &mut Vec<ContainerEntry>,
) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "ParameterRefEntry" => list.push(read_para_entry(mdb, ctx, &cnode)?),
            "ContainerRefEntry" => {}
            "IndirectParameterRefEntry" => {}
            "ArrayParameterRefEntry" => {}
            "" => continue,
            _ => log::warn!(
                "ignoring sequence container entry list unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    Ok(())
}

fn read_para_entry(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<ContainerEntry, XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
    let (pidx, aggr_path) = resolve_para_ref(mdb, ctx, &pref)?;

    if let Some(_) = aggr_path {
        return Err(XtceError::InvalidReference(format!(
            "Cannot reference a aggregate member in the container parameter entry: {}",
            pref
        )));
    }

    let mut entry = ContainerEntry {
        location_in_container: None,
        include_condition: None,
        data: ContainerEntryData::ParameterRef(pidx),
    };

    read_common_entry_elements(mdb, ctx, node, &mut entry)?;

    Ok(entry)
}

fn read_common_entry_elements(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    entry: &mut ContainerEntry,
) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "LocationInContainerInBits" => {
                let lic = read_location_in_container(mdb, ctx, &cnode)?;
                entry.location_in_container.replace(lic);
            }
            "IncludeCondition" => {
                entry.include_condition.replace(read_match_criteria(mdb, ctx, &cnode)?);
            }
            "" => continue,
            _ => log::warn!("ignoring unknown  '{}'", cnode.tag_name().name()),
        };
    }
    Ok(())
}

fn read_location_in_container(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<LocationInContainerInBits, XtceError> {
    let reference_location = (read_attribute::<ReferenceLocationType>(node, "referenceLocation")?)
        .unwrap_or(ReferenceLocationType::PreviousEntry);

    let iv = read_integer_value(mdb, ctx, &node)?;

    let location_in_bits = match iv {
        IntegerValue::FixedValue(v) => i32::try_from(v).map_err(|_| {
            get_parse_error(
                format!("Value {}  specified for LocationInContainerInBits is out of range", v),
                node,
            )
        })?,
        IntegerValue::DynamicValue(_) => {
            return Err(get_parse_error(
                format!("DynamicValue not supported for LocationInContainerInBits"),
                node,
            ))
        }
    };

    let loc = LocationInContainerInBits { reference_location, location_in_bits };

    Ok(loc)
}

impl FromStr for ReferenceLocationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "containerStart" => Ok(ReferenceLocationType::ContainerStart),
            "previousEntry" => Ok(ReferenceLocationType::PreviousEntry),
            _ => Err("please use one of previousEntry or containerStart".to_owned()),
        }
    }
}
