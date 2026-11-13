//! TC encoding: the first byte selects the command, the rest is split by ';' into the argument values
//! (given as strings, in the order of the arguments).
#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use xtce_rs::{
    mdb::{MetaCommandIdx, NamedItem},
    tc::{all_arguments, encode_command},
    value::Value,
};

mod common;

fuzz_target!(|data: &[u8]| {
    let mdb = common::mdb();
    if data.is_empty() || mdb.commands.is_empty() {
        return;
    }
    let cmd = MetaCommandIdx::new(data[0] as usize % mdb.commands.len());
    let text = String::from_utf8_lossy(&data[1..]);
    let mut values = text.split(';');
    let mut args = HashMap::new();
    for (arg, _) in all_arguments(mdb, cmd) {
        if let Some(v) = values.next() {
            args.insert(mdb.name2str(arg.name()).to_owned(), Value::StringValue(Box::new(v.to_owned())));
        }
    }
    let _ = encode_command(mdb, cmd, &args);
});
