//! Tests for the Yamcs compatible decoding semantics: value types, absolute times,
//! parameter instances, math algorithms, validity and alarm checking.

use std::path::Path;

use xtce_rs::{
    mdb::{MissionDatabase, NamedItem},
    parser,
    proc::TmProcessor,
    pvlist::ParameterValueList,
    time,
    value::{MonitoringResult, ParameterValue, RangeCondition, Value},
};

fn mdb() -> MissionDatabase {
    let mut mdb = MissionDatabase::new();
    parser::parse(&mut mdb, Path::new("test-xtce-files/phase2.xml")).unwrap();
    mdb
}

fn process(tmp: &mut TmProcessor, mdb: &MissionDatabase, packet: &[u8]) -> ParameterValueList {
    let root = mdb.search_container("/P2/root").unwrap();
    tmp.process(packet, root, 1000, 2000).unwrap()
}

/// returns the last value of the parameter with the given name
fn get<'a>(mdb: &MissionDatabase, r: &'a ParameterValueList, name: &str) -> Option<&'a ParameterValue> {
    let pidx = mdb.search_parameter(&format!("/P2/{}", name)).unwrap();
    r.iter_param_rev(pidx).next()
}

fn eng(mdb: &MissionDatabase, r: &ParameterValueList, name: &str) -> Value {
    get(mdb, r, name).unwrap_or_else(|| panic!("no value for {}", name)).eng_value.clone()
}

#[test]
fn value_types() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let packet = [
        1, // pkt_id
        0, 0, 0, 1, 0, 0, 0, 5, // u32: 64 bits raw truncated to 32 bits
        0xFF, 0xFF, 0xFF, 0xFF, // s32: unsigned raw cast to signed 32 bits
        5,    // r_pos: 0.5*5 = 2.5 -> 3
        0xFB, // r_neg: 0.5*-5 = -2.5 -> -2 (Math.round)
        3,    // ur: 3-10 = -7 -> 0
        3,    // f32: 0.1*3 rounded to single precision
    ];
    let r = process(&mut tmp, &mdb, &packet);
    assert_eq!(Some(&Value::Uint64(0x1_0000_0005)), get(&mdb, &r, "u32").unwrap().raw_value.as_ref());
    assert_eq!(Value::Uint64(5), eng(&mdb, &r, "u32"));
    assert_eq!(Value::Int64(-1), eng(&mdb, &r, "s32"));
    assert_eq!(Value::Int64(3), eng(&mdb, &r, "r_pos"));
    assert_eq!(Value::Int64(-2), eng(&mdb, &r, "r_neg"));
    assert_eq!(Value::Uint64(0), eng(&mdb, &r, "ur"));
    assert_eq!(Value::Double(0.3f32 as f64), eng(&mdb, &r, "f32"));

    // the Yamcs value types
    let vt = |name: &str| {
        let p = mdb.get_parameter(mdb.search_parameter(&format!("/P2/{}", name)).unwrap());
        mdb.get_data_type(p.ptype.unwrap()).eng_value_type()
    };
    use xtce_rs::value::ValueType::*;
    assert_eq!(Uint32, vt("u32"));
    assert_eq!(Sint32, vt("s32"));
    assert_eq!(Float, vt("f32"));
    assert_eq!(Double, vt("volt"));
    assert_eq!(Timestamp, vt("t_gps"));
}

#[test]
fn absolute_times() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let mut packet = vec![2];
    packet.extend_from_slice(&2000u32.to_be_bytes()); // t_gps: 1 + 0.001*2000 seconds after the GPS epoch
    packet.extend_from_slice(&1_700_000_000u32.to_be_bytes()); // t_unix
    packet.extend_from_slice(&1.5f32.to_be_bytes()); // t_date: 1.5 seconds after 2020-01-01
    packet.push(10); // t_rel: 0.5*10 seconds after t_unix

    let r = process(&mut tmp, &mdb, &packet);
    let ts = |name| match eng(&mdb, &r, name) {
        Value::Timestamp(t) => time::to_string(t),
        v => panic!("not a timestamp: {:?}", v),
    };
    assert_eq!("1980-01-06T00:00:03.000Z", ts("t_gps"));
    assert_eq!("2023-11-14T22:13:20.000Z", ts("t_unix"));
    assert_eq!("2020-01-01T00:00:01.500Z", ts("t_date"));
    assert_eq!("2023-11-14T22:13:25.000Z", ts("t_rel"));
    assert_eq!(Value::Timestamp(time::from_unix_millis(1_700_000_000_000)), eng(&mdb, &r, "t_unix"));
}

#[test]
fn times_are_set() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let r = process(&mut tmp, &mdb, &[3, 0, 50, 1]);
    assert!(!r.is_empty());
    for pv in r.iter() {
        assert_eq!(1000, pv.generation_time);
        assert_eq!(2000, pv.acquisition_time);
    }
}

