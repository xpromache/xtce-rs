//! Stateful telemetry processing, similar to the Yamcs XtceTmProcessor:
//! container extraction, algorithms, validity and alarm checking and the cache of the previous values.

use std::collections::{HashMap, VecDeque};

use crate::{
    mdb::{
        types::{
            AlarmLevel, AlarmRanges, DataType, EnumerationAlarm, NumericAlarm, NumericRange,
            TypeData,
        },
        ContainerIdx, MissionDatabase, NamedItem, ParameterIdx,
    },
    proc::criteria_evaluator::MatchResult,
    pvlist::ParameterValueList,
    value::{MonitoringResult, ParameterValue, RangeCondition, Value},
};

use super::{
    calibrators, containers, types, ContainerBuf, ProcCtx, ProcError, ProcessorData, Result,
};

/// Keeps the last values of the parameters. For the parameters referenced with negative instances
/// (e.g. `instance="-2"`) it keeps enough values to resolve the references.
#[derive(Default)]
pub struct ParameterCache {
    /// the most recent values first
    values: HashMap<ParameterIdx, VecDeque<ParameterValue>>,
}

impl ParameterCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// returns the last value of the parameter
    pub fn last_value(&self, pidx: ParameterIdx) -> Option<&ParameterValue> {
        self.values.get(&pidx).and_then(|v| v.front())
    }

    /// iterates over the cached values of the parameter, the most recent first
    pub fn iter(&self, pidx: ParameterIdx) -> impl Iterator<Item = &ParameterValue> {
        self.values.get(&pidx).into_iter().flat_map(|v| v.iter())
    }

    /// adds the value to the cache, keeping maximum depth values for the parameter
    fn add(&mut self, pv: ParameterValue, depth: usize) {
        let v = self.values.entry(pv.pidx).or_default();
        v.push_front(pv);
        v.truncate(depth.max(1));
    }
}

/// Like in Yamcs, the cache initially contains the initial values of the parameters whose types define one
/// (e.g. the mission constants used in the include conditions).
fn initial_cache(mdb: &MissionDatabase) -> ParameterCache {
    let mut cache = ParameterCache::new();
    for (i, p) in mdb.parameters.iter().enumerate() {
        let Some(ptype) = p.ptype else { continue };
        let dtype = mdb.get_data_type(ptype);
        let Some(iv) = &dtype.initial_value else { continue };
        match crate::tc::args::to_eng_value(mdb, dtype, &Value::StringValue(Box::new(iv.clone()))) {
            Ok(v) => cache.add(ParameterValue::with_raw(ParameterIdx::new(i), None, v), 1),
            Err(e) => log::warn!("Invalid initial value '{}' for parameter {}: {}", iv, mdb.name2str(p.name()), e),
        }
    }
    cache
}

/// Options for [TmProcessor::process_with].
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    /// collect the positions of the extracted values, the matched containers and the alarm ranges
    /// (see [TmResult]); off by default because it costs some time and memory
    pub details: bool,
    /// run the algorithms triggered by the extracted parameters (on by default)
    pub algorithms: bool,
    /// keep the values in the cache of the previous values (on by default); off to decode packets
    /// independently of each other, in any order
    pub update_cache: bool,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        ProcessOptions { details: false, algorithms: true, update_cache: true }
    }
}

/// Where a parameter value was extracted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryPosition {
    /// the container which contains the entry (for an inherited entry: the base container defining it)
    pub container: ContainerIdx,
    /// the offset in bytes in the packet where that container starts
    /// (non zero for containers included with a ContainerRefEntry)
    pub container_start: u32,
    /// the offset in bits from the beginning of the packet
    pub bit_offset: u32,
    /// the number of bits extracted
    pub bit_size: u32,
}

/// A container extracted from the packet: the root, the matching inheriting containers and the included
/// containers, in the order of extraction (a container is listed before the ones inheriting from it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchedContainer {
    pub container: ContainerIdx,
    /// the offset in bytes in the packet where the container starts
    pub start: u32,
}

#[derive(Default)]
pub(crate) struct Details {
    pub(crate) positions: Vec<Option<EntryPosition>>,
    pub(crate) containers: Vec<MatchedContainer>,
    pub(crate) alarm_ranges: Vec<Option<AlarmRanges>>,
}

