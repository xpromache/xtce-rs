use crate::{
    bitbuffer::BitBuffer,
    mdb::{MatchCriteria, MatchCriteriaIdx, MissionDatabase},
    pvlist::ParameterValueList, error::MdbError,
};

use self::criteria_evaluator::{CriteriaEvaluator};

pub mod containers;
pub mod criteria_evaluator;
pub mod encodings;
pub mod misc;
pub mod types;



pub struct ProcessorData {
    evaluators: Vec<Box<dyn CriteriaEvaluator>>,
}

impl ProcessorData {
    pub fn new(mdb: &MissionDatabase) -> Result<ProcessorData, MdbError> {
        let mut evaluators = Vec::new();
        for criteria in &mdb.match_criteria {
            evaluators.push(ProcessorData::create_evaluator(mdb, criteria)?);
        }
        Ok(ProcessorData { evaluators })
    }

    fn get_criteria_evaluator(&self, mcidx: MatchCriteriaIdx) -> &Box<dyn CriteriaEvaluator> {
        &self.evaluators[mcidx.index()]
    }

    fn create_evaluator(
        mdb: &MissionDatabase,
        criteria: &MatchCriteria,
    ) -> Result<Box<dyn CriteriaEvaluator>, MdbError> {
        let res = match criteria {
            MatchCriteria::Comparison(comp) => criteria_evaluator::from_comparison(mdb, comp)?,
            MatchCriteria::ComparisonList(clist) => criteria_evaluator::from_comparison_list(mdb, clist)?,
        };

        Ok(res)
    }
}

pub struct ContainerBuf<'a> {
    buf: BitBuffer<'a>,

    //where in the overall packet this container starts
    start_offset: u32,
}

impl<'a> ContainerBuf<'a> {
    pub fn new(packet: &'a [u8]) -> ContainerBuf {
        ContainerBuf { buf: BitBuffer::wrap(packet), start_offset: 0 }
    }

    pub fn slice(&'a self) -> ContainerBuf {
        ContainerBuf { buf: self.buf.slice(), start_offset: (self.buf.get_position() / 8) as u32 }
    }

    fn set_position(&mut self, bit_pos: usize) {
        self.buf.set_position(bit_pos);
    }

    fn get_position(&self) -> usize {
        self.buf.get_position()
    }

    /// return the total size in bits of the container buffer
    fn bitsize(&self) -> usize {
        self.buf.bitsize()
    }
}

pub(crate) struct ProcCtx<'a, 'b, 'c> {
    mdb: &'a MissionDatabase,
    pdata: &'b mut ProcessorData,
    cbuf: ContainerBuf<'c>,
    result: ParameterValueList,
}

impl<'a> ProcCtx<'a, '_, '_> {
    fn mdb(&mut self) -> &'a MissionDatabase {
        self.mdb
    }
}
