//! Tests for the command encoding

use std::{collections::HashMap, path::Path};

use xtce_rs::{
    mdb::{MissionDatabase, NamedItem},
    parser,
    tc::{all_arguments, encode_command, encode_command_with, EncodeOptions, EncodedCommand, TcError},
    value::{AggregateValue, ArrayValue, Value},
};

fn load(f: &str) -> MissionDatabase {
    parser::parse_files(&[Path::new(f)]).unwrap()
}

fn s(v: &str) -> Value {
    Value::StringValue(Box::new(v.to_owned()))
}

fn args(a: &[(&str, Value)]) -> HashMap<String, Value> {
    a.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn encode(mdb: &MissionDatabase, cmd: &str, a: &[(&str, Value)]) -> Result<EncodedCommand, TcError> {
    let cmd = mdb.search_command(cmd).unwrap_or_else(|| panic!("no command {}", cmd));
    encode_command(mdb, cmd, &args(a))
}

fn hex(mdb: &MissionDatabase, cmd: &str, a: &[(&str, Value)]) -> String {
    hex::encode(encode(mdb, cmd, a).unwrap().binary)
}

#[test]
fn yamcs_simulator_commands() {
    let mdb = load("test-xtce-files/simulator.xml");
    // CCSDS header with apid 100, the secondary header with timeId=1, checksumIndicator=1 (initial value),
    // packet-type=10, packet-id=1 (assigned by SWITCH_VOLTAGE_ON) followed by voltage_num
    assert_eq!(
        "1864c000000000000000006a0000000101",
        hex(&mdb, "/YSS/SIMULATOR/SWITCH_VOLTAGE_ON", &[("voltage_num", s("1"))])
    );
    assert_eq!(
        "1864c000000000000000006a0000000103",
        hex(&mdb, "/YSS/SIMULATOR/SWITCH_VOLTAGE_ON_ENUM", &[("voltage_num", s("BACKUP"))])
    );
    // null terminated string and the initial values of the other arguments
    assert_eq!(
        "1864c000000000000000006a000000080000000b612e74787400ffffffffffffffffffffffffffffffff",
        hex(&mdb, "/YSS/SIMULATOR/DOWNLOAD_FILE", &[("filename", s("a.txt"))])
    );

    // the valid range applies to the raw value
    let e = encode(&mdb, "/YSS/SIMULATOR/SWITCH_VOLTAGE_ON", &[("voltage_num", s("4"))]).unwrap_err();
    assert!(matches!(e, TcError::InvalidArgument { .. }), "{:?}", e);
    let e = encode(&mdb, "/YSS/SIMULATOR/SWITCH_VOLTAGE_ON", &[]).unwrap_err();
    assert_eq!(TcError::MissingArgument("voltage_num".to_owned()), e);
    let e = encode(&mdb, "/YSS/SIMULATOR/SIM_TC", &[]).unwrap_err();
    assert_eq!(TcError::AbstractCommand("SIM_TC".to_owned()), e);
    let e = encode(&mdb, "/YSS/SIMULATOR/SWITCH_VOLTAGE_ON", &[("voltage_num", s("1")), ("packet-id", s("5"))])
        .unwrap_err();
    assert_eq!(TcError::AssignedArgument("packet-id".to_owned()), e);
}

#[test]
fn all_files_load() {
    for f in ["BogusSAT-2", "ref-xtce", "simulator", "strings-tm", "phase3"] {
        let mdb = load(&format!("test-xtce-files/{}.xml", f));
        assert!(!mdb.commands.is_empty(), "{}", f);
    }
}

#[test]
fn numeric_arguments() {
    let mdb = load("test-xtce-files/phase3.xml");
    let a = [
        ("s16le", Value::Int64(-2)),
        ("sm8", s("-5")),
        ("f32", Value::Double(1.5)),
        ("volt", s("5.25")), // raw = (5.25 + 10) / 0.5 = 30.5 -> 31 (Math.round)
        ("temp", Value::Double(125.0)), // spline reversed: raw 15
    ];
    let enc = encode(&mdb, "/P3/CMD_NUMERIC", &a).unwrap();
    // header: 11 + apid 100 (11 bits) + flag 1 + type_id 1 (8 bits) + 00, then the arguments
    assert_eq!("c32404feff853fc00000001f0f01", hex::encode(&enc.binary));

    let volt = enc.arg(&mdb, "volt").unwrap();
    assert_eq!(Value::Double(5.25), volt.eng_value);
    assert_eq!(Value::Uint64(31), volt.raw_value);
    let mode = enc.arg(&mdb, "mode").unwrap();
    assert_eq!("ON", mode.eng_value.to_string());
    assert_eq!(Value::Int64(1), mode.raw_value);
    // the order: base command arguments first
    let names: Vec<&str> = enc.args.iter().map(|a| mdb.name2str(a.name)).collect();
    assert_eq!(vec!["apid", "flag", "type_id", "s16le", "sm8", "f32", "volt", "temp", "mode"], names);

    let enc = encode(&mdb, "/P3/CMD_NUMERIC", &[&a[..], &[("mode", s("STANDBY"))]].concat()).unwrap();
    assert_eq!(0x05, *enc.binary.last().unwrap());
}

#[test]
fn argument_errors() {
    let mdb = load("test-xtce-files/phase3.xml");
    let ok = [
        ("s16le", Value::Int64(-2)),
        ("sm8", s("-5")),
        ("f32", Value::Double(1.5)),
        ("volt", s("5.25")),
        ("temp", Value::Double(125.0)),
    ];
    let with = |name: &'static str, v: Value| {
        let mut a: Vec<(&str, Value)> = ok.iter().filter(|(n, _)| *n != name).cloned().collect();
        a.push((name, v));
        encode(&mdb, "/P3/CMD_NUMERIC", &a)
    };
    let arg_error = |r: Result<EncodedCommand, TcError>, arg: &str| match r {
        Err(TcError::InvalidArgument { name, msg }) => {
            assert_eq!(arg, name, "{}", msg);
            msg
        }
        r => panic!("unexpected {:?}", r),
    };

    // outside of the type range
    arg_error(with("s16le", Value::Int64(40000)), "s16le");
    // -128 fits in the type but not in the sign magnitude encoding
    let msg = arg_error(with("sm8", Value::Int64(-128)), "sm8");
    assert!(msg.contains("does not fit"), "{}", msg);
    // the valid range applies to the engineering value
    arg_error(with("volt", Value::Double(60.0)), "volt");
    arg_error(with("mode", s("FOO")), "mode");
    arg_error(with("f32", s("abc")), "f32");

    assert_eq!(Err(TcError::UnknownArgument("foo".to_owned())), with("foo", Value::Int64(1)).map(|_| ()));
    assert_eq!(Err(TcError::AssignedArgument("type_id".to_owned())), with("type_id", Value::Int64(1)).map(|_| ()));
    let missing: Vec<(&str, Value)> = ok.iter().skip(1).cloned().collect();
    assert_eq!(
        Err(TcError::MissingArgument("s16le".to_owned())),
        encode(&mdb, "/P3/CMD_NUMERIC", &missing).map(|_| ())
    );

    // decalibration with a polynomial of degree 2 is not supported
    let e = encode(&mdb, "/P3/CMD_POLY2", &[("x", Value::Double(4.0))]).unwrap_err();
    assert!(matches!(e, TcError::Unsupported(_)), "{:?}", e);
}