/// The result of [TmProcessor::process_with].
pub struct TmResult {
    pub params: ParameterValueList,
    /// with [ProcessOptions::details]: for each value in `params` (same index), where it was extracted from;
    /// None for the values not extracted from the packet (algorithm outputs). Empty otherwise.
    pub positions: Vec<Option<EntryPosition>>,
    /// with [ProcessOptions::details]: the extracted containers. Empty otherwise.
    pub containers: Vec<MatchedContainer>,
    /// with [ProcessOptions::details]: for each value in `params`, the static alarm ranges applied to it
    /// (the context alarm matching, or else the default alarm); None if it has no numeric alarm. Empty otherwise.
    pub alarm_ranges: Vec<Option<AlarmRanges>>,
}

impl TmResult {
    /// the packet name like in the Yamcs packet viewer: the last container extracted starting at the
    /// beginning of the packet (the most specialized one). Requires [ProcessOptions::details].
    pub fn packet_container(&self) -> Option<ContainerIdx> {
        self.containers.iter().filter(|c| c.start == 0).last().map(|c| c.container)
    }
}

/// Telemetry processor: extracts the parameters from packets and keeps the state required for
/// the references to previous parameter values and for the algorithms.
pub struct TmProcessor<'a> {
    mdb: &'a MissionDatabase,
    pdata: ProcessorData,
    cache: ParameterCache,
    /// the last known values of the inputs for each algorithm (missing inputs keep their previous value)
    algo_inputs: Vec<Vec<f64>>,
}

impl<'a> TmProcessor<'a> {
    pub fn new(mdb: &'a MissionDatabase) -> Result<Self> {
        let pdata = ProcessorData::new(mdb)?;
        let algo_inputs = mdb.algorithms.iter().map(|a| vec![0.0; a.inputs.len()]).collect();
        Ok(TmProcessor { mdb, pdata, cache: initial_cache(mdb), algo_inputs })
    }

    pub fn cache(&self) -> &ParameterCache {
        &self.cache
    }

    /// Processes a packet starting from the given root container.
    ///
    /// The generation and acquisition times (instants, see [crate::time]) are set on all the returned values.
    ///
    /// In addition to the parameters extracted from the packet, the result contains the outputs of the
    /// algorithms triggered by the extracted parameters. The values are checked against their valid ranges
    /// and alarms; the cache of the previous values is updated.
    pub fn process(
        &mut self,
        packet: &[u8],
        root_container: ContainerIdx,
        generation_time: i64,
        acquisition_time: i64,
    ) -> Result<ParameterValueList> {
        let opts = ProcessOptions::default();
        Ok(self.process_with(packet, root_container, generation_time, acquisition_time, &opts)?.params)
    }

    /// Like [TmProcessor::process], with options; see [ProcessOptions] and [TmResult].
    pub fn process_with(
        &mut self,
        packet: &[u8],
        root_container: ContainerIdx,
        generation_time: i64,
        acquisition_time: i64,
        opts: &ProcessOptions,
    ) -> Result<TmResult> {
        if packet.len() > containers::MAX_PACKET_SIZE {
            return Err(ProcError::OutOfBounds(format!(
                "Packet too long: {} bytes; max size is {}",
                packet.len(),
                containers::MAX_PACKET_SIZE
            )));
        }
        let mut ctx = ProcCtx {
            mdb: self.mdb,
            pdata: &mut self.pdata,
            cbuf: ContainerBuf::new(packet),
            result: ParameterValueList::new(),
            pidx: None,
            cache: &self.cache,
            details: if opts.details { Some(Details::default()) } else { None },
            current_container: None,
        };
        containers::extract(&mut ctx, root_container)?;

        for i in 0..ctx.result.len() {
            let pv = ctx.result.get_mut(i);
            pv.generation_time = generation_time;
            pv.acquisition_time = acquisition_time;
        }

        if opts.algorithms {
            run_algorithms(&mut ctx, &mut self.algo_inputs, generation_time, acquisition_time);
        }
        check_values(&mut ctx);

        let details = ctx.details.take();
        let result = ctx.result;
        if opts.update_cache {
            for pv in result.iter() {
                let depth = self.mdb.history_depth.get(&pv.pidx).copied().unwrap_or(0) + 1;
                self.cache.add(pv.clone(), depth);
            }
        }

        let Details { mut positions, containers, mut alarm_ranges } = details.unwrap_or_default();
        if opts.details {
            positions.resize(result.len(), None);
            alarm_ranges.resize(result.len(), None);
        }
        Ok(TmResult { params: result, positions, containers, alarm_ranges })
    }
}

