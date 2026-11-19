//! Text representations identical to the ones of Yamcs (Java): values (`Value.toString()`), calibrators.
//!
//! Tools comparing or displaying values like Yamcs (e.g. the packet viewer, whose display filter compares
//! the engineering values as strings) need the Java formatting of the numbers: `1.0`, `1.0E10`, `1.0E-5`.

use crate::{
    mdb::{
        types::{Calibrator, DataType, MathOpElement, TypeData},
        MissionDatabase, NamedItem,
    },
    value::{Value, ValueType},
};

/// Java `Double.toString(x)` (JDK 19+ algorithm): among the shortest decimals (at least 2 digits) that round
/// to x, the closest one, the even one in case of a tie; e.g. `4.9E-324` for the smallest double.
///
/// Decimal notation with at least one fractional digit for 10^-3 <= |x| < 10^7, otherwise
/// computerized scientific notation (`1.234E-5`, `1.0E7`).
pub fn java_double(x: f64) -> String {
    if let Some(s) = special(x.is_nan(), x.is_infinite(), x == 0.0, x.is_sign_negative()) {
        return s;
    }
    // the length of the shortest decimal that rounds to x (at least 2 digits); among the decimals of that
    // length, Java takes the closest to x, the even one in case of a tie (the formatting with a precision
    // rounds the exact binary value, ties to even)
    let shortest = format!("{:e}", x);
    let n = sig_digits(&shortest).max(2);
    let closest = format!("{:.*e}", n - 1, x);
    if closest.parse::<f64>() == Ok(x) {
        render(&closest)
    } else {
        render(&shortest)
    }
}

/// Java `Float.toString(x)`: like [java_double] for the float precision.
pub fn java_float(x: f32) -> String {
    if let Some(s) = special(x.is_nan(), x.is_infinite(), x == 0.0, x.is_sign_negative()) {
        return s;
    }
    // the length of the shortest decimal that rounds to x (at least 2 digits); among the decimals of that
    // length, Java takes the closest to x, the even one in case of a tie (the formatting with a precision
    // rounds the exact binary value, ties to even)
    let shortest = format!("{:e}", x);
    let n = sig_digits(&shortest).max(2);
    let closest = format!("{:.*e}", n - 1, x);
    if closest.parse::<f32>() == Ok(x) {
        render(&closest)
    } else {
        render(&shortest)
    }
}

fn special(nan: bool, inf: bool, zero: bool, negative: bool) -> Option<String> {
    let s = if nan {
        "NaN"
    } else if inf {
        if negative { "-Infinity" } else { "Infinity" }
    } else if zero {
        if negative { "-0.0" } else { "0.0" }
    } else {
        return None;
    };
    Some(s.to_owned())
}

/// number of significant digits of a Rust scientific representation ("-1.25e3" -> 3)
fn sig_digits(sci: &str) -> usize {
    let mantissa = sci.split('e').next().unwrap_or("");
    mantissa.chars().filter(|c| c.is_ascii_digit()).count()
}

/// renders a Rust scientific representation ("-1.25e3") like Java
fn render(sci: &str) -> String {
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let (negative, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => (true, m),
        None => (false, mantissa),
    };
    let mut digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let mut out = String::with_capacity(digits.len() + 8);
    if negative {
        out.push('-');
    }
    if (-3..7).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                out.push_str(&digits);
                out.extend(std::iter::repeat('0').take(int_len - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat('0').take((-exp - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        out.push_str(if digits.len() > 1 { &digits[1..] } else { "0" });
        out.push('E');
        out.push_str(&exp.to_string());
    }
    out
}

/// Java `StringConverter.arrayToHexString`: upper case hexadecimal
pub fn hex_upper(b: &[u8]) -> String {
    hex::encode_upper(b)
}

/// The engineering value as Yamcs prints it (`Value.toString()`), using the data type of the parameter for
/// the Yamcs value type (FLOAT vs DOUBLE) and the order of the aggregate members.
pub fn eng_value_to_string(mdb: &MissionDatabase, dtype: &DataType, v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, mdb, Some(dtype), v, false);
    s
}

/// The raw value as Yamcs prints it; the value types come from the data encodings.
pub fn raw_value_to_string(mdb: &MissionDatabase, dtype: &DataType, v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, mdb, Some(dtype), v, true);
    s
}

/// A value without type information: integers, booleans... as Yamcs; floating point values as DOUBLE,
/// aggregate members ordered by name.
pub fn value_to_string(mdb: &MissionDatabase, v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, mdb, None, v, false);
    s
}