#[test]
fn alarms() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let mut check = |state: u8, volt_raw: u8| {
        let r = process(&mut tmp, &mdb, &[3, state, volt_raw, 0]);
        let volt = get(&mdb, &r, "volt").unwrap().clone();
        let state = get(&mdb, &r, "state").unwrap().clone();
        (state.monitoring_result, volt.monitoring_result, volt.range_condition, volt.status.is_invalid())
    };
    use MonitoringResult::*;

    // NOMINAL is explicitly normal; 5.0 is inside all the ranges
    assert_eq!((Some(InLimits), Some(InLimits), None, false), check(0, 50));
    // FAILED is critical; 9.5 violates the watch and warning ranges -> the most severe is reported
    assert_eq!((Some(Critical), Some(Warning), Some(RangeCondition::High), false), check(2, 95));
    // 0.3 violates the critical range (exclusive 0.5)
    assert_eq!((Some(InLimits), Some(Critical), Some(RangeCondition::Low), false), check(0, 3));
    // SAFE takes the default level (watch); the context alarm replaces the default alarm for volt
    assert_eq!((Some(Watch), Some(Warning), Some(RangeCondition::High), false), check(1, 50));
    assert_eq!((Some(Watch), Some(InLimits), None, false), check(1, 30));
    // raw 250 is outside of the valid range [0, 200] (applies to raw): the value is kept but marked invalid
    let (_, mr, rc, invalid) = check(0, 250);
    assert_eq!((Some(Critical), Some(RangeCondition::High), true), (mr, rc, invalid));
}

#[test]
fn no_alarm_no_monitoring_result() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let r = process(&mut tmp, &mdb, &[3, 0, 50, 7]);
    let counter = get(&mdb, &r, "counter").unwrap();
    assert_eq!(None, counter.monitoring_result);
    assert!(!counter.status.is_invalid());
}

#[test]
fn algorithms() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();

    let r = process(&mut tmp, &mdb, &[3, 0, 50, 7]);
    // the previous instance of counter is not available: the input keeps its initial value 0
    assert_eq!(Value::Int64(7), eng(&mdb, &r, "counter_delta"));
    assert_eq!(Value::Double(12.0), eng(&mdb, &r, "sum"));
    // chained algorithm, executed after sum even if defined before it
    assert_eq!(Value::Double(24.0), eng(&mdb, &r, "sum_x2"));
    let sum = get(&mdb, &r, "sum").unwrap();
    assert_eq!(None, sum.raw_value);
    assert_eq!(1000, sum.generation_time);

    let r = process(&mut tmp, &mdb, &[3, 0, 20, 10]);
    assert_eq!(Value::Int64(3), eng(&mdb, &r, "counter_delta"));
    assert_eq!(Value::Double(12.0), eng(&mdb, &r, "sum"));

    // a packet without the inputs does not trigger the algorithms
    let r = process(&mut tmp, &mdb, &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert!(get(&mdb, &r, "sum").is_none());
    assert!(get(&mdb, &r, "counter_delta").is_none());

    // the outputs are in the cache
    let sum_idx = mdb.search_parameter("/P2/sum").unwrap();
    assert_eq!(Value::Double(12.0), tmp.cache().last_value(sum_idx).unwrap().eng_value);
}

#[test]
fn previous_instances() {
    let mdb = mdb();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let names = |r: &ParameterValueList| -> Vec<String> {
        r.iter().map(|pv| mdb.name2str(mdb.get_parameter(pv.pidx).name()).to_owned()).collect()
    };

    // like in Yamcs, the instance is ignored in the include condition: the current counter is used
    let r = process(&mut tmp, &mdb, &[4, 0, 99]);
    assert!(!names(&r).contains(&"volt_prev".to_owned()));
    let r = process(&mut tmp, &mdb, &[4, 5, 99]);
    assert_eq!(Value::Uint64(99), eng(&mdb, &r, "volt_prev"));
    let r = process(&mut tmp, &mdb, &[4, 7, 99]);
    assert_eq!(Value::Uint64(99), eng(&mdb, &r, "volt_prev"));
    // counter_delta uses the previous counter as well
    assert_eq!(Value::Int64(2), eng(&mdb, &r, "counter_delta"));
}

#[test]
fn stateless_process_has_no_history() {
    let mdb = mdb();
    let root = mdb.search_container("/P2/root").unwrap();
    for _ in 0..2 {
        let r = xtce_rs::proc::containers::process(&mdb, &[4, 7, 99], root).unwrap();
        // the previous counter is not available
        assert_eq!(Value::Int64(7), eng(&mdb, &r, "counter_delta"));
        assert_eq!(xtce_rs::time::INVALID_INSTANT, r[0].generation_time);
    }
}
