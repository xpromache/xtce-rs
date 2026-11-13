use std::{path::Path, sync::OnceLock};

use xtce_rs::{mdb::MissionDatabase, parser};

/// the database used by the TM and TC targets, loaded once
pub fn mdb() -> &'static MissionDatabase {
    static MDB: OnceLock<MissionDatabase> = OnceLock::new();
    MDB.get_or_init(|| {
        let file = std::env::var("XTCE_FUZZ_FILE").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../test-xtce-files/BogusSAT-2.xml").to_owned()
        });
        parser::parse_files(&[Path::new(&file)]).expect("cannot load the XTCE file")
    })
}
