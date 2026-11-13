use crate::{
    bitbuffer::{BitBuffer, BitBufferError},
    mdb::{
        debug::MdbItemDebug, utils::get_member_value, DynamicValueType, IntegerValue,
        MatchCriteria, MatchCriteriaIdx, MissionDatabase, NamedItem, ParameterIdx,
        ParameterInstanceRef, MdbError,
    },
    pvlist::ParameterValueList,
    value::{ParameterValue, Value},
};

use self::criteria_evaluator::CriteriaEvaluator;

pub mod calibrators;
pub mod containers;
pub mod criteria_evaluator;
pub mod encodings;
pub mod misc;
pub mod tm;
pub mod types;

pub use tm::{ParameterCache, TmProcessor};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcError {
    #[error("out of bounds: {0}")]
    OutOfBounds(String),
    #[error("no data type available: {0}")]
    NoDataTypeAvailable(String),
    #[error("invalid mdb: {0}")]
    InvalidMdb(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("out of range: {0}")]
    OutOfRange(String),
    #[error("decoding error: {0}")]
    DecodingError(String),
    #[error("missing value: {0}")]
    MissingValue(String),
    #[error("MDB error: {0}")]
    Mdb(MdbError),
}

type Result<T> = std::result::Result<T, ProcError>;


impl From<std::num::ParseIntError> for ProcError {
    fn from(e: std::num::ParseIntError) -> ProcError {
        return ProcError::InvalidValue(format!("{}", e));
    }
}

impl From<BitBufferError> for ProcError {
    fn from(e: BitBufferError) -> ProcError {
        ProcError::DecodingError(e.to_string())
    }
}

impl From<MdbError> for ProcError {
    fn from(e: MdbError) -> ProcError {
        return ProcError::Mdb(e);
    }
}


pub struct ProcessorData {
    evaluators: Vec<Box<dyn CriteriaEvaluator>>,
}

impl ProcessorData {
    pub fn new(mdb: &MissionDatabase) -> Result<ProcessorData> {
        let mut evaluators: Vec<Box<dyn CriteriaEvaluator>> = Vec::new();
        for criteria in &mdb.match_criteria {
            // an invalid criteria should not prevent the processing of everything else
            match ProcessorData::create_evaluator(mdb, criteria) {
                Ok(ev) => evaluators.push(ev),
                Err(e) => {
                    log::warn!(
                        "Cannot create evaluator for {:?}: {}; it will always evaluate to ERROR",
                        MdbItemDebug { mdb, item: criteria },
                        e
                    );
                    evaluators.push(Box::new(criteria_evaluator::ErrorEvaluator));
                }
            }
        }
        Ok(ProcessorData { evaluators })
    }

    fn get_criteria_evaluator(&self, mcidx: MatchCriteriaIdx) -> &Box<dyn CriteriaEvaluator> {
        &self.evaluators[mcidx.index()]
    }

