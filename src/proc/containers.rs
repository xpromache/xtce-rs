use crate::{
    mdb::{
        types::TypeData, ArrayParameterRefEntry, ContainerEntryData, ContainerIdx,
        IndirectParameterRefEntry, MissionDatabase, NamedItem, ParameterIdx,
        ReferenceLocationType, SequenceContainer,
    },
    proc::criteria_evaluator::MatchResult,
    pvlist::ParameterValueList,
    value::ParameterValue,
};

use super::{types, ProcCtx, ProcError, Result, TmProcessor};
use crate::time::INVALID_INSTANT;

//1GB that should be plenty enough
pub(crate) const MAX_PACKET_SIZE: usize = (u32::MAX / 4) as usize;

/// Processes a packet without any state: references to previous parameter values are not resolved
/// and the generation and acquisition times are not set.
///
/// Use [TmProcessor] to process a stream of packets.
pub fn process(
    mdb: &MissionDatabase,
    packet: &[u8],
    root_container: ContainerIdx,
) -> Result<ParameterValueList> {
    let mut tmp = TmProcessor::new(mdb)?;
    tmp.process(packet, root_container, INVALID_INSTANT, INVALID_INSTANT)
}

/// extracts the parameters from the packet starting with the root container; the values are added to the ctx.result
pub(crate) fn extract(ctx: &mut ProcCtx, root_container: ContainerIdx) -> Result<()> {
    let container = ctx.mdb.get_container(root_container);
    extract_container(ctx, container)
}

fn extract_container(ctx: &mut ProcCtx, container: &SequenceContainer) -> Result<()> {
    let mdb = ctx.mdb();
    log::debug!("Extracting container {}", mdb.name2str(container.name()));
    if let Some(details) = ctx.details.as_mut() {
        details.containers.push(super::MatchedContainer {
            container: container.idx,
            start: ctx.cbuf.start_offset,
        });
    }
    let parent_container = ctx.current_container.replace(container.idx);
    let r = extract_container_entries(ctx, container);
    ctx.current_container = parent_container;
    r
}

fn extract_container_entries(ctx: &mut ProcCtx, container: &SequenceContainer) -> Result<()> {
    let mdb = ctx.mdb();

    // the maximum position reached by any entry (entries can be positioned relative to the container start)
    let mut max_pos = ctx.cbuf.get_position();

    for entry in &container.entries {
        if let Some(mcidx) = &entry.include_condition {
            let evaluator = ctx.pdata.get_criteria_evaluator(*mcidx);
            if evaluator.evaluate(ctx) != MatchResult::OK {
                continue;
            }
        }

        if let Some(lic) = &entry.location_in_container {
            let location = ctx.get_integer_value(&lic.location_in_bits)?;
            let pos = ctx.cbuf.get_position();
            let newpos = match lic.reference_location {
                ReferenceLocationType::ContainerStart => location,
                ReferenceLocationType::PreviousEntry => pos as i64 + location,
            };

            if newpos < 0 || newpos > ctx.cbuf.bitsize() as i64 {
                return Err(ProcError::OutOfBounds(format!(
                    "Container {}: the entry location {} is outside the buffer (size: {} bits)",
                    mdb.name2str(container.name()),
                    newpos,
                    ctx.cbuf.bitsize()
                )));
            }
            ctx.cbuf.set_position(newpos as usize)
        }

        if let Some(rep) = &entry.repeat {
            let count = ctx.get_integer_value(&rep.count)?;
            if count < 0 {
                return Err(ctx.decoding_error(&format!("negative repeat count {}", count)));
            }
            for _ in 0..count {
                extract_entry(&entry.data, ctx)?;
                // like in Yamcs, the offset is applied also after the last repetition
                let offset = ctx.get_integer_value(&rep.offset_in_bits)?;
                let newpos = ctx.cbuf.get_position() as i64 + offset;
                if newpos < 0 || newpos > ctx.cbuf.bitsize() as i64 {
                    return Err(ProcError::OutOfBounds(format!(
                        "repeat offset leads to position {} outside the buffer",
                        newpos
                    )));
                }
                ctx.cbuf.set_position(newpos as usize);
            }
        } else {
            extract_entry(&entry.data, ctx)?;
        }
        max_pos = max_pos.max(ctx.cbuf.get_position());
    }

    // all the inheriting containers whose restriction criteria match are extracted,
    // each of them starting at the position after the last entry of this container
    let bitp = ctx.cbuf.get_position();
    if let Some(children) = mdb.child_containers.get(&container.idx) {
        for c in children {
            let child = mdb.get_container(*c);

            //unwrap is ok becasue the child has to have the base_container set to its parent
            let mcidx = child.base_container.unwrap().1;
            let match_res = match mcidx {
                Some(mcidx) => {
                    let evaluator = ctx.pdata.get_criteria_evaluator(mcidx);
                    evaluator.evaluate(ctx)
                }
                //no match criteria means it always matches
                None => MatchResult::OK,
            };

            let log_level = match match_res {
                MatchResult::NOK | MatchResult::OK => log::Level::Trace,
                MatchResult::UNDEF => log::Level::Info,
                MatchResult::ERROR => log::Level::Warn,
            };
            log::log!(log_level,
                "Match result for {} -> {} inheritance: {:?}",
                mdb.name2str(container.name()),
                mdb.name2str(child.name()),
                match_res
            );

            if match_res == MatchResult::OK {
                ctx.cbuf.set_position(bitp);
                extract_container(ctx, child)?;
                max_pos = max_pos.max(ctx.cbuf.get_position());
            }
        }
    }
    ctx.cbuf.set_position(max_pos);

    Ok(())
}