/// Runs the algorithms triggered by the parameters in the result.
///
/// An algorithm is triggered if one of its triggers (or if it has no trigger, one of its inputs) is part of the result.
/// The outputs are added to the result and can trigger other algorithms; each algorithm runs at most once.
fn run_algorithms(
    ctx: &mut ProcCtx,
    algo_inputs: &mut [Vec<f64>],
    generation_time: i64,
    acquisition_time: i64,
) {
    let mdb = ctx.mdb;
    let mut executed = vec![false; mdb.algorithms.len()];
    loop {
        let mut progress = false;
        for (i, algo) in mdb.algorithms.iter().enumerate() {
            if executed[i] {
                continue;
            }
            let triggered = if algo.triggers.is_empty() {
                algo.inputs.iter().any(|r| ctx.result.contains(r.pidx))
            } else {
                algo.triggers.iter().any(|p| ctx.result.contains(*p))
            };
            if !triggered {
                continue;
            }
            executed[i] = true;
            progress = true;

            let inputs = &mut algo_inputs[i];
            for (k, iref) in algo.inputs.iter().enumerate() {
                if let Some(x) = ctx.get_param_value(iref).and_then(|v| f64::try_from(v).ok()) {
                    inputs[k] = x;
                }
            }

            let name = || mdb.name2str(algo.name());
            let y = match calibrators::math_operation(&algo.expression, 0.0, inputs) {
                Ok(y) => y,
                Err(e) => {
                    log::warn!("Error executing algorithm {}: {}", name(), e);
                    continue;
                }
            };
            let param = mdb.get_parameter(algo.output);
            let Some(dtype) = param.ptype.map(|t| mdb.get_data_type(t)) else {
                log::warn!("Algorithm {}: the output parameter has no type", name());
                continue;
            };
            match types::from_double(y, dtype) {
                Ok(eng) => {
                    let mut pv = ParameterValue::with_raw(algo.output, None, eng);
                    pv.generation_time = generation_time;
                    pv.acquisition_time = acquisition_time;
                    ctx.result.push(pv);
                }
                Err(e) => log::warn!("Algorithm {}: cannot convert the output {}: {}", name(), y, e),
            }
        }
        if !progress {
            break;
        }
    }
}

/// result of checking one parameter value
struct CheckResult {
    invalid: bool,
    monitoring: Option<(MonitoringResult, Option<RangeCondition>)>,
}

/// checks the values in the result against their valid ranges and alarms
fn check_values(ctx: &mut ProcCtx) {
    let mdb = ctx.mdb;
    let mut results = Vec::with_capacity(ctx.result.len());
    let mut ranges = Vec::new();
    for (i, pv) in ctx.result.iter().enumerate() {
        let param = mdb.get_parameter(pv.pidx);
        let Some(dtype) = param.ptype.map(|t| mdb.get_data_type(t)) else {
            continue;
        };
        let invalid = !is_valid(pv, dtype);
        let monitoring = check_alarms(pv, dtype, ctx);
        if invalid || monitoring.is_some() {
            results.push((i, CheckResult { invalid, monitoring }));
        }
        if ctx.details.is_some() {
            if let Some(alarm) = numeric_alarm(dtype, ctx) {
                ranges.push((i, alarm.static_alarm_ranges.clone()));
            }
        }
    }
    if let Some(details) = ctx.details.as_mut() {
        for (i, r) in ranges {
            details.alarm_ranges.resize(i, None);
            details.alarm_ranges.push(Some(r));
        }
    }
    for (i, cr) in results {
        let pv = ctx.result.get_mut(i);
        if cr.invalid {
            pv.status.set_invalid();
        }
        if let Some((mr, rc)) = cr.monitoring {
            pv.monitoring_result = Some(mr);
            pv.range_condition = rc;
        }
    }
}

