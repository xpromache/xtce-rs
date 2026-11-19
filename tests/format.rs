//! Yamcs-like text of the values and calibrators (xtce_rs::format)
use std::{collections::HashMap, path::Path};

use xtce_rs::{
    format::{calibrator_to_string, eng_value_to_string, raw_value_to_string, value_to_string},
    mdb::{MissionDatabase, NamedItem},
    parser,
    value::{AggregateValue, ArrayValue, EnumeratedValue, Value},
};

fn load(f: &str) -> MissionDatabase {
    parser::parse_files(&[Path::new(f)]).unwrap()
}

#[test]
fn typed_values() {
    let mdb = load("test-xtce-files/simulator.xml");
    let dt = |n: &str| mdb.get_data_type(mdb.search_parameter_type(n).unwrap());

    // FLOAT type: printed with the float precision
    let f32t = dt("/YSS/SIMULATOR/float32");
    assert_eq!("0.3", eng_value_to_string(&mdb, f32t, &Value::Double(0.3f32 as f64)));
    assert_eq!("1.0E10", eng_value_to_string(&mdb, f32t, &Value::Double(1e10)));

    // aggregate: the members in the order of the type
    let pos = dt("/YSS/SIMULATOR/position");
    let name = |s: &str| mdb.name_db_ref().get(s).unwrap();
    let v = Value::Aggregate(Box::new(AggregateValue(HashMap::from([
        (name("altitude"), Value::Double(3.5)),
        (name("longitude"), Value::Double(1.0)),
        (name("latitude"), Value::Double(0.3f32 as f64)),
    ]))));
    assert_eq!("longitude : 1.0, latitude : 0.3, altitude : 3.5", eng_value_to_string(&mdb, pos, &v));
    // without the type: by name, as doubles
    assert_eq!(
        "altitude : 3.5, latitude : 0.30000001192092896, longitude : 1.0",
        value_to_string(&mdb, &v)
    );
}

#[test]
fn untyped_values() {
    let mdb = load("test-xtce-files/simulator.xml");
    assert_eq!("-5", value_to_string(&mdb, &Value::Int64(-5)));
    assert_eq!("18446744073709551615", value_to_string(&mdb, &Value::Uint64(u64::MAX)));
    assert_eq!("true", value_to_string(&mdb, &Value::Boolean(true)));
    assert_eq!("0AFF", value_to_string(&mdb, &Value::Binary(Box::new(vec![0x0a, 0xff]))));
    let ev = EnumeratedValue { key: 3, value: "ON".into() };
    assert_eq!("ON", value_to_string(&mdb, &Value::Enumerated(Box::new(ev))));
    let arr = ArrayValue { dims: vec![2], values: vec![Value::Double(1.0), Value::Double(0.5)] };
    assert_eq!("[1.0, 0.5]", value_to_string(&mdb, &Value::Array(Box::new(arr))));
    assert_eq!("1970-01-01T00:00:00.000Z", value_to_string(&mdb, &Value::Timestamp(xtce_rs::time::from_unix_millis(0))));
}

#[test]
fn raw_values_use_the_encoding_type() {
    let mdb = load("test-xtce-files/phase1.xml");
    let dt = mdb.get_data_type(mdb.search_parameter_type("/P1/poly_t").unwrap());
    // DOUBLE engineering type, integer encoding
    assert_eq!("12", raw_value_to_string(&mdb, dt, &Value::Uint64(12)));
    assert_eq!("0.30000001192092896", eng_value_to_string(&mdb, dt, &Value::Double(0.3f32 as f64)));
}

#[test]
fn calibrators() {
    let mdb = load("test-xtce-files/phase1.xml");
    let calib = |n: &str| {
        let dt = mdb.get_data_type(mdb.search_parameter_type(n).unwrap());
        calibrator_to_string(dt.calibrator.as_ref().unwrap())
    };
    assert_eq!("PolynomialCalibrator[-10.0, 0.5]", calib("/P1/poly_t"));
    // inherited from the base type
    assert_eq!("PolynomialCalibrator[-10.0, 0.5]", calib("/P1/poly_derived_t"));
    // the points are sorted by raw value, like Yamcs
    assert_eq!("SplineCalibrator[(0.0,0.0), (10.0,100.0), (20.0,150.0)]", calib("/P1/spline_t"));
    assert_eq!("MathOperationCalibrator[this 2.0 * 1.0 +]", calib("/P1/math_t"));
}

#[test]
fn ops_name() {
    let mdb = load("test-xtce-files/simulator.xml");
    let p = mdb.get_parameter(mdb.search_parameter("/YSS/SIMULATOR/BatteryVoltage2").unwrap());
    assert_eq!(Some("SIMULATOR_BatteryVoltage2"), p.name_descr().ops_name());
    let p = mdb.get_parameter(mdb.search_parameter("/YSS/SIMULATOR/BatteryVoltage1").unwrap());
    assert_eq!(None, p.name_descr().ops_name());
}
