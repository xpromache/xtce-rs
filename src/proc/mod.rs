use thiserror::Error;

use crate::{mdb::{MatchCriteria, MissionDatabase}, bitbuffer::BitBuffer, value::ParameterValue, pvlist::ParameterValueList};

pub mod containers;
pub mod types;
pub mod encodings;

fn check_match(mc: &MatchCriteria, ctx: &ProcCtx) -> bool {
    true
}


#[derive(Error, Debug)]
pub enum MdbProcError {
    #[error("out of bounds")]
     OutOfBounds(String),
    #[error("no data type available")]
    NoDataTypeAvailable(String),
    #[error("invalid mdb")]
    InvalidMdb(String),
    #[error("invalid value")]
    InvalidValue(String),
    
}

pub(crate) struct ProcCtx<'a> {
    mdb: &'a MissionDatabase,
    buf: BitBuffer<'a>,
    result: ParameterValueList,

    //where in the overall packet this container starts
    start_offset: u32
}
