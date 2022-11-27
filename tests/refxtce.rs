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
    let path = Path::new("test-xtce-files/ref-xtce.xml");
    parser::parse(&mut mdb, path).unwrap();
    mdb
}

#[test]
fn binary_leading_size() {
    let mdb = init_mdb();
    
    let packet: Vec<u8> = vec![0x03, 0x01, 0x02, 0x03 ];

    let root_container = mdb.search_container("/RefXtce/packet1").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!(1, r.len());
    assert_eq!("010203", r[0].eng_value.to_string());
}


#[test]
fn fixed_sized_array() {
    let mdb = init_mdb();
    // null terminated string in fixed size buffer
    let packet: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08 ];

    let root_container = mdb.search_container("/RefXtce/packet3").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();
}

#[test]
fn numeric_string_encoding() {
    let mdb = init_mdb();
    // null terminated string in fixed size buffer
    let packet: Vec<u8> = vec![b'1', b'0', b'0', 0, 0, 0,
    b'-', b'3', b'.', b'1', b'4', 0 ];

    let root_container = mdb.search_container("/RefXtce/packet4").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!(2, r.len());
    assert_eq!("100", r.raw(0).to_string());
    assert_eq!(100u64, r.eng(1).try_into().unwrap());
    
    assert_eq!("-3.14", r.raw(1).to_string());
    assert_eq!(-3.14, r.eng(1).try_into().unwrap());
}

