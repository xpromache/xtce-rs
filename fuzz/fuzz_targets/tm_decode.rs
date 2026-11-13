//! TM decoding: the first byte selects the start container, the rest is the packet.
#![no_main]

use libfuzzer_sys::fuzz_target;
use xtce_rs::{
    mdb::ContainerIdx,
    proc::{containers::process, TmProcessor},
};

mod common;

fuzz_target!(|data: &[u8]| {
    let mdb = common::mdb();
    if data.is_empty() || mdb.containers.is_empty() {
        return;
    }
    let cidx = ContainerIdx::new(data[0] as usize % mdb.containers.len());
    let packet = &data[1..];
    let _ = process(mdb, packet, cidx);
    // the second packet uses the history of the first one
    let mut tmp = TmProcessor::new(mdb).unwrap();
    let _ = tmp.process(packet, cidx, 0, 0);
    let _ = tmp.process(packet, cidx, 0, 0);
});
