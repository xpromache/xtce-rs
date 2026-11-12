//! Regression tests for bugs fixed during the Phase 0 hardening.

use std::path::Path;

use xtce_rs::{
    mdb::{types::TypeData, MissionDatabase, NamedItem},
    parser,
    proc::containers::process,
};

fn load(file: &str) -> MissionDatabase {
    let mut mdb = MissionDatabase::new();
    parser::parse(&mut mdb, Path::new(file)).unwrap();
    mdb
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}

/// Decoding a truncated packet has to return an error (or a partial result), never panic
#[test]
fn truncated_packets_do_not_panic() {
    let mdb = load("test-xtce-files/simulator.xml");
    let cases = [
        ("/YSS/SIMULATOR/DHS", "0801fff50015517e58c1b065000000020401050105010402000074b6"),
        ("/YSS/SIMULATOR/FlightData", "0801fb7e0047517e74b4b36500000021435dc000c27265604254e148458ccd9a41ddb43940314c983e00c49c42ec8a3d42ec8a3d3ebbbecb3f7ec02f4238333340af2a30c1ad70a441ddb4390520"),
    ];

    for (cname, hex) in cases {
        let packet = hex_to_bytes(hex);
        let root = mdb.search_container(cname).unwrap();
        assert!(process(&mdb, &packet, root).is_ok());
        for len in 0..packet.len() {
            // the result does not matter, it just should not panic
            let _ = process(&mdb, &packet[..len], root);
        }
    }

    // a packet ending in the middle of the first parameter has to fail
    let root = mdb.search_container("/YSS/SIMULATOR/DHS").unwrap();
    assert!(process(&mdb, &[0x08], root).is_err());
}

#[test]
fn truncated_binary_leading_size() {
    let mdb = load("test-xtce-files/ref-xtce.xml");
    let root = mdb.search_container("/RefXtce/packet1").unwrap();
    // the size tag says 3 bytes but only 1 is available
    assert!(process(&mdb, &[0x03, 0x01], root).is_err());
}

/// Name lookup used to panic after 10000 calls
#[test]
fn many_name_lookups() {
    let mdb = load("test-xtce-files/simulator.xml");
    let p = &mdb.parameters[0];
    for _ in 0..20_000 {
        mdb.name2str(p.name());
    }
}

#[test]
fn enumeration_max_value() {
    let mdb = load("test-xtce-files/BogusSAT-2.xml");
    let found = mdb.parameter_types.iter().any(|dt| match &dt.type_data {
        TypeData::Enumerated(edt) => {
            edt.enumeration.iter().any(|e| e.label == "FAILED" && e.value == 6 && e.max_value == 16)
        }
        _ => false,
    });
    assert!(found, "enumeration range with maxValue=16 not found");
}

#[test]
fn container_abstract_default() {
    let mdb = load("test-xtce-files/BogusSAT-2.xml");
    let c = mdb.get_container(mdb.search_container("/BogusSAT/CCSDSPacket").unwrap());
    assert!(c.abstract_);

    let mdb = load("test-xtce-files/simulator.xml");
    let c = mdb.get_container(mdb.search_container("/YSS/SIMULATOR/FlightData").unwrap());
    assert!(!c.abstract_, "containers are not abstract unless specified");
}

#[test]
fn units() {
    let mdb = load("test-xtce-files/simulator.xml");
    let dt = mdb
        .parameter_types
        .iter()
        .find(|dt| mdb.name2str(dt.name()) == "uint8A")
        .unwrap();
    assert_eq!(1, dt.units.len());
    assert_eq!("A", dt.units[0].unit);
}
