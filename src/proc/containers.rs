
use crate::{
    bitbuffer::BitBuffer,
    mdb::{
        ContainerEntryData, ContainerIdx, 
        MissionDatabase, NamedItem, ParameterIdx, ReferenceLocationType,
        SequenceContainer
    },
    value::{ParameterValue}, pvlist::ParameterValueList,
};

use super::{check_match, types, MdbProcError, ProcCtx};

//1GB that should be plenty enough
const MAX_PACKET_SIZE: usize = (u32::MAX / 4) as usize;

pub fn process(
    mdb: &MissionDatabase,
    packet: &[u8],
    root_container: ContainerIdx,
) -> Result<ParameterValueList, MdbProcError> {
    if packet.len() > MAX_PACKET_SIZE {
        panic!("Packet too long. max size is {}", MAX_PACKET_SIZE)
    }
    let container = mdb.get_container(root_container);
    let mut ctx =
        ProcCtx { mdb, buf: BitBuffer::wrap(packet), result: ParameterValueList::new(), start_offset: 0 };
    extract_container(container, &mut ctx)?;

    Ok(ctx.result)
}

fn extract_container(container: &SequenceContainer, ctx: &mut ProcCtx) -> Result<(), MdbProcError> {
    for entry in &container.entries {
        if let Some(mc) = &entry.include_condition {
            if !check_match(mc, ctx) {
                continue;
            }
        }
        if let Some(lic) = &entry.location_in_container {
            let pos = ctx.buf.get_position();
            let newpos = match lic.reference_location {
                ReferenceLocationType::ContainerStart => lic.location_in_bits as i64,
                ReferenceLocationType::PreviousEntry => pos as i64 + lic.location_in_bits as i64,
            };
            if newpos < 0 || newpos > ctx.buf.bitsize() as i64 {
                let serr = format!("Error when extracting entry from container {}. Bit position {} is outside the container (size in bits: {})",
                ctx.mdb.name2str(container.name()), newpos, ctx.buf.bitsize());
                return Err(MdbProcError::OutOfBounds(serr));
            }
            ctx.buf.set_position(newpos as usize)
        }
        extract_entry(&entry.data, ctx)?;
    }

    Ok(())
}

fn extract_entry(entry: &ContainerEntryData, ctx: &mut ProcCtx) -> Result<(), MdbProcError> {
    match *entry {
        ContainerEntryData::ParameterRef(pidx) => extract_parameter(pidx, ctx)?,
        ContainerEntryData::ContainerRef(_) => todo!(),
        ContainerEntryData::IndirectParameterRef(_) => todo!(),
        ContainerEntryData::ArrayParameterRef(_) => todo!(),
    };

    Ok(())
}

fn extract_parameter(pidx: ParameterIdx, ctx: &mut ProcCtx) -> Result<(), MdbProcError> {
    let param = ctx.mdb.get_parameter(pidx);

    let ptype_idx = param.ptype.ok_or(MdbProcError::NoDataTypeAvailable(format!(
        "No data type available for parameter {}",
        ctx.mdb.name2str(param.name())
    )))?;
    let dtype = ctx.mdb.get_data_type(ptype_idx);

    let raw_value = types::extract(dtype, ctx)?;
    let eng_value = types::calibrate(&raw_value, dtype, ctx)?;

    let pv = ParameterValue { pidx, raw_value, eng_value };

    println!("------------ Yuhuuu got our first pv: {:?}", pv);
    
    ctx.result.push(pv);

    Ok(())
}
