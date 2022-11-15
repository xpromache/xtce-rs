use thiserror::Error;

use crate::{
    bitbuffer::BitBuffer,
    mdb::{
        ContainerEntry, ContainerEntryData, ContainerIdx, MatchCriteria, MissionDatabase,
        NamedItem, Parameter, ReferenceLocationType, SequenceContainer, DataType, DataEncoding,
    },
    value::{ParameterValue, RawValue},
};

use super::{check_match, ProcCtx, MdbProcError, encodings::extract_encoding};

//1GB that should be plenty enough
const MAX_PACKET_SIZE: usize = (u32::MAX / 4) as usize;

pub fn process(
    mdb: &MissionDatabase,
    packet: &[u8],
    root_container: ContainerIdx,
) -> Result<Vec<ParameterValue>, MdbProcError> {
    if packet.len() > MAX_PACKET_SIZE {
        panic!("Packet too long. max size is {}", MAX_PACKET_SIZE)
    }
    let container = mdb.get_container(root_container);
    let mut ctx = ProcCtx { mdb, buf: BitBuffer::wrap(packet), result: Vec::new() };
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
        extract_entry(&entry.data, ctx);
    }

    Ok(())
}

fn extract_entry(entry: &ContainerEntryData, ctx: &mut ProcCtx) -> Result<(), MdbProcError> {
    match *entry {
        ContainerEntryData::ParameterRef(pidx) => {
            let p = ctx.mdb.get_parameter(pidx);
            extract_parameter(p, ctx);
        }
        ContainerEntryData::ContainerRef(_) => todo!(),
        ContainerEntryData::IndirectParameterRef(_) => todo!(),
        ContainerEntryData::ArrayParameterRef(_) => todo!(),
    };

    Ok(())
}

fn extract_parameter(parameter: &Parameter, ctx: &mut ProcCtx) -> Result<(), MdbProcError> {
    let ptype = parameter.ptype.ok_or(MdbProcError::NoDataTypeAvailable(format!(
        "No data type available for parameter {}",
        ctx.mdb.name2str(parameter.name())
    )))?;
    let ptype = ctx.mdb.get_parameter_type(ptype);
    
    let rv = extract(ptype, ctx)?;
    Ok(())
}


fn extract(ptype: &DataType, ctx: &mut ProcCtx) -> Result<RawValue, MdbProcError> {
    if let DataEncoding::None = ptype.encoding {
        match ptype.type_data {
            crate::mdb::TypeData::Aggregate(_) => todo!(),
            crate::mdb::TypeData::Array(_) => todo!(),
            _ => {
                return Err(MdbProcError::InvalidMdb(format!("base data type without encoding: {}", ctx.mdb.name2str(ptype.name()))));
            }
        }
    } else {
        return extract_encoding(&ptype.encoding, ctx);
    }
}
