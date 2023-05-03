#[macro_use]
extern crate enum_map;
pub mod bitbuffer;
pub mod pvlist;
pub mod value;

pub mod mdb;
pub mod parser;
pub mod proc;



#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mdb::MissionDatabase, proc::containers::process};
    use std::path::Path;

    #[test]
    fn test_bogus2() {
    
    }
}