fn write_value(out: &mut String, mdb: &MissionDatabase, dtype: Option<&DataType>, v: &Value, raw: bool) {
    match v {
        Value::Int64(x) => out.push_str(&x.to_string()),
        Value::Uint64(x) => out.push_str(&x.to_string()),
        Value::Double(x) => {
            let vtype = dtype.and_then(|dt| if raw { dt.encoding.raw_value_type() } else { Some(dt.eng_value_type()) });
            if vtype == Some(ValueType::Float) {
                out.push_str(&java_float(*x as f32))
            } else {
                out.push_str(&java_double(*x))
            }
        }
        Value::Boolean(x) => out.push_str(if *x { "true" } else { "false" }),
        Value::StringValue(x) => out.push_str(x),
        Value::Enumerated(x) => out.push_str(&x.value),
        Value::Binary(x) => out.push_str(&hex_upper(x)),
        Value::Timestamp(t) => out.push_str(&crate::time::to_string(*t)),
        Value::Aggregate(agg) => {
            let members = dtype.and_then(|dt| match &dt.type_data {
                TypeData::Aggregate(at) => Some(&at.members),
                _ => None,
            });
            let mut first = true;
            let mut write_member = |out: &mut String, name: &str, mdt: Option<&DataType>, mv: &Value| {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(name);
                out.push_str(" : ");
                write_value(out, mdb, mdt, mv, raw);
            };
            match members {
                Some(members) => {
                    for m in members {
                        if let Some(mv) = agg.0.get(&m.name()) {
                            write_member(out, mdb.name2str(m.name()), Some(mdb.get_data_type(m.dtype)), mv);
                        }
                    }
                }
                None => {
                    let mut entries: Vec<_> = agg.0.iter().map(|(k, v)| (mdb.name2str(*k), v)).collect();
                    entries.sort_by(|a, b| a.0.cmp(b.0));
                    for (name, mv) in entries {
                        write_member(out, name, None, mv);
                    }
                }
            }
        }
        Value::Array(arr) => {
            let edt = dtype.and_then(|dt| match &dt.type_data {
                TypeData::Array(at) => Some(mdb.get_data_type(at.dtype)),
                _ => None,
            });
            out.push('[');
            for (i, ev) in arr.values.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, mdb, edt, ev, raw);
            }
            out.push(']');
        }
    }
}

/// The calibrator as Yamcs prints it: `PolynomialCalibrator[1.0, 2.0]`, `SplineCalibrator[(0.0,1.0), (1.0,3.0)]`.
///
/// Yamcs has no text for the math operation calibrators (Java `Object.toString()`); here they are
/// printed as `MathOperationCalibrator[this 2.0 *]` (reverse polish notation).
pub fn calibrator_to_string(c: &Calibrator) -> String {
    match c {
        Calibrator::Polynomial(coefs) => {
            format!("PolynomialCalibrator[{}]", coefs.iter().map(|&x| java_double(x)).collect::<Vec<_>>().join(", "))
        }
        Calibrator::Spline(sc) => format!(
            "SplineCalibrator[{}]",
            sc.points
                .iter()
                .map(|p| format!("({},{})", java_double(p.raw), java_double(p.calibrated)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Calibrator::MathOperation(elements) => format!(
            "MathOperationCalibrator[{}]",
            elements
                .iter()
                .map(|e| match e {
                    MathOpElement::Value(x) => java_double(*x),
                    MathOpElement::ThisParameter => "this".to_owned(),
                    MathOpElement::Input(_) => "pref".to_owned(),
                    MathOpElement::Operator(op) => op.xtce_name().to_owned(),
                })
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Calibrator::Unsupported(descr) => descr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_like_java() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (-1.0, "-1.0"),
            (0.1, "0.1"),
            (0.001, "0.001"),
            (0.000999, "9.99E-4"),
            (1e-5, "1.0E-5"),
            (123.456, "123.456"),
            (1234567.0, "1234567.0"),
            (9999999.0, "9999999.0"),
            (1e7, "1.0E7"),
            (12345678.9, "1.23456789E7"),
            (1e10, "1.0E10"),
            (-2.5e-7, "-2.5E-7"),
            (1.7976931348623157e308, "1.7976931348623157E308"),
            (4.9e-324, "4.9E-324"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (0.30000000000000004, "0.30000000000000004"),
            (f64::from_bits(0xc316554ded775f99), "-1.5715606681251582E15"),
        ];
        for (x, s) in cases {
            assert_eq!(*s, java_double(*x), "{:e}", x);
        }
    }

    #[test]
    fn floats_like_java() {
        let cases: &[(f32, &str)] = &[
            (0.3, "0.3"),
            (1.0, "1.0"),
            (1e10, "1.0E10"),
            (3.4028235e38, "3.4028235E38"),
            (1.4e-45, "1.4E-45"),
            (16777216.0, "1.6777216E7"),
            (0.001, "0.001"),
            // ties between two shortest decimals: the even one
            (2924131.25, "2924131.2"),
            (36921.5625, "36921.562"),
        ];
        for (x, s) in cases {
            assert_eq!(*s, java_float(*x), "{:e}", x);
        }
        // a FLOAT value stored as f64 prints with the float precision
        assert_eq!("0.3", java_float((0.3f32 as f64) as f32));
        assert_eq!("0.30000001192092896", java_double(0.3f32 as f64));
    }
}