/// verifies the value against the valid range of the type (only for integer and float types)
fn is_valid(pv: &ParameterValue, dtype: &DataType) -> bool {
    if !matches!(dtype.type_data, TypeData::Integer(_) | TypeData::Float(_)) {
        return true;
    }
    let Some(vr) = &dtype.valid_range else {
        return true;
    };
    let v = if vr.applies_to_calibrated { Some(&pv.eng_value) } else { pv.raw_value.as_ref() };
    match v.and_then(|v| f64::try_from(v).ok()) {
        Some(x) => vr.range.contains(x),
        None => true,
    }
}

/// checks the alarms; returns None if the type has no alarm defined
fn check_alarms(
    pv: &ParameterValue,
    dtype: &DataType,
    ctx: &ProcCtx,
) -> Option<(MonitoringResult, Option<RangeCondition>)> {
    let matches = |mc| ctx.pdata.get_criteria_evaluator(mc).evaluate(ctx) == MatchResult::OK;
    match &dtype.type_data {
        TypeData::Integer(_) | TypeData::Float(_) => {
            check_numeric_alarm(&pv.eng_value, numeric_alarm(dtype, ctx)?)
        }
        TypeData::Enumerated(edt) => {
            let alarm = edt
                .context_alarm
                .iter()
                .find(|ca| matches(ca.context))
                .map(|ca| &ca.alarm)
                .or(edt.default_alarm.as_ref())?;
            check_enumeration_alarm(&pv.eng_value, alarm)
        }
        _ => None,
    }
}

/// the numeric alarm applicable in the current context: the first matching context alarm or else the default one
fn numeric_alarm<'a>(dtype: &'a DataType, ctx: &ProcCtx) -> Option<&'a NumericAlarm> {
    let matches = |mc| ctx.pdata.get_criteria_evaluator(mc).evaluate(ctx) == MatchResult::OK;
    match &dtype.type_data {
        TypeData::Integer(idt) => idt
            .context_alarm
            .iter()
            .find(|ca| matches(ca.context))
            .map(|ca| &ca.alarm)
            .or(idt.default_alarm.as_ref()),
        TypeData::Float(fdt) => fdt
            .context_alarm
            .iter()
            .find(|ca| matches(ca.context))
            .map(|ca| &ca.alarm)
            .or(fdt.default_alarm.as_ref()),
        _ => None,
    }
}

/// checks the ranges in the order severe, critical, distress, warning, watch; the first violated range gives the result
fn check_numeric_alarm(
    v: &Value,
    alarm: &NumericAlarm,
) -> Option<(MonitoringResult, Option<RangeCondition>)> {
    let x = f64::try_from(v).ok()?;
    let AlarmRanges { watch, warning, distress, critical, severe } = &alarm.static_alarm_ranges;
    let ranges = [
        (severe, MonitoringResult::Severe),
        (critical, MonitoringResult::Critical),
        (distress, MonitoringResult::Distress),
        (warning, MonitoringResult::Warning),
        (watch, MonitoringResult::Watch),
    ];
    for (range, mr) in ranges {
        if let Some(r) = range {
            if !r.contains(x) {
                return Some((mr, Some(range_condition(r, x))));
            }
        }
    }
    Some((MonitoringResult::InLimits, None))
}

/// LOW if the value is below the minimum of the range, HIGH otherwise
fn range_condition(r: &NumericRange, x: f64) -> RangeCondition {
    let below = r.min_inclusive.is_some_and(|m| x < m) || r.min_exclusive.is_some_and(|m| x <= m);
    if below {
        RangeCondition::Low
    } else {
        RangeCondition::High
    }
}

fn check_enumeration_alarm(
    v: &Value,
    alarm: &EnumerationAlarm,
) -> Option<(MonitoringResult, Option<RangeCondition>)> {
    let Value::Enumerated(ev) = v else {
        return None;
    };
    let level = alarm
        .alarms
        .iter()
        .find(|(label, _)| *label == ev.value)
        .map_or(alarm.default_level, |(_, level)| *level);
    let mr = match level {
        AlarmLevel::Normal => MonitoringResult::InLimits,
        AlarmLevel::Watch => MonitoringResult::Watch,
        AlarmLevel::Warning => MonitoringResult::Warning,
        AlarmLevel::Distress => MonitoringResult::Distress,
        AlarmLevel::Critical => MonitoringResult::Critical,
        AlarmLevel::Severe => MonitoringResult::Severe,
    };
    Some((mr, None))
}