#[test]
fn structured_arguments() {
    let mdb = load("test-xtce-files/phase3.xml");
    let name = |n: &str| mdb.name_db_ref().get(n).unwrap();
    let pos = Value::Aggregate(Box::new(AggregateValue(HashMap::from([(name("y"), Value::Int64(-1))]))));
    let arr = Value::Array(Box::new(ArrayValue {
        dims: vec![3],
        values: vec![Value::Uint64(1), s("2"), Value::Int64(3)],
    }));
    let a = vec![
        ("name_fixed", s("ab")),
        ("name_var", s("xyz")),
        ("blob", s("0102")),
        ("t", s("1980-01-06T00:00:01.500Z")),
        ("pos", pos.clone()),
        ("arr", arr.clone()),
    ];
    // header with apid 200 and type_id 2, "ab" padded to 4 bytes, "xyz" with a size tag, 0x0102 with a 16 bits
    // size tag, 1500 ms since the GPS epoch, pos.x from its initial value, the array,
    // the TRAILER container padded to 32 bits and the repeated argument
    assert_eq!(
        "c64408616200000378797a00020102000005dc01ff010203ab0000000707",
        hex(&mdb, "/P3/CMD_STRUCT", &a)
    );

    let with = |n: &'static str, v: Value| {
        let mut a1: Vec<(&str, Value)> = a.iter().filter(|(k, _)| *k != n).cloned().collect();
        a1.push((n, v));
        encode(&mdb, "/P3/CMD_STRUCT", &a1)
    };
    let is_arg_error = |r: Result<EncodedCommand, TcError>| matches!(r, Err(TcError::InvalidArgument { .. }));
    // too long for the fixed size
    assert!(is_arg_error(with("name_fixed", s("abcde"))));
    // SizeRangeInCharacters max 5
    assert!(is_arg_error(with("name_var", s("abcdef"))));
    // SizeRangeInBytes max 4
    assert!(is_arg_error(with("blob", s("0102030405"))));
    // the array type has 3 elements
    let arr2 = Value::Array(Box::new(ArrayValue { dims: vec![2], values: vec![Value::Uint64(1), Value::Uint64(2)] }));
    assert!(is_arg_error(with("arr", arr2)));
    // y has no initial value
    let pos2 = Value::Aggregate(Box::new(AggregateValue(HashMap::from([(name("x"), Value::Int64(1))]))));
    assert!(is_arg_error(with("pos", pos2)));
    assert!(is_arg_error(with("t", s("yesterday"))));
}

