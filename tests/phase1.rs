//! Tests for base types, calibrators, alarms, arrays, repeat entries, container ref entries,
//! indirect parameter entries, dynamic locations and boolean expressions.

use std::path::Path;

use xtce_rs::{
    mdb::{
        types::{AlarmLevel, Calibrator, DataType, NumericRange, TimeEpoch, TimeReference, TypeData},
        DataSource, MissionDatabase, NamedItem,
    },
    parser,
    proc::containers::process,
    pvlist::ParameterValueList,
    value::Value,
};

fn mdb() -> MissionDatabase {
    let mut mdb = MissionDatabase::new();
    parser::parse(&mut mdb, Path::new("test-xtce-files/phase1.xml")).unwrap();
    mdb
}

fn dtype<'a>(mdb: &'a MissionDatabase, name: &str) -> &'a DataType {
    mdb.get_data_type(mdb.search_parameter_type(&format!("/P1/{}", name)).unwrap())
}

/// returns (parameter name, eng value as string) for each value in the list
fn names_values(mdb: &MissionDatabase, r: ParameterValueList) -> Vec<(String, String)> {
    r.into_iter()
        .map(|pv| {
            let name = mdb.name2str(mdb.get_parameter(pv.pidx).name()).to_owned();
            (name, pv.eng_value.to_string())
        })
        .collect()
}

