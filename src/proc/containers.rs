use crate::{
    mdb::{
        ContainerEntryData, ContainerIdx, MissionDatabase, NamedItem, ParameterIdx,
        ReferenceLocationType, SequenceContainer,
    },
    pvlist::ParameterValueList,
    value::ParameterValue,
};

use super::{types, ContainerBuf, MdbError, ProcCtx, ProcessorData};

//1GB that should be plenty enough
const MAX_PACKET_SIZE: usize = (u32::MAX / 4) as usize;

pub fn process(
    mdb: &MissionDatabase,
    packet: &[u8],
    root_container: ContainerIdx,
) -> Result<ParameterValueList, MdbError> {
    if packet.len() > MAX_PACKET_SIZE {
        panic!("Packet too long. max size is {}", MAX_PACKET_SIZE)
    }
    let container = mdb.get_container(root_container);

    let mut pdata = ProcessorData::new(mdb)?;
    let cbuf = ContainerBuf::new(packet);
    let mut ctx = ProcCtx { mdb, pdata: &mut pdata, cbuf, result: ParameterValueList::new() };
    extract_container(&mut ctx, container)?;

    Ok(ctx.result)
}

fn extract_container(ctx: &mut ProcCtx, container: &SequenceContainer) -> Result<(), MdbError> {
    let mdb = ctx.mdb();
    //let pdata: &mut ProcessorData = &mut ctx.pdata;

    for entry in &container.entries {
        if let Some(mcidx) = &entry.include_condition {
            let evaluator = ctx.pdata.get_criteria_evaluator(*mcidx);
            if !evaluator.evaluate(ctx) {
                continue;
            }
        }

        if let Some(lic) = &entry.location_in_container {
            let cbuf = &mut ctx.cbuf;
            let pos = cbuf.get_position();
            let newpos = match lic.reference_location {
                ReferenceLocationType::ContainerStart => lic.location_in_bits as i64,
                ReferenceLocationType::PreviousEntry => pos as i64 + lic.location_in_bits as i64,
            };

            if newpos < 0 || newpos > cbuf.bitsize() as i64 {
                let serr = format!("Error when extracting entry from container {}. Bit position {} is outside the container (size in bits: {})",
                ctx.mdb.name2str(container.name()), newpos, cbuf.bitsize());
                return Err(MdbError::OutOfBounds(serr));
            }
            cbuf.set_position(newpos as usize)
        }
        extract_entry(&entry.data, ctx)?;
    }

    if let Some((base_container_idx, mcidx)) = &container.base_container {
        if match mcidx {
            Some(mcidx) =>   {
                let evaluator = ctx.pdata.get_criteria_evaluator(*mcidx);
                evaluator.evaluate(ctx) 
            },
            None => true
        } {

        }
    }

    Ok(())
}

fn extract_entry<'a, 'b>(
    entry: &'a ContainerEntryData,
    ctx: &mut ProcCtx,
) -> Result<(), MdbError> {
    match *entry {
        ContainerEntryData::ParameterRef(pidx) => extract_parameter(pidx, ctx)?,
        ContainerEntryData::ContainerRef(_) => todo!(),
        ContainerEntryData::IndirectParameterRef(_) => todo!(),
        ContainerEntryData::ArrayParameterRef(_) => todo!(),
    };

    Ok(())
}

fn extract_parameter(pidx: ParameterIdx, ctx: &mut ProcCtx) -> Result<(), MdbError> {
    let mdb = ctx.mdb();
    let param = mdb.get_parameter(pidx);

    let ptype_idx = param.ptype.ok_or(MdbError::NoDataTypeAvailable(format!(
        "No data type available for parameter {}",
        mdb.name2str(param.name())
    )))?;
    let dtype = mdb.get_data_type(ptype_idx);

    let raw_value = types::extract(dtype, ctx)?;
    let eng_value = types::calibrate(&raw_value, dtype, ctx)?;

    let pv = ParameterValue { pidx, raw_value, eng_value };

    println!("------------ Yuhuuu got our first pv: {:?}", pv);

    ctx.result.push(pv);

    Ok(())
}
