//! Minimal JSON parser used to convert the string values of the aggregate and array arguments
//! (e.g. `{"x": 1, "y": [1, 2]}`), like Yamcs does.

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    String(String),
    /// number, true, false or null as written in the source
    Literal(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// the text passed to the conversion of the element type: the content of the strings, the JSON text otherwise
    pub(crate) fn as_arg_string(&self) -> String {
        match self {
            Json::String(s) => s.clone(),
            _ => self.to_string(),
        }
    }
}

impl std::fmt::Display for Json {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Json::String(s) => {
                write!(f, "\"")?;
                for c in s.chars() {
                    match c {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\r' => write!(f, "\\r")?,
                        '\t' => write!(f, "\\t")?,
                        c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
                        c => write!(f, "{}", c)?,
                    }
                }
                write!(f, "\"")
            }
            Json::Literal(s) => write!(f, "{}", s),
            Json::Array(a) => {
                write!(f, "[")?;
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", x)?;
                }
                write!(f, "]")
            }
            Json::Object(o) => {
                write!(f, "{{")?;
                for (i, (k, x)) in o.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}:{}", Json::String(k.clone()), x)?;
                }
                write!(f, "}}")
            }
        }
    }
}

const MAX_DEPTH: usize = 64;

pub(crate) fn parse(s: &str) -> Result<Json, String> {
    let mut p = Parser { s: s.as_bytes(), pos: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.pos != p.s.len() {
        return Err(format!("unexpected characters at position {}", p.pos));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at position {}", c as char, self.pos))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err("too many nesting levels".into());
        }
        self.ws();
        match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                let mut members = Vec::new();
                self.ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(Json::Object(members));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.expect(b':')?;
                    let v = self.value(depth + 1)?;
                    members.push((k, v));
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Json::Object(members));
                        }
                        _ => return Err(format!("expected ',' or '}}' at position {}", self.pos)),
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut values = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Array(values));
                        }
                        _ => return Err(format!("expected ',' or ']' at position {}", self.pos)),
                    }
                }
            }
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(c) if c == b'-' || c.is_ascii_alphanumeric() => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.') {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                // the input is a &str and the literal contains only ASCII characters
                let lit = std::str::from_utf8(&self.s[start..self.pos]).unwrap();
                let valid = matches!(lit, "true" | "false" | "null") || lit.parse::<f64>().is_ok();
                if !valid {
                    return Err(format!("invalid literal '{}'", lit));
                }
                Ok(Json::Literal(lit.to_owned()))
            }
            Some(c) => Err(format!("unexpected character '{}' at position {}", c as char, self.pos)),
            None => Err("unexpected end of input".into()),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected string at position {}", self.pos));
        }
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek().ok_or("unterminated string")?;
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or("unterminated string")?;
                    self.pos += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut code = self.hex4()?;
                            // surrogate pair
                            if (0xD800..0xDC00).contains(&code) && self.s[self.pos..].starts_with(b"\\u") {
                                self.pos += 2;
                                let low = self.hex4()?;
                                code = 0x10000 + ((code - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return Err(format!("invalid escape at position {}", self.pos)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => out.push(c),
            }
        }
        // the input is valid UTF-8 and the escapes produce valid UTF-8
        String::from_utf8(out).map_err(|e| e.to_string())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.s.get(self.pos..self.pos + 4).ok_or("invalid unicode escape")?;
        let h = std::str::from_utf8(h).map_err(|e| e.to_string())?;
        let v = u32::from_str_radix(h, 16).map_err(|_| format!("invalid unicode escape '{}'", h))?;
        self.pos += 4;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_values() {
        let j = parse(r#" {"x": 1, "y": [1.5, -2e3, "a\"b"], "z": {"t": true}} "#).unwrap();
        assert_eq!(r#"{"x":1,"y":[1.5,-2e3,"a\"b"],"z":{"t":true}}"#, j.to_string());
        let Json::Object(m) = j else { panic!() };
        assert_eq!("1", m[0].1.as_arg_string());
        assert_eq!("[1.5,-2e3,\"a\\\"b\"]", m[1].1.as_arg_string());
        assert_eq!(Json::String("é€😀".into()), parse(r#""\u00e9\u20ac\ud83d\ude00""#).unwrap());
    }

    #[test]
    fn parse_errors() {
        for s in ["", "{", "[1,", "{\"a\" 1}", "[1 2]", "tru", "\"abc", "{1: 2}", "[1]x", "\"\\u12\""] {
            assert!(parse(s).is_err(), "{}", s);
        }
        let deep = "[".repeat(1000);
        assert!(parse(&deep).is_err());
    }
}