fn run(mdb: &MissionDatabase, packet: &[u8]) -> Vec<(String, String)> {
    let root = mdb.search_container("/P1/root").unwrap();
    names_values(mdb, process(mdb, packet, root).unwrap())
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn calibrators() {
    let mdb = mdb();
    let packet = [
        1, 200, // mode = ACTIVE, count = 200
        0x00, 0x28, // p_poly raw 40 -> -10 + 0.5*40
        0x00, 0x64, // p_poly_d raw 100 -> -10 + 0.5*100 (calibrator inherited from the base type)
        7,    // p_poly_int 0.4 * 7 = 2.8 -> rounded to 3
        15,   // p_spline: between (10,100) and (20,150)
        5,    // p_math: 2*5+1
        3,    // p_ctx: context calibrator (mode == ACTIVE) 10*x
        0xFE, 0xFF, // p_le: little endian twos complement -> -2
    ];
    let r = run(&mdb, &packet);
    assert_eq!(
        pairs(&[
            ("mode", "ACTIVE"),
            ("count", "200"),
            ("p_poly", "10"),
            ("p_poly_d", "40"),
            ("p_poly_int", "3"),
            ("p_spline", "125"),
            ("p_math", "11"),
            ("p_ctx", "30"),
            ("p_le", "-2"),
        ]),
        r
    );
}

#[test]
fn context_calibrator_default() {
    let mdb = mdb();
    // mode=OTHER: the calib container matches through the second branch of the OR (100 < count < 250)
    // and p_ctx is calibrated with the default calibrator (identity) because the context (mode == ACTIVE) does not match
    let packet = [5, 150, 0x00, 0x28, 0x00, 0x64, 7, 15, 5, 3, 0xFE, 0xFF];
    let r = run(&mdb, &packet);
    assert_eq!(("p_ctx".to_owned(), "3".to_owned()), r[7]);

    // count outside (100, 250): calib does not match
    let r = run(&mdb, &[5, 6]);
    assert_eq!(pairs(&[("mode", "OTHER"), ("count", "6")]), r);
}

#[test]
fn structures() {
    let mdb = mdb();
    let packet = [
        0, 3, // mode = IDLE, count = 3
        0x0A, 0xFF, 0x0B, 0xFF, 0x0C, 0xFF, // rep x 3, each followed by an 8 bits offset
        0x01, 0x02, // arr, size 2 given in the entry
        0x11, 0x12, 0x13, 0x14, // arr_dyn, ending index = count => size 4
        7,    // selector = 7 => target_b (alias id=7)
        0x01, 0x02, // target_b
        0x21, 0x22, 0x00, // included container inc (inc1, inc2) of size 24 bits
        0x33, // after_inc
        2,    // offset_param => dyn_loc is 2*8 bits after
        0xEE, 0xEE, 0x44, // dyn_loc
    ];
    let r = run(&mdb, &packet);
    assert_eq!(
        pairs(&[
            ("mode", "IDLE"),
            ("count", "3"),
            ("rep", "10"),
            ("rep", "11"),
            ("rep", "12"),
            ("arr", "[1, 2]"),
            ("arr_dyn", "[17, 18, 19, 20]"),
            ("selector", "7"),
            ("target_b", "258"),
            ("inc1", "33"),
            ("inc2", "34"),
            ("after_inc", "51"),
            ("offset_param", "2"),
            ("dyn_loc", "68"),
        ]),
        r
    );
}

#[test]
fn structures_truncated() {
    let mdb = mdb();
    let root = mdb.search_container("/P1/root").unwrap();
    // count = 200 repetitions but the packet is short
    assert!(process(&mdb, &[0, 200, 1, 2, 3], root).is_err());
    // selector pointing to a non existing alias
    assert!(process(&mdb, &[0, 0, 0x01, 0x02, 0x11, 9, 0, 0], root).is_err());
}

#[test]
fn parameter_to_parameter_condition() {
    let mdb = mdb();
    // raw mode (5) == count (5)
    let r = run(&mdb, &[5, 5, 0x42]);
    assert_eq!(pairs(&[("mode", "OTHER"), ("count", "5"), ("a", "66")]), r);

    let r = run(&mdb, &[5, 6, 0x42]);
    assert_eq!(pairs(&[("mode", "OTHER"), ("count", "6")]), r);
}

#[test]
fn model() {
    let mdb = mdb();

    // base type inheritance
    let le = dtype(&mdb, "int16_le");
    match &le.type_data {
        TypeData::Integer(idt) => {
            assert!(idt.signed);
            assert_eq!(32, idt.size_in_bits);
        }
        _ => panic!("expected integer"),
    }
    let derived = dtype(&mdb, "poly_derived_t");
    assert_eq!(Some("derived"), derived.ndescr.short_description.as_deref());
    assert_eq!("V", derived.units[0].unit);
    assert!(matches!(derived.calibrator, Some(Calibrator::Polynomial(_))));

    // valid range and alarms
    let poly = dtype(&mdb, "poly_t");
    let vr = poly.valid_range.as_ref().unwrap();
    assert!(vr.applies_to_calibrated);
    assert_eq!(Some(-10.0), vr.range.min_inclusive);
    assert_eq!(Some(100.0), vr.range.max_inclusive);
    match &poly.type_data {
        TypeData::Float(fdt) => {
            let alarm = fdt.default_alarm.as_ref().unwrap();
            assert_eq!(2, alarm.min_violations);
            assert_eq!(
                Some(NumericRange {
                    min_inclusive: Some(0.0),
                    max_inclusive: Some(50.0),
                    ..Default::default()
                }),
                alarm.static_alarm_ranges.warning
            );
            assert_eq!(Some(-5.0), alarm.static_alarm_ranges.critical.as_ref().unwrap().min_exclusive);
            assert_eq!(1, fdt.context_alarm.len());
            assert!(alarm.static_alarm_ranges.warning.as_ref().unwrap().contains(50.0));
            assert!(!alarm.static_alarm_ranges.critical.as_ref().unwrap().contains(80.0));
        }
        _ => panic!("expected float"),
    }
    match &dtype(&mdb, "mode_t").type_data {
        TypeData::Enumerated(edt) => {
            let alarm = edt.default_alarm.as_ref().unwrap();
            assert_eq!(vec![("OTHER".to_owned(), AlarmLevel::Warning)], alarm.alarms);
            assert_eq!(AlarmLevel::Normal, alarm.default_level);
        }
        _ => panic!("expected enumerated"),
    }

    // absolute time
    let t = dtype(&mdb, "time_t");
    match &t.type_data {
        TypeData::AbsoluteTime(atd) => {
            assert_eq!(0.001, atd.scale);
            assert_eq!(1.0, atd.offset);
            assert!(matches!(atd.reference, TimeReference::Epoch(TimeEpoch::Gps)));
        }
        _ => panic!("expected absolute time"),
    }
    assert!(matches!(t.encoding, xtce_rs::mdb::types::DataEncoding::Integer(_)));

    // arrays
    match &dtype(&mdb, "arr_t").type_data {
        TypeData::Array(at) => {
            assert_eq!(1, at.number_of_dimensions);
            assert!(at.dim.is_empty());
        }
        _ => panic!("expected array"),
    }

    // aliases and data source
    let pidx = mdb.get_parameter_by_alias("MDB:OPS Name", "TARGET_B").unwrap();
    assert_eq!(Some(pidx), mdb.search_parameter("/P1/target_b"));
    assert_eq!(2, mdb.get_parameter(pidx).ndescr.aliases.len());
    let local = mdb.get_parameter(mdb.search_parameter("/P1/local1").unwrap());
    assert!(matches!(local.data_source, DataSource::Local));

    // container size
    let inc = mdb.get_container(mdb.search_container("/P1/inc").unwrap());
    assert_eq!(Some(24), inc.size_in_bits);
}

#[test]
fn eng_value_types() {
    let mdb = mdb();
    let root = mdb.search_container("/P1/root").unwrap();
    let packet = [1, 200, 0x00, 0x28, 0x00, 0x64, 7, 15, 5, 3, 0xFE, 0xFF];
    let r = process(&mdb, &packet, root).unwrap();
    // float calibrated -> double, integer calibrated -> rounded unsigned integer
    assert_eq!(&Value::Double(10.0), r.eng(2));
    assert_eq!(Some(&Value::Uint64(40)), r.raw(2));
    assert_eq!(&Value::Uint64(3), r.eng(4));
    assert_eq!(&Value::Int64(-2), r.eng(8));
}

/// BogusSAT uses ContainerRefEntry, ArrayParameterRefEntry with RepeatEntry and many calibrators
#[test]
fn bogussat_loads() {
    let mut mdb = MissionDatabase::new();
    parser::parse(&mut mdb, Path::new("test-xtce-files/BogusSAT-2.xml")).unwrap();
    let n_cal = mdb.parameter_types.iter().filter(|dt| dt.calibrator.is_some()).count();
    let n_ctx = mdb.parameter_types.iter().filter(|dt| !dt.context_calibrators.is_empty()).count();
    assert!(n_cal >= 10, "only {} types with calibrators", n_cal);
    assert!(n_ctx >= 3, "only {} types with context calibrators", n_ctx);
}

/// CCSDS_SpacePacket4 from BogusSAT includes two containers with ContainerRefEntry, one of them conditionally
#[test]
fn bogussat_container_ref() {
    let mut mdb = MissionDatabase::new();
    parser::parse(&mut mdb, Path::new("test-xtce-files/BogusSAT-2.xml")).unwrap();
    let root = mdb.search_container("/BogusSAT/CCSDSPacket").unwrap();

    let mut packet = vec![
        0x00, 0x04, // version 0, type 0, no secondary header, APID 4
        0xC0, 0x00, // sequence flags 3, count 0
        0x00, 0x00, // length (not checked)
        0x01, // Payload_1_State = ON
    ];
    let included1 = [
        0x00, 0x00, 0x00, 0x01, // Basic_uint32 = 1
        0x80, 0x00, 0x00, 0x05, // Basic_int32_signmag = -5
        0xFF, 0xFF, 0xFF, 0xFB, // Basic_int32_twoscomp = -5
        0xFF, 0xFF, 0xFF, 0xFA, // Basic_int32_onescomp = -5
    ];
    let included2 = [
        0x3F, 0xC0, 0x00, 0x00, // Basic_Float32 = 1.5
        0x40, 0x04, 0, 0, 0, 0, 0, 0, // Basic_Float64 = 2.5
        0x00, 0x01, 0x00, 0x01, 0x00, 0x01, // three enum_int16
    ];
    packet.extend_from_slice(&included1);
    packet.extend_from_slice(&included2);

    let r = names_values(&mdb, process(&mdb, &packet, root).unwrap());
    let names: Vec<&str> = r.iter().map(|(n, _)| n.as_str()).collect();
    let pos = names.iter().position(|&n| n == "Payload_1_State").unwrap();
    assert_eq!(
        vec![
            "Payload_1_State",
            "Basic_uint32",
            "Basic_int32_signmag",
            "Basic_int32_twoscomp",
            "Basic_int32_onescomp",
            "Basic_Float32",
            "Basic_Float64",
            "enum_int16_signmag",
            "enum_int16_twoscomp",
            "enum_int16_onescomp",
        ],
        names[pos..]
    );
    let values: Vec<&str> = r[pos..pos + 7].iter().map(|(_, v)| v.as_str()).collect();
    assert_eq!(vec!["true", "1", "-5", "-5", "-5", "1.5", "2.5"], values);

    // Payload_1_State = OFF: the first included container is skipped
    packet[6] = 0;
    let mut p2 = packet[..7].to_vec();
    p2.extend_from_slice(&included2);
    let r = names_values(&mdb, process(&mdb, &p2, root).unwrap());
    let names: Vec<&str> = r.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"Basic_uint32"));
    assert!(names.contains(&"Basic_Float64"));
}
