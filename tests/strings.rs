use std::{path::Path};

use xtce_rs::{mdb::MissionDatabase, parser, proc::containers::process};
static INIT: std::sync::Once = std::sync::Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

#[test]
fn test_bogus2() {
    init_logging();

    let mut mdb = MissionDatabase::new();
    let path = Path::new("test-xtce-files/BogusSAT-2.xml");
    parser::parse(&mut mdb, path).unwrap();

    // println!("mdb: {:?}", mdb);

    let packet: Vec<u8> = vec![
        0x08, 0x23, // CCSDS_Packet_ID {version=0, type = 0, SecHdrFlag = 1, apid=0x23
        0xC0, 0x56, // CCSDS_Packet_Sequence {GroupFlags=3, count = 0x56}
        0, 5, // length 5
        0x35, 0x10, 0x20, 0x03,
        0x05, // PUS_Data_Field_Header {Spare1 = 0, Version=3, Spare4=5, Service = 0x10,
        // Subservice=0x20, SeqCount = 3, Destination=5}
        0, 0,
    ];

    let root_container = mdb.search_container("/BogusSAT/CCSDSPacket").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    for pv in &r {
        println!("{:?}", pv.dbg(&mdb));
    }

    assert_eq!(4, r.len());

    //println!("container result: {:?}", r);
}

fn str_mdb() -> MissionDatabase {
    init_logging();

    let mut mdb = MissionDatabase::new();
    let path = Path::new("test-xtce-files/strings-tm.xml");
    parser::parse(&mut mdb, path).unwrap();
    mdb
}

#[test]
fn fixed_size_buf() {
    let mdb = str_mdb();

    // null terminated string in fixed size buffer
    let packet: Vec<u8> = vec![b'a', b'b', 0, 0, 0, 0, 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet1").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!("ab", r[0].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(1).try_into().unwrap());
}

#[test]
fn fixed_size_noterminator() {
    let mdb = str_mdb();

    // null terminated string in fixed size buffer but the string is as long as the buffer so there is no terminator
    let packet: Vec<u8> = vec![b'a', b'b', b'c', b'd', b'e', b'f', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet1").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!("abcdef", r[0].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(1).try_into().unwrap());
}

#[test]
fn fixed_size2() {
    let mdb = str_mdb();

    // fixed size string in fixed size buffer
    let packet: Vec<u8> = vec![b'a', b'b', b'c', b'd', b'e', b'f', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet2").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!("abcdef", r[0].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(1).try_into().unwrap());
}

#[test]
fn fixed_size3() {
    let mdb = str_mdb();

    // null terminated string in undefined buffer
    let packet: Vec<u8> = vec![b'a', b'b', 0, 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet3").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!("ab", r[0].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(1).try_into().unwrap());
}

#[test]
fn fixed_size3_no_terminator() {
    let mdb = str_mdb();

    // null terminated string in undefined buffer
    let packet: Vec<u8> = vec![b'a', b'b', b'c', b'd', b'e', b'f', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet3").unwrap();
    let r = process(&mdb, &packet, root_container);
    assert!(r.is_err());
}

#[test]
fn fixed_size4() {
    let mdb = str_mdb();

    // null terminated string in undefined buffer
    let packet: Vec<u8> = vec![0, 6, 3, b'a', b'b', b'c', b'x', b'x', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet4").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!(6u64, r.eng(0).try_into().unwrap());
    assert_eq!("abc", r[1].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(2).try_into().unwrap());
}

#[test]
fn fixed_size5() {
    let mdb = str_mdb();

    // null terminated string in undefined buffer
    let packet: Vec<u8> = vec![0, 2, b'a', b'b', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet5").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!("ab", r[0].eng_value.to_string());
    assert_eq!(0x0102u64, r.eng(1).try_into().unwrap());
}

#[test]
fn fixed_size5_too_long() {
    let mdb = str_mdb();

    // null terminated string in undefined buffer
    let packet: Vec<u8> = vec![0, 5, b'a', b'b', b'c', b'd', b'e', 0x01, 0x02];

    let root_container = mdb.search_container("/StringsTm/packet5").unwrap();
    let r = process(&mdb, &packet, root_container);
    assert!(r.is_err());
}
