use std::fmt::Error;

use crate::{
    bitbuffer::BitBuffer,
    error::MdbError,
    mdb::{
        utils::get_member_value, DynamicValueType, MatchCriteria, MatchCriteriaIdx,
        MissionDatabase, NamedItem, ParameterIdx, ParameterInstanceRef,
    },
    pvlist::ParameterValueList,
    value::Value,
};

use self::criteria_evaluator::CriteriaEvaluator;

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
            MatchCriteria::ComparisonList(clist) => {
                criteria_evaluator::from_comparison_list(mdb, clist)?
            }
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
    fn remaining_bytes(&self) -> usize {
        self.buf.remaining_bytes()
    }

    fn get_bits(&mut self, num_bits: usize) -> u64 {
        self.buf.get_bits(num_bits)
    }

    fn get_byte(&mut self) -> u8 {
        self.buf.get_byte()
    }

    pub fn get_bytes_ref(&mut self, len: usize) -> &[u8] {
        self.buf.get_bytes_ref(len)
    }
}

pub(crate) struct ProcCtx<'a, 'b, 'c> {
    mdb: &'a MissionDatabase,
    pdata: &'b mut ProcessorData,
    cbuf: ContainerBuf<'c>,
    result: ParameterValueList,
    pidx: Option<ParameterIdx>,
}

impl<'a> ProcCtx<'a, '_, '_> {
    fn mdb(&mut self) -> &'a MissionDatabase {
        self.mdb
    }

    fn get_param_value(&self, para_ref: &ParameterInstanceRef) -> Option<&Value> {
        if para_ref.instance != 0 {
            todo!()
        }
        if !para_ref.use_calibrated_value {
            todo!()
        }

        self.result.last_inserted(para_ref.pidx).map(|pv| &pv.eng_value).map_or(None, |val| {
            if let Some(path) = &para_ref.member_path {
                get_member_value(val, path)
            } else {
                Some(val)
            }
        })
    }

    ///
    /// returns the value of the dynamic value as a unsigned integer.
    /// returns an error if the value cannot be extracted from the current context or if it cannot be converted to u64
    fn get_dynamic_uint_value(&self, dynpara: &DynamicValueType) -> Result<u64, MdbError> {
        let para_ref = &dynpara.para_ref;
        //let para_name = self.mdb.name2str(self.mdb.get_parameter(para_ref.pidx).name());

        let para_name = || self.mdb.name2str(self.mdb.get_parameter(para_ref.pidx).name());

        let v = self.get_param_value(para_ref).ok_or_else(|| MdbError::MissingValue(format!(
            "Cannot find a value for parameter {} in the current context",
            para_name()
        )))?;

        if let Some(adj) = &dynpara.adjustment {
            //linear adjusment is with f64, convert everything to f64
            let x: f64 = v.try_into().map_err(|_| {
                MdbError::DecodingError(format!(
                    "Cannot convert value {:?} for parameter {} to f64 (double)",
                    v,
                    para_name()
                ))
            })?;
            let y = x * adj.slope + adj.intercept;
            Ok(y as u64)
        } else {
            let x: u64 = v.try_into().map_err(|_| {
                MdbError::DecodingError(format!(
                    "Cannot convert value {:?} for parameter {} to u64",
                    v,
                    para_name()
                ))
            })?;

            Ok(x as u64)
        }
    }

    fn decoding_error(&self, msg: &str) -> MdbError {      
        if let Some(pidx) = self.pidx {
            return MdbError::DecodingError(format!(
                "Error decoding parameter {}: {}",
                self.mdb.name2str(self.mdb.get_parameter(pidx).name()),
                msg
            ));
        } else {
            return MdbError::DecodingError(msg.to_owned());
        }
    }
}
