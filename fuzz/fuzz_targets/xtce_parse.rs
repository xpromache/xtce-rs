//! XTCE parsing. Use the files from test-xtce-files as seed corpus:
//!   mkdir -p corpus/xtce_parse && cp ../test-xtce-files/*.xml corpus/xtce_parse/
#![no_main]

use libfuzzer_sys::fuzz_target;
use xtce_rs::{mdb::MissionDatabase, parser, proc::TmProcessor};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let mut mdb = MissionDatabase::new();
    if parser::parse_str(&mut mdb, text).is_ok() {
        let _ = TmProcessor::new(&mdb);
    }
});
