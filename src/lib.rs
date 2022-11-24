#[macro_use]
extern crate enum_map;
pub mod value;
pub mod pvlist;
pub mod bitbuffer;

pub mod mdb;
pub mod parser;
pub mod proc;
pub mod error;



#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mdb::{MissionDatabase}, proc::containers::process};
    use std::path::Path;
    
    #[test]
    fn test_bogus2() {
        env_logger::init();
        
        let mut mdb = MissionDatabase::new();
        let path = Path::new("/home/nm/git/yamcs/yamcs-core/src/test/resources/xtce/BogusSAT-2.xml");
        parser::parse(&mut mdb, path).unwrap();
       
       // println!("mdb: {:?}", mdb);

        let packet: Vec<u8> = vec![0x08, 0x23, // CCSDS_Packet_ID {version=0, type = 0, SecHdrFlag = 1, apid=0x23
             0xC0, 0x56, // CCSDS_Packet_Sequence {GroupFlags=3, count = 0x56}
             0, 5, // length 5
            0x35, 0x10, 0x20, 0x03, 0x05, // PUS_Data_Field_Header {Spare1 = 0, Version=3, Spare4=5, Service = 0x10,
                                          // Subservice=0x20, SeqCount = 3, Destination=5}
            0, 0];

        let root_container = mdb.search_container("/BogusSAT/CCSDSPacket").unwrap();
        let r = process(&mdb, &packet, root_container).unwrap();

        for pv in &r {
            println!("{:?}", pv.dbg(&mdb));
        }

        assert_eq!(4, r.len());

        //println!("container result: {:?}", r);
        
    }


}