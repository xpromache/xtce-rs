#[macro_use]
extern crate enum_map;
pub mod mdb;
pub mod mdb_debug;
pub mod value;


pub mod parser;


#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdb::{MissionDatabase};
    use std::path::Path;
    // use lasso::{Rodeo, Spur};





    #[test]
    fn test_api() {
        let mut mdb = MissionDatabase::new();
        let path = Path::new("/home/nm/git/yamcs/yamcs-core/src/test/resources/xtce/BogusSAT-2.xml");
        if let Err(e) = parser::parse(&mut mdb, path) {
            eprintln!("Error: {:?}", e);
        }


        println!("mdb: {:?}", mdb);
    }
}