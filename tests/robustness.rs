//! Randomized robustness tests: the TM decoding, the TC encoding and the XTCE parser must never panic,
//! whatever the input. They return an error for the invalid inputs.
//!
//! The tests are deterministic (fixed seeds). The number of iterations can be increased with the environment
//! variable XTCE_FUZZ_ITERATIONS (e.g. `XTCE_FUZZ_ITERATIONS=100000 cargo test --release --test robustness`).
//! For coverage guided fuzzing see the cargo-fuzz targets in the fuzz directory.

use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
};

use rand::{rngs::SmallRng, Rng, SeedableRng};
use xtce_rs::{
    mdb::{MetaCommandIdx, MissionDatabase},
    parser,
    proc::{containers::process, TmProcessor},
    tc::{all_arguments, encode_command},
    value::Value,
    mdb::{ContainerIdx, NamedItem},
};

const FILES: [&str; 8] = [
    "BogusSAT-2.xml",
    "ref-xtce.xml",
    "simulator.xml",
    "strings-tm.xml",
    "phase1.xml",
    "phase2.xml",
    "phase3.xml",
    "tc-extensions.xml",
];

fn iterations(default: usize) -> usize {
    std::env::var("XTCE_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn load(f: &str) -> MissionDatabase {
    parser::parse_files(&[Path::new(&format!("test-xtce-files/{}", f))]).unwrap()
}

/// runs f and reports the input if it panics
fn no_panic<R>(what: &str, input: impl FnOnce() -> String, f: impl FnOnce() -> R) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        panic!("{} panicked for input {}", what, input());
    }
}

/// the packets from the golden cases, used as seeds for the mutations
fn seed_packets() -> HashMap<String, Vec<(String, Vec<u8>)>> {
    let mut r: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();
    let text = std::fs::read_to_string("tests/golden/expected.txt").unwrap();
    for line in text.lines().filter(|l| l.starts_with("tm\t")) {
        let f: Vec<&str> = line.split('\t').collect();
        r.entry(f[1].to_owned()).or_default().push((f[2].to_owned(), hex::decode(f[3]).unwrap()));
    }
    r
}

fn mutate(rng: &mut SmallRng, p: &[u8]) -> Vec<u8> {
    let mut p = p.to_vec();
    for _ in 0..rng.gen_range(1..4) {
        match rng.gen_range(0..5) {
            // flip a bit
            0 if !p.is_empty() => {
                let i = rng.gen_range(0..p.len());
                p[i] ^= 1 << rng.gen_range(0..8);
            }
            // random byte
            1 if !p.is_empty() => {
                let i = rng.gen_range(0..p.len());
                p[i] = rng.gen();
            }
            // truncate
            2 => p.truncate(rng.gen_range(0..=p.len())),
            // extend
            3 => p.extend((0..rng.gen_range(1..16)).map(|_| rng.gen::<u8>())),
            // extreme values
            _ if !p.is_empty() => {
                let i = rng.gen_range(0..p.len());
                p[i] = if rng.gen() { 0xFF } else { 0 };
            }
            _ => {}
        }
    }
    p
}

#[test]
fn tm_random_packets() {
    let n = iterations(300);
    let seeds = seed_packets();
    let mut rng = SmallRng::seed_from_u64(1);
    for f in FILES {
        let mdb = load(f);
        let containers: Vec<ContainerIdx> = (0..mdb.containers.len()).map(ContainerIdx::new).collect();
        if containers.is_empty() {
            continue;
        }
        let empty = Vec::new();
        let seeds = seeds.get(f).unwrap_or(&empty);
        // the stateful processor (with algorithms, alarms and history) is reused for all the packets
        let mut tmp = TmProcessor::new(&mdb).unwrap();
        for i in 0..n {
            let (cidx, packet) = if !seeds.is_empty() && i % 2 == 0 {
                let (cname, p) = &seeds[rng.gen_range(0..seeds.len())];
                (mdb.search_container(cname).unwrap(), mutate(&mut rng, p))
            } else {
                let len = rng.gen_range(0..80);
                (containers[rng.gen_range(0..containers.len())], (0..len).map(|_| rng.gen()).collect())
            };
            let input = || {
                format!("{} container {} packet {}", f, mdb.name2str(mdb.get_container(cidx).name()), hex::encode(&packet))
            };
            no_panic("TM processing", input, || {
                let _ = process(&mdb, &packet, cidx);
                let _ = tmp.process(&packet, cidx, 0, 0);
            });
        }
    }
}

