use std::path::Path;

use xtce_rs::{mdb::MissionDatabase, parser, proc::containers::process};

static INIT: std::sync::Once = std::sync::Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        env_logger::init();
    });
}


fn init_multi_mdb() -> MissionDatabase {
    init_logging();

    let paths = [
        "test-xtce-files/multi-dt.xml",
        "test-xtce-files/multi-pkt.xml"
    ].map(Path::new);
    
    parser::parse_files(&paths)
        .expect("multixtce files should be valid")
}

#[test]
fn type_defined_in_different_file() {
    let mdb = init_multi_mdb();

    let packet: Vec<u8> = vec![0xff, 0xef];

    let root_container = mdb.search_container("/multi-pkt/packet-signedint").unwrap();
    let r = process(&mdb, &packet, root_container).unwrap();

    assert_eq!(1, r.len());
    assert_eq!("-17", r[0].eng_value.to_string());
}