fn extract_entry(entry: &ContainerEntryData, ctx: &mut ProcCtx) -> Result<()> {
    match entry {
        ContainerEntryData::ParameterRef(pidx) => extract_parameter(*pidx, ctx)?,
        ContainerEntryData::ContainerRef(cidx) => extract_container_ref(*cidx, ctx)?,
        ContainerEntryData::IndirectParameterRef(ipe) => extract_indirect_parameter(ipe, ctx)?,
        ContainerEntryData::ArrayParameterRef(ape) => extract_array_parameter(ape, ctx)?,
    };

    Ok(())
}

/// extracts a container included in this one.
///
/// The included container is processed in a buffer starting at the current position (which has to be byte aligned);
/// the position in the parent advances with the container size if specified or else with the size of the data extracted.
///
/// Note: Yamcs processes the root of the inheritance hierarchy of the referenced container;
/// here the referenced container is processed directly (together with its matching inheriting containers).
/// The two are equivalent if the referenced container has no base container.
fn extract_container_ref(cidx: ContainerIdx, ctx: &mut ProcCtx) -> Result<()> {
    let mdb = ctx.mdb();
    let container = mdb.get_container(cidx);
    let pos = ctx.cbuf.get_position();
    if pos % 8 != 0 {
        // like Yamcs, skip it with a warning
        log::warn!(
            "Container {} referenced at bit position {} which is not byte aligned; skipping it",
            mdb.name2str(container.name()),
            pos
        );
        return Ok(());
    }

    let sub = ctx.cbuf.slice()?;
    let parent = std::mem::replace(&mut ctx.cbuf, sub);
    let r = extract_container(ctx, container);
    let consumed = ctx.cbuf.get_position();
    ctx.cbuf = parent;
    r?;

    let size = container.size_in_bits.map(|s| s as usize).unwrap_or(consumed);
    if pos + size > ctx.cbuf.bitsize() {
        return Err(ProcError::OutOfBounds(format!(
            "Container {} of size {} bits exceeds the buffer",
            mdb.name2str(container.name()),
            size
        )));
    }
    ctx.cbuf.set_position(pos + size);
    Ok(())
}

/// extracts a parameter whose identity is given by the value of another parameter
fn extract_indirect_parameter(ipe: &IndirectParameterRefEntry, ctx: &mut ProcCtx) -> Result<()> {
    let mdb = ctx.mdb();
    let v = ctx.get_param_value(&ipe.para_ref).ok_or_else(|| {
        ProcError::MissingValue(format!(
            "No value for {} required by the indirect parameter entry",
            ipe.para_ref.to_string(mdb)
        ))
    })?;
    let name = v.to_string();
    let pidx = match &ipe.alias_namespace {
        Some(ns) => mdb.get_parameter_by_alias(ns, &name),
        None => mdb.search_parameter(&name),
    }
    .ok_or_else(|| {
        ProcError::DecodingError(format!(
            "Cannot find the parameter '{}' (alias namespace {:?}) referenced by the indirect parameter entry",
            name, ipe.alias_namespace
        ))
    })?;

    extract_parameter(pidx, ctx)
}

fn extract_array_parameter(ape: &ArrayParameterRefEntry, ctx: &mut ProcCtx) -> Result<()> {
    let mdb = ctx.mdb();
    let pidx = ape.pidx;
    let param = mdb.get_parameter(pidx);
    let dtype = param.ptype.map(|idx| mdb.get_data_type(idx)).ok_or_else(|| {
        ProcError::NoDataTypeAvailable(format!(
            "No data type available for parameter {}",
            mdb.name2str(param.name())
        ))
    })?;
    let atype = match &dtype.type_data {
        TypeData::Array(atype) => atype,
        _ => {
            return Err(ProcError::InvalidMdb(format!(
                "Parameter {} used in an ArrayParameterRefEntry is not an array",
                mdb.name2str(param.name())
            )))
        }
    };
    let dims = if ape.dim.is_empty() { &atype.dim } else { &ape.dim };
    if dims.is_empty() {
        return Err(ProcError::InvalidMdb(format!(
            "No array size specified for parameter {}",
            mdb.name2str(param.name())
        )));
    }

    ctx.pidx.replace(pidx);
    let start_bit = ctx.cbuf.get_position();
    let (raw_value, _cpos) = types::extract_array(atype, dims, ctx)?;
    let eng_value = types::calibrate(&raw_value, dtype, ctx)?;
    ctx.result.push(ParameterValue::new(pidx, raw_value, eng_value));
    ctx.record_position(start_bit);
    ctx.pidx.take();

    Ok(())
}

fn extract_parameter(pidx: ParameterIdx, ctx: &mut ProcCtx) -> Result<()> {
    ctx.pidx.replace(pidx);
    let mdb = ctx.mdb();
    let param = mdb.get_parameter(pidx);

    let ptype_idx = param.ptype.ok_or_else(|| ProcError::NoDataTypeAvailable(format!(
        "No data type available for parameter {}",
        mdb.name2str(param.name())
    )))?;
    let dtype = mdb.get_data_type(ptype_idx);

    let start_bit = ctx.cbuf.get_position();
    let (raw_value, _cpos) = types::extract(dtype, ctx)?;
    let eng_value = types::calibrate(&raw_value, dtype, ctx)?;

    let pv = ParameterValue::new(pidx, raw_value, eng_value);

    ctx.result.push(pv);
    ctx.record_position(start_bit);
    ctx.pidx.take();

    Ok(())
}