#[test]
fn location_and_parameters() {
    let mdb = load("test-xtce-files/phase3.xml");
    // the argument is placed at bit 40 from the start of the container
    assert_eq!("c3240c000009", hex(&mdb, "/P3/CMD_LOCATION", &[("a", Value::Uint64(9))]));

    let cmd = mdb.search_command("/P3/CMD_PARAM").unwrap();
    let e = encode_command(&mdb, cmd, &HashMap::new()).unwrap_err();
    assert!(matches!(e, TcError::Unsupported(_)), "{:?}", e);
    let mut opts = EncodeOptions::default();
    opts.parameter_values.insert(mdb.search_parameter("/P3/p_cnt").unwrap(), Value::Uint64(0x42));
    assert_eq!(vec![0x42], encode_command_with(&mdb, cmd, &HashMap::new(), &opts).unwrap().binary);

    // maximum size
    let opts = EncodeOptions { max_size: 2, ..Default::default() };
    let cmd = mdb.search_command("/P3/CMD_LOCATION").unwrap();
    let e = encode_command_with(&mdb, cmd, &args(&[("a", Value::Uint64(9))]), &opts).unwrap_err();
    assert!(matches!(e, TcError::TooLarge(_)), "{:?}", e);
}

#[test]
fn argument_list() {
    let mdb = load("test-xtce-files/phase3.xml");
    let cmd = mdb.search_command("/P3/CMD_STRUCT").unwrap();
    let r: Vec<(String, Option<String>)> = all_arguments(&mdb, cmd)
        .into_iter()
        .map(|(a, v)| (mdb.name2str(a.name()).to_owned(), v.map(|x| x.to_owned())))
        .collect();
    assert_eq!(("apid".to_owned(), Some("200".to_owned())), r[0]);
    assert_eq!(("flag".to_owned(), None), r[1]);
    assert_eq!(("type_id".to_owned(), Some("2".to_owned())), r[2]);
    assert_eq!(10, r.len());
}

#[test]
fn ref_xtce_commands() {
    let mdb = load("test-xtce-files/ref-xtce.xml");
    let name = |n: &str| mdb.name_db_ref().get(n).unwrap();
    // GPS and UNIX times encoded as seconds
    assert_eq!(
        "000000010000000a",
        hex(
            &mdb,
            "/RefXtce/command1",
            &[("t1", s("1980-01-06T00:00:01Z")), ("t2", Value::Timestamp(xtce_rs::time::from_unix_millis(10_000)))]
        )
    );
    // aggregate argument with the m2 member limited to 100
    let aggr = |m2: f64| {
        Value::Aggregate(Box::new(AggregateValue(HashMap::from([
            (name("m1"), Value::Uint64(1)),
            (name("m2"), Value::Double(m2)),
        ]))))
    };
    assert_eq!("00000001400c000000000000", hex(&mdb, "/RefXtce/command2", &[("arg1", aggr(3.5))]));
    assert!(encode(&mdb, "/RefXtce/command2", &[("arg1", aggr(101.0))]).is_err());
    // the aggregate initial value is composed from the member initial values (7, 3.14);
    // the float type has the default size of 32 bits so the value is a single precision float
    assert_eq!(
        format!("00000007{:016x}", ("3.14".parse::<f32>().unwrap() as f64).to_bits()),
        hex(&mdb, "/RefXtce/command4", &[])
    );
    // binary with the Yamcs LeadingSizeBinaryEncoder(16)
    assert_eq!("0003010203", hex(&mdb, "/RefXtce/command3", &[("arg1", s("010203"))]));
}
