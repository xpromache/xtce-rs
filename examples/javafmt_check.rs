//! Compares format::java_double/java_float with a file of reference outputs produced by Java:
//! lines "d <hex bits> <Double.toString>" or "f <hex bits> <Float.toString>".
fn main() {
    let path = std::env::args().nth(1).expect("usage: javafmt_check cases.txt");
    let text = std::fs::read_to_string(path).unwrap();
    let (mut n, mut bad) = (0, 0);
    for line in text.lines() {
        let mut it = line.split(' ');
        let (kind, bits, expected) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let got = if kind == "d" {
            xtce_rs::format::java_double(f64::from_bits(u64::from_str_radix(bits, 16).unwrap()))
        } else {
            xtce_rs::format::java_float(f32::from_bits(u32::from_str_radix(bits, 16).unwrap()))
        };
        n += 1;
        if got != expected {
            bad += 1;
            if bad <= 20 {
                println!("{} {}: java {} rust {}", kind, bits, expected, got);
            }
        }
    }
    println!("{} cases, {} differences", n, bad);
}
