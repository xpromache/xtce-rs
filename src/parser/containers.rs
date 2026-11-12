use std::str::FromStr;

use roxmltree::Node;

use crate::{
    mdb::{
        ArrayParameterRefEntry, ContainerEntry, ContainerEntryData, ContainerIdx, Index,
        IndirectParameterRefEntry, IntegerValue, LocationInContainerInBits, MatchCriteriaIdx,
        MissionDatabase, NameReferenceType, ReferenceLocationType, RepeatEntry, SequenceContainer,
    },
    parser::utils::{read_attribute, read_mandatory_attribute, read_name_description},
};

use super::{
    misc::{read_integer_value, read_match_criteria, read_para_insta_ref, resolve_para_ref, resolve_ref},
    types::read_dimension_list,
    utils::{children, get_parse_error, missing, unsupported},
    ParseContext, XtceError,
};

pub(super) fn add_container(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<(), XtceError> {
    let abstract_ = read_attribute::<bool>(&ctx.node, "abstract")?.unwrap_or(false);
    let ndescr = read_name_description(ctx);

    let mut entry_list: Vec<ContainerEntry> = Vec::new();
    let mut base_container = None;
    let mut size_in_bits = None;

    for cnode in children(&ctx.node) {
        match cnode.tag_name().name() {
            "EntryList" => {
                read_entry_list(mdb, ctx, &cnode, &mut entry_list)?;
            }
            "BaseContainer" => {
                base_container.replace(read_base_container(mdb, ctx, &cnode)?);
            }
            "BinaryEncoding" => size_in_bits = Some(read_container_size(mdb, ctx, &cnode)?),
            "LongDescription" | "AliasSet" | "AncillaryDataSet" | "DefaultRateInStream"
            | "RateInStreamSet" => continue,
            _ => log::warn!("ignoring container unknown property '{}'", cnode.tag_name().name()),
        };
    }

    let sc = SequenceContainer {
        ndescr,
        base_container,
        abstract_,
        entries: entry_list,
        idx: Index::invalid(),
        size_in_bits,
    };
    mdb.add_container(ctx.path, sc);
    Ok(())
}

/// reads the size from the BinaryEncoding element of the container; only a fixed size is supported
pub(super) fn read_container_size(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<u32, XtceError> {
    for cnode in children(node) {
        if cnode.tag_name().name() == "SizeInBits" {
            return match read_integer_value(mdb, ctx, &cnode)? {
                IntegerValue::FixedValue(v) if v >= 0 => Ok(v as u32),
                _ => Err(get_parse_error(
                    "Only a positive FixedValue is supported for the container size",
                    &cnode,
                )),
            };
        }
    }
    Err(missing("SizeInBits", node))
}

fn read_base_container(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<(ContainerIdx, Option<MatchCriteriaIdx>), XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "containerRef")?;
    let cidx = resolve_ref(mdb, ctx, &pref, NameReferenceType::SequenceContainer)?;
    let mut mcidx = None;

    for cnode in children(node) {
        match cnode.tag_name().name() {
            "RestrictionCriteria" => mcidx = Some(read_match_criteria(mdb, ctx, &cnode)?),
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
    for cnode in children(node) {
        let data = match cnode.tag_name().name() {
            "ParameterRefEntry" => read_para_entry(mdb, ctx, &cnode)?,
            "ContainerRefEntry" => {
                let cref = read_mandatory_attribute::<String>(&cnode, "containerRef")?;
                let cidx = resolve_ref(mdb, ctx, &cref, NameReferenceType::SequenceContainer)?;
                ContainerEntryData::ContainerRef(cidx)
            }
            "ArrayParameterRefEntry" => read_array_para_entry(mdb, ctx, &cnode)?,
            "IndirectParameterRefEntry" => read_indirect_para_entry(mdb, ctx, &cnode)?,
            _ => {
                return Err(unsupported("container entry", &cnode));
            }
        };
        let mut entry =
            ContainerEntry { location_in_container: None, include_condition: None, repeat: None, data };
        read_common_entry_elements(mdb, ctx, &cnode, &mut entry)?;
        list.push(entry);
    }

    Ok(())
}

fn read_para_entry(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<ContainerEntryData, XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
    let (pidx, aggr_path) = resolve_para_ref(mdb, ctx, &pref)?;

    if aggr_path.is_some() {
        return Err(XtceError::InvalidReference(format!(
            "Cannot reference a aggregate member in the container parameter entry: {}",
            pref
        )));
    }

    Ok(ContainerEntryData::ParameterRef(pidx))
}

fn read_array_para_entry(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<ContainerEntryData, XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
    let (pidx, aggr_path) = resolve_para_ref(mdb, ctx, &pref)?;
    if aggr_path.is_some() {
        return Err(XtceError::InvalidReference(format!(
            "Cannot reference a aggregate member in the container array parameter entry: {}",
            pref
        )));
    }
    let mut dim = Vec::new();
    for cnode in children(node) {
        if cnode.tag_name().name() == "DimensionList" {
            dim = read_dimension_list(mdb, ctx, &cnode)?;
        }
    }
    Ok(ContainerEntryData::ArrayParameterRef(ArrayParameterRefEntry { pidx, dim }))
}

fn read_indirect_para_entry(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<ContainerEntryData, XtceError> {
    let alias_namespace = read_attribute::<String>(node, "aliasNameSpace")?;
    let pinode = children(node)
        .find(|n| n.tag_name().name() == "ParameterInstance")
        .ok_or_else(|| missing("ParameterInstance", node))?;
    let para_ref = read_para_insta_ref(mdb, ctx, &pinode, false)?;
    Ok(ContainerEntryData::IndirectParameterRef(IndirectParameterRefEntry { alias_namespace, para_ref }))
}

fn read_common_entry_elements(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    entry: &mut ContainerEntry,
) -> Result<(), XtceError> {
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "LocationInContainerInBits" => {
                let lic = read_location_in_container(mdb, ctx, &cnode)?;
                entry.location_in_container.replace(lic);
            }
            "IncludeCondition" => {
                entry.include_condition.replace(read_match_criteria(mdb, ctx, &cnode)?);
            }
            "RepeatEntry" => {
                entry.repeat.replace(read_repeat_entry(mdb, ctx, &cnode)?);
            }
            // processed by the entry specific functions
            "DimensionList" | "ParameterInstance" => {}
            "TimeAssociation" | "AncillaryDataSet" => {
                log::debug!("ignoring container entry element {}", cnode.tag_name().name())
            }
            _ => log::warn!("ignoring unknown  '{}'", cnode.tag_name().name()),
        };
    }
    Ok(())
}

pub(super) fn read_repeat_entry(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<RepeatEntry, XtceError> {
    let mut count = None;
    let mut offset_in_bits = IntegerValue::FixedValue(0);
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "Count" => count = Some(read_integer_value(mdb, ctx, &cnode)?),
            "Offset" => offset_in_bits = read_integer_value(mdb, ctx, &cnode)?,
            _ => return Err(unsupported("repeat entry element", &cnode)),
        }
    }
    let count = count.ok_or_else(|| missing("Count", node))?;
    Ok(RepeatEntry { count, offset_in_bits })
}

pub(super) fn read_location_in_container(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<LocationInContainerInBits, XtceError> {
    let reference_location = (read_attribute::<ReferenceLocationType>(node, "referenceLocation")?)
        .unwrap_or(ReferenceLocationType::PreviousEntry);

    let location_in_bits = read_integer_value(mdb, ctx, node)?;

    Ok(LocationInContainerInBits { reference_location, location_in_bits })
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