    fn create_evaluator(
        mdb: &MissionDatabase,
        criteria: &MatchCriteria,
    ) -> Result<Box<dyn CriteriaEvaluator>> {
        let res = match criteria {
            MatchCriteria::Comparison(comp) => criteria_evaluator::from_comparison(mdb, comp)?,
            MatchCriteria::ComparisonList(clist) => {
                criteria_evaluator::from_comparison_list(mdb, clist)?
            }
            MatchCriteria::BooleanExpression(bexpr) => {
                criteria_evaluator::from_boolean_expression(mdb, bexpr)?
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
    pub fn new(packet: &'a [u8]) -> ContainerBuf<'a> {
        ContainerBuf { buf: BitBuffer::wrap(packet), start_offset: 0 }
    }

    /// returns a new container buffer starting at the current position (which has to be at a byte boundary)
    pub fn slice(&self) -> Result<ContainerBuf<'a>> {
        Ok(ContainerBuf {
            buf: self.buf.slice()?,
            start_offset: self.start_offset + (self.buf.get_position() / 8) as u32,
        })
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
    fn remaining_bytes(&self) -> Result<usize> {
        Ok(self.buf.remaining_bytes()?)
    }

    fn get_bits(&mut self, num_bits: usize) -> Result<u64> {
        Ok(self.buf.get_bits(num_bits)?)
    }

    fn get_byte(&mut self) -> Result<u8> {
        Ok(self.buf.get_byte()?)
    }

    pub fn get_bytes_ref(&mut self, len: usize) -> Result<&[u8]> {
        Ok(self.buf.get_bytes_ref(len)?)
    }
}

pub(crate) struct ProcCtx<'a, 'b, 'c, 'd> {
    mdb: &'a MissionDatabase,
    pdata: &'b mut ProcessorData,
    cbuf: ContainerBuf<'c>,
    result: ParameterValueList,
    pidx: Option<ParameterIdx>,
    /// values from the previous packets
    cache: &'d ParameterCache,
}

impl<'a> ProcCtx<'a, '_, '_, '_> {
    fn mdb(&mut self) -> &'a MissionDatabase {
        self.mdb
    }

    /// returns the parameter value instance: 0 is the last value, -1 the one before, etc.
    /// The values extracted from the current packet are considered first and then the ones from the cache.
    fn get_param_instance(&self, pidx: ParameterIdx, instance: i32) -> Option<&ParameterValue> {
        if instance > 0 {
            return None;
        }
        let k = instance.unsigned_abs() as usize;
        self.result.iter_param_rev(pidx).chain(self.cache.iter(pidx)).nth(k)
    }

    /// returns the value of the referenced parameter from the current context.
    /// Returns None if the value is not available.
    fn get_param_value(&self, para_ref: &ParameterInstanceRef) -> Option<&Value> {
        self.get_param_value_at(para_ref, para_ref.instance)
    }

    /// returns the value of the referenced parameter used in a match criteria.
    ///
    /// Like Yamcs, the instance is ignored: the last value from the current packet is used or, if the parameter
    /// has not been extracted from the current packet, the last value from the cache.
    fn get_criteria_value(&self, para_ref: &ParameterInstanceRef) -> Option<&Value> {
        self.get_param_value_at(para_ref, 0)
    }

    fn get_param_value_at(&self, para_ref: &ParameterInstanceRef, instance: i32) -> Option<&Value> {
        let pv = self.get_param_instance(para_ref.pidx, instance)?;
        let v = if para_ref.use_calibrated_value { &pv.eng_value } else { pv.raw_value.as_ref()? };
        Some(v).map_or(None, |val| {
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
    fn get_dynamic_uint_value(&self, dynpara: &DynamicValueType) -> Result<u64> {
        let para_ref = &dynpara.para_ref;
        //let para_name = self.mdb.name2str(self.mdb.get_parameter(para_ref.pidx).name());

        let para_name = || self.mdb.name2str(self.mdb.get_parameter(para_ref.pidx).name());

        let v = self.get_param_value(para_ref).ok_or_else(|| ProcError::MissingValue(format!(
            "Cannot find a value for parameter {} in the current context",
            para_name()
        )))?;

        if let Some(adj) = &dynpara.adjustment {
            //linear adjusment is with f64, convert everything to f64
            let x: f64 = v.try_into().map_err(|_| {
                ProcError::DecodingError(format!(
                    "Cannot convert value {:?} for parameter {} to f64 (double)",
                    v,
                    para_name()
                ))
            })?;
            let y = x * adj.slope + adj.intercept;
            Ok(y as u64)
        } else {
            let x: u64 = v.try_into().map_err(|_| {
                ProcError::DecodingError(format!(
                    "Cannot convert value {:?} for parameter {} to u64",
                    v,
                    para_name()
                ))
            })?;

            Ok(x as u64)
        }
    }

    /// returns the value of a fixed or dynamic integer value
    fn get_integer_value(&self, iv: &IntegerValue) -> Result<i64> {
        match iv {
            IntegerValue::FixedValue(v) => Ok(*v),
            IntegerValue::DynamicValue(dv) => self.get_dynamic_int_value(dv),
        }
    }

    /// returns the value of the dynamic value as a signed integer
    fn get_dynamic_int_value(&self, dynpara: &DynamicValueType) -> Result<i64> {
        let para_ref = &dynpara.para_ref;
        let para_name = || self.mdb.name2str(self.mdb.get_parameter(para_ref.pidx).name());
        let v = self.get_param_value(para_ref).ok_or_else(|| {
            ProcError::MissingValue(format!(
                "Cannot find a value for parameter {} in the current context",
                para_name()
            ))
        })?;
        let not_integer = || {
            ProcError::DecodingError(format!(
                "Cannot convert value {:?} for parameter {} to integer",
                v,
                para_name()
            ))
        };
        if let Some(adj) = &dynpara.adjustment {
            let x: f64 = v.try_into().map_err(|_| not_integer())?;
            Ok((x * adj.slope + adj.intercept) as i64)
        } else {
            match v {
                Value::Int64(x) => Ok(*x),
                Value::Uint64(x) => i64::try_from(*x).map_err(|_| not_integer()),
                _ => Err(not_integer()),
            }
        }
    }

    fn decoding_error(&self, msg: &str) -> ProcError {      
        if let Some(pidx) = self.pidx {
            return ProcError::DecodingError(format!(
                "Error decoding parameter {}: {}",
                self.mdb.name2str(self.mdb.get_parameter(pidx).name()),
                msg
            ));
        } else {
            return ProcError::DecodingError(msg.to_owned());
        }
    }
}
