//! Details of the TM processing used by the packet viewer: positions of the values, matched containers,
//! applied alarm ranges, processing without algorithms.
use std::path::Path;

use xtce_rs::{
    mdb::{MissionDatabase, NamedItem},
    parser,
    proc::{ProcessOptions, TmProcessor, TmResult},
    time::INVALID_INSTANT,
};

fn load(f: &str) -> MissionDatabase {
    parser::parse_files(&[Path::new(f)]).unwrap()
}

fn process(mdb: &MissionDatabase, root: &str, packet: &[u8], opts: &ProcessOptions) -> TmResult {
    let root = mdb.search_container(root).unwrap();
    let mut tmp = TmProcessor::new(mdb).unwrap();
    tmp.process_with(packet, root, INVALID_INSTANT, INVALID_INSTANT, opts).unwrap()
}

fn details() -> ProcessOptions {
    ProcessOptions { details: true, ..Default::default() }
}

/// (name, bit offset, bit size, container name, container start) for each value
fn positions(mdb: &MissionDatabase, r: &TmResult) -> Vec<(String, u32, u32, String, u32)> {
    r.params
        .iter()
        .zip(&r.positions)
        .map(|(pv, p)| {
            let p = p.unwrap();
            (
                mdb.name2str(mdb.get_parameter(pv.pidx).name()).to_owned(),
                p.bit_offset,
                p.bit_size,
                mdb.name2str(mdb.get_container(p.container).name()).to_owned(),
                p.container_start,
            )
        })
        .collect()
}

fn container_names(mdb: &MissionDatabase, r: &TmResult) -> Vec<(String, u32)> {
    r.containers
        .iter()
        .map(|c| (mdb.name2str(mdb.get_container(c.container).name()).to_owned(), c.start))
        .collect()
}

fn t(name: &str, off: u32, size: u32, c: &str, start: u32) -> (String, u32, u32, String, u32) {
    (name.to_owned(), off, size, c.to_owned(), start)
}

#[test]
fn simulator_dhs() {
    let mdb = load("test-xtce-files/simulator.xml");
    let packet = hex::decode("0801fff50015517e58c1b065000000020401050105010402000074b6").unwrap();
    let r = process(&mdb, "/YSS/ccsds-default", &packet, &details());

    assert_eq!(vec![("ccsds-default".to_owned(), 0), ("DHS".to_owned(), 0)], container_names(&mdb, &r));
    let dhs = mdb.search_container("/YSS/SIMULATOR/DHS").unwrap();
    assert_eq!(Some(dhs), r.packet_container());

    let pos = positions(&mdb, &r);
    assert_eq!(15, pos.len());
    assert_eq!(t("ccsds-apid", 5, 11, "ccsds-default", 0), pos[0]);
    assert_eq!(t("ccsds-seqcount", 18, 14, "ccsds-default", 0), pos[1]);
    assert_eq!(t("coarse-time", 48, 32, "ccsds-default", 0), pos[2]);
    assert_eq!(t("packet-id", 96, 32, "ccsds-default", 0), pos[5]);
    assert_eq!(t("PrimBusVoltage1", 128, 8, "DHS", 0), pos[6]);
    assert_eq!(t("MemoryRate", 192, 8, "DHS", 0), pos[14]);
    assert_eq!(r.params.len(), r.alarm_ranges.len());
}

#[test]
fn included_container() {
    let mdb = load("test-xtce-files/phase1.xml");
    let packet = [
        0, 3, 0x0A, 0xFF, 0x0B, 0xFF, 0x0C, 0xFF, 0x01, 0x02, 0x11, 0x12, 0x13, 0x14, 7, 0x01, 0x02, 0x21, 0x22,
        0x00, 0x33, 2, 0xEE, 0xEE, 0x44,
    ];
    let r = process(&mdb, "/P1/root", &packet, &details());
    let pos = positions(&mdb, &r);
    let get = |n: &str| pos.iter().filter(|p| p.0 == n).cloned().collect::<Vec<_>>();

    assert_eq!(vec![t("mode", 0, 8, "root", 0)], get("mode"));
    // repeated entry: one position per value
    assert_eq!(
        vec![t("rep", 16, 8, "struct", 0), t("rep", 32, 8, "struct", 0), t("rep", 48, 8, "struct", 0)],
        get("rep")
    );
    assert_eq!(vec![t("arr", 64, 16, "struct", 0)], get("arr"));
    assert_eq!(vec![t("target_b", 120, 16, "struct", 0)], get("target_b"));
    // the included container starts at byte 17
    assert_eq!(vec![t("inc1", 136, 8, "inc", 17)], get("inc1"));
    assert_eq!(vec![t("inc2", 144, 8, "inc", 17)], get("inc2"));
    assert_eq!(vec![t("after_inc", 160, 8, "struct", 0)], get("after_inc"));
    assert_eq!(vec![t("dyn_loc", 192, 8, "struct", 0)], get("dyn_loc"));

    let cnames = container_names(&mdb, &r);
    assert!(cnames.contains(&("inc".to_owned(), 17)), "{:?}", cnames);
    // the packet container is not the included one
    assert_eq!("struct", mdb.name2str(mdb.get_container(r.packet_container().unwrap()).name()));
}

