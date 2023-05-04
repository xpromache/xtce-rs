use std::path::Path;

use xtce_rs::{mdb::MissionDatabase, parser, proc::containers::process};

static INIT: std::sync::Once = std::sync::Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

fn init_mdb() -> MissionDatabase {
    init_logging();

    let mut mdb = MissionDatabase::new();
    let path = Path::new("test-xtce-files/simulator.xml");
    parser::parse(&mut mdb, path).unwrap();
    mdb
}

#[test]
fn dhs() {
    let mdb = init_mdb();

    let packet: Vec<u8> =
        hex_to_bytes("0801fff50015517e58c1b065000000020401050105010402000074b6").unwrap();

    let root_container = mdb.search_container("/YSS/SIMULATOR/DHS").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();
}

#[test]
fn flightdata() {
    let mdb = init_mdb();

    let packet: Vec<u8> =
        hex_to_bytes("0801fb7e0047517e74b4b36500000021435dc000c27265604254e148458ccd9a41ddb43940314c983e00c49c42ec8a3d42ec8a3d3ebbbecb3f7ec02f4238333340af2a30c1ad70a441ddb4390520").unwrap();

    let root_container = mdb.search_container("/YSS/SIMULATOR/FlightData").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();
}

fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 == 0 {
        (0..s.len())
            .step_by(2)
            .map(|i| s.get(i..i + 2).and_then(|sub| u8::from_str_radix(sub, 16).ok()))
            .collect()
    } else {
        None
    }
}
