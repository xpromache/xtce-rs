//! Loads an XTCE file and prints a summary; useful to check what is not supported.
//! Usage: RUST_LOG=warn cargo run --example load -- file.xml
use std::path::Path;

fn main() {
    env_logger::init();
    for f in std::env::args().skip(1) {
        match xtce_rs::parser::parse_files(&[Path::new(&f)]) {
            Ok(mdb) => println!(
                "{}: {} data types, {} parameters, {} containers, {} commands, {} command containers",
                f,
                mdb.parameter_types.len(),
                mdb.parameters.len(),
                mdb.containers.len(),
                mdb.commands.len(),
                mdb.command_containers.len()
            ),
            Err(e) => println!("{}: ERROR {:?}", f, e),
        }
    }
}