#[test]
fn no_details_by_default() {
    let mdb = load("test-xtce-files/simulator.xml");
    let packet = hex::decode("0801fff50015517e58c1b065000000020401050105010402000074b6").unwrap();
    let r = process(&mdb, "/YSS/ccsds-default", &packet, &ProcessOptions::default());
    assert_eq!(15, r.params.len());
    assert!(r.positions.is_empty() && r.containers.is_empty() && r.alarm_ranges.is_empty());
}

#[test]
fn alarm_ranges() {
    let mdb = load("test-xtce-files/phase2.xml");
    let ranges = |state: u8| {
        let r = process(&mdb, "/P2/root", &[3, state, 50, 0], &details());
        let get = |n: &str| {
            let i = r.params.iter().position(|pv| mdb.name2str(mdb.get_parameter(pv.pidx).name()) == n).unwrap();
            r.alarm_ranges[i].clone()
        };
        // enumerated and parameters without alarm have no numeric ranges
        assert!(get("state").is_none());
        assert!(get("counter").is_none());
        get("volt").unwrap()
    };

    // default alarm
    let ar = ranges(0);
    assert_eq!(Some(2.0), ar.watch.as_ref().unwrap().min_inclusive);
    assert_eq!(Some(9.0), ar.warning.as_ref().unwrap().max_inclusive);
    // state SAFE: the context alarm replaces the default one
    let ar = ranges(1);
    assert!(ar.watch.is_none());
    assert_eq!(Some(3.0), ar.warning.as_ref().unwrap().max_inclusive);
}

#[test]
fn without_algorithms() {
    let mdb = load("test-xtce-files/phase2.xml");
    let name = |r: &TmResult| {
        r.params.iter().map(|pv| mdb.name2str(mdb.get_parameter(pv.pidx).name()).to_owned()).collect::<Vec<_>>()
    };
    let r = process(&mdb, "/P2/root", &[3, 0, 50, 7], &details());
    assert!(name(&r).contains(&"sum".to_owned()));
    // the algorithm outputs have no position
    let i = name(&r).iter().position(|n| n == "sum").unwrap();
    assert!(r.positions[i].is_none());

    let opts = ProcessOptions { details: true, algorithms: false, ..Default::default() };
    let r = process(&mdb, "/P2/root", &[3, 0, 50, 7], &opts);
    assert!(!name(&r).contains(&"sum".to_owned()));
    assert!(r.positions.iter().all(|p| p.is_some()));
}

#[test]
fn without_cache_update() {
    let mdb = load("test-xtce-files/phase2.xml");
    let root = mdb.search_container("/P2/root").unwrap();
    let pidx = mdb.search_parameter("/P2/counter").unwrap();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let opts = ProcessOptions { update_cache: false, ..Default::default() };
    tmp.process_with(&[3, 0, 50, 7], root, INVALID_INSTANT, INVALID_INSTANT, &opts).unwrap();
    assert!(tmp.cache().last_value(pidx).is_none());
    tmp.process(&[3, 0, 50, 7], root, INVALID_INSTANT, INVALID_INSTANT).unwrap();
    assert!(tmp.cache().last_value(pidx).is_some());
}

#[test]
fn partial_result_on_error() {
    let mdb = load("test-xtce-files/simulator.xml");
    // DHS packet truncated in the middle of the user data
    let packet = hex::decode("0801fff50015517e58c1b065000000020401050105").unwrap();
    let root = mdb.search_container("/YSS/ccsds-default").unwrap();
    let mut tmp = TmProcessor::new(&mdb).unwrap();
    let r = tmp.process_with(&packet, root, INVALID_INSTANT, INVALID_INSTANT, &details()).unwrap();
    assert!(r.error.is_some());
    assert_eq!("DHS", mdb.name2str(mdb.get_container(r.packet_container().unwrap()).name()));
    assert!(r.params.len() > 6 && r.params.len() < 15, "{}", r.params.len());
    assert_eq!(r.params.len(), r.positions.len());
    assert!(tmp.process(&packet, root, INVALID_INSTANT, INVALID_INSTANT).is_err());
}