/// argument values likely to find problems
fn random_arg_value(rng: &mut SmallRng) -> Value {
    const STRINGS: &[&str] = &[
        "", "0", "1", "-1", "255", "256", "65535", "-32768", "2147483648", "-2147483649", "4294967296",
        "9223372036854775807", "-9223372036854775808", "18446744073709551615", "18446744073709551616", "1e300",
        "-1e300", "NaN", "Infinity", "-0.0", "0.5", "3.14159", "0x10", "true", "false", "True", "ON", "OFF",
        "BACKUP", "STANDBY", "abc", "a.txt", "0102", "zz", "{}", "[]", "[1,2,3]", "[[1],[2]]", r#"{"x": 1}"#,
        r#"{"x": 1, "y": 2}"#, r#"{"m1": 1, "m2": 2.5}"#, r#"{"x": {"y": 1}}"#, "{", "[1,", "2020-01-01T00:00:00Z",
        "1970-01-01T00:00:00Z", "1900-01-01T00:00:00Z", "9999-12-31T23:59:59.999Z", "2016-12-31T23:59:60Z",
        "not a time", "\u{0}", "ünïcödé",
    ];
    match rng.gen_range(0..6) {
        0 => Value::Int64(rng.gen()),
        1 => Value::Uint64(rng.gen()),
        2 => Value::Double(f64::from_bits(rng.gen())),
        3 => Value::Boolean(rng.gen()),
        4 => Value::Binary(Box::new((0..rng.gen_range(0..8)).map(|_| rng.gen()).collect())),
        _ => Value::StringValue(Box::new(STRINGS[rng.gen_range(0..STRINGS.len())].to_owned())),
    }
}

#[test]
fn tc_random_arguments() {
    let n = iterations(300);
    let mut rng = SmallRng::seed_from_u64(2);
    for f in FILES {
        let mdb = load(f);
        let commands: Vec<MetaCommandIdx> = (0..mdb.commands.len()).map(MetaCommandIdx::new).collect();
        if commands.is_empty() {
            continue;
        }
        for _ in 0..n {
            let cmd = commands[rng.gen_range(0..commands.len())];
            let mut args = HashMap::new();
            for (arg, _) in all_arguments(&mdb, cmd) {
                // most of the time all the arguments are given
                if rng.gen_range(0..10) > 0 {
                    args.insert(mdb.name2str(arg.name()).to_owned(), random_arg_value(&mut rng));
                }
            }
            if rng.gen_range(0..20) == 0 {
                args.insert("unknown".to_owned(), Value::Int64(1));
            }
            let input = || format!("{} command {} args {:?}", f, mdb.name2str(mdb.get_command(cmd).name()), args);
            no_panic("TC encoding", input, || {
                let _ = encode_command(&mdb, cmd, &args);
            });
        }
    }
}

/// mutates an XML document: changes attribute values, removes elements or characters
fn mutate_xml(rng: &mut SmallRng, xml: &str) -> String {
    const VALUES: &[&str] = &[
        "0", "-1", "1", "64", "65", "128", "-129", "4294967296", "99999999999999999999", "1e309", "NaN", "",
        "abc", "true", "/", "..", "../..", "_yamcs_ignore", "a/b/c", "x[0]", "twosComplement", "signMagnitude",
        "IEEE754_1985", "MILSTD_1750A", "UTF-16", "containerStart", "previousEntry", "containerEnd",
    ];
    let mut s = xml.to_owned();
    for _ in 0..rng.gen_range(1..4) {
        match rng.gen_range(0..3) {
            // replace an attribute value
            0 => {
                let quotes: Vec<usize> = s.match_indices("=\"").map(|(i, _)| i + 2).collect();
                if quotes.is_empty() {
                    continue;
                }
                let start = quotes[rng.gen_range(0..quotes.len())];
                if let Some(len) = s[start..].find('"') {
                    let v = VALUES[rng.gen_range(0..VALUES.len())];
                    s.replace_range(start..start + len, v);
                }
            }
            // remove an element (from a start tag to the next end tag)
            1 => {
                let tags: Vec<usize> = s.match_indices('<').map(|(i, _)| i).collect();
                let start = tags[rng.gen_range(0..tags.len())];
                if let Some(end) = s[start..].find("/>").map(|e| start + e + 2) {
                    s.replace_range(start..end, "");
                }
            }
            // change the text of an element
            _ => {
                let texts: Vec<usize> = s.match_indices('>').map(|(i, _)| i + 1).collect();
                let start = texts[rng.gen_range(0..texts.len())];
                if let Some(len) = s[start..].find('<') {
                    if s[start..start + len].trim().is_empty() {
                        continue;
                    }
                    let v = VALUES[rng.gen_range(0..VALUES.len())];
                    s.replace_range(start..start + len, v);
                }
            }
        }
    }
    s
}

#[test]
fn parser_mutated_files() {
    let n = iterations(40);
    let mut rng = SmallRng::seed_from_u64(3);
    for f in FILES {
        let xml = std::fs::read_to_string(format!("test-xtce-files/{}", f)).unwrap();
        for _ in 0..n {
            let mxml = mutate_xml(&mut rng, &xml);
            let input = || {
                let path = std::env::temp_dir().join("xtce-rs-robustness-failure.xml");
                std::fs::write(&path, &mxml).unwrap();
                format!("mutated {} saved in {}", f, path.display())
            };
            no_panic("XTCE parsing", input, || {
                let mut mdb = MissionDatabase::new();
                if parser::parse_str(&mut mdb, &mxml).is_ok() {
                    // the mutated database is still usable
                    let _ = TmProcessor::new(&mdb);
                }
            });
        }
    }
}
