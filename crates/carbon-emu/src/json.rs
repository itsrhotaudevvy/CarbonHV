//! A strict, allocation-light JSON reader for the two places the emulated engine needs one:
//! host-facts fixtures from `carbonctl` and QMP replies.
//!
//! There is no `serde` in this workspace, and both inputs are small, untrusted and schema-checked
//! by hand, so a focused parser is safer than a new dependency: it rejects trailing data,
//! duplicate keys, control characters in strings and input past an explicit size cap.

use std::fmt::Write as _;

/// Largest document either caller may hand us.
pub const MAX_DOCUMENT_BYTES: usize = 1 << 20;
/// Deepest nesting either caller may hand us.
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// Insertion-ordered, duplicate-free; QMP replies keep a stable key order for tests.
    Object(Vec<(String, Value)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    TooLarge,
    TooDeep,
    UnexpectedEnd,
    TrailingData,
    UnterminatedString,
    InvalidEscape,
    InvalidNumber,
    InvalidLiteral,
    DuplicateKey(String),
    ControlCharacterInString,
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Numbers are parsed as `f64`, so an integer accessor refuses values that are not exact.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(value)
                if value.is_finite()
                    && *value >= 0.0
                    && value.fract() == 0.0
                    && *value < 2f64.powi(53) =>
            {
                Some(*value as u64)
            }
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(value)
                if value.is_finite() && value.fract() == 0.0 && value.abs() < 2f64.powi(53) =>
            {
                Some(*value as i64)
            }
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(values) => Some(values),
            _ => None,
        }
    }

    /// Serialises for the QMP command channel. Object keys keep insertion order.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Number(value) => {
                if value.fract() == 0.0 && value.abs() < 2f64.powi(53) {
                    let _ = write!(out, "{}", *value as i64);
                } else {
                    let _ = write!(out, "{value}");
                }
            }
            Value::String(value) => write_json_string(out, value),
            Value::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    value.write_json(out);
                }
                out.push(']');
            }
            Value::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write_json_string(out, key);
                    out.push(':');
                    value.write_json(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

pub fn parse(document: &str) -> Result<Value, JsonError> {
    if document.len() > MAX_DOCUMENT_BYTES {
        return Err(JsonError::TooLarge);
    }
    let mut parser = Parser {
        bytes: document.as_bytes(),
        index: 0,
        depth: 0,
    };
    parser.skip_whitespace();
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.index != parser.bytes.len() {
        return Err(JsonError::TrailingData);
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.index += 1;
        }
    }

    fn value(&mut self) -> Result<Value, JsonError> {
        match self.peek().ok_or(JsonError::UnexpectedEnd)? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(Value::String),
            b't' | b'f' => self.boolean(),
            b'n' => self.null(),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(JsonError::InvalidLiteral),
        }
    }

    fn enter(&mut self) -> Result<(), JsonError> {
        if self.depth >= MAX_DEPTH {
            return Err(JsonError::TooDeep);
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn object(&mut self) -> Result<Value, JsonError> {
        self.enter()?;
        self.index += 1; // '{'
        let mut entries: Vec<(String, Value)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.index += 1;
            self.leave();
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(JsonError::InvalidLiteral);
            }
            let key = self.string()?;
            if entries.iter().any(|(existing, _)| existing == &key) {
                return Err(JsonError::DuplicateKey(key));
            }
            self.skip_whitespace();
            if self.bump() != Some(b':') {
                return Err(JsonError::InvalidLiteral);
            }
            self.skip_whitespace();
            let value = self.value()?;
            entries.push((key, value));
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => {
                    self.leave();
                    return Ok(Value::Object(entries));
                }
                _ => return Err(JsonError::InvalidLiteral),
            }
        }
    }

    fn array(&mut self) -> Result<Value, JsonError> {
        self.enter()?;
        self.index += 1; // '['
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.index += 1;
            self.leave();
            return Ok(Value::Array(values));
        }
        loop {
            self.skip_whitespace();
            values.push(self.value()?);
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => {
                    self.leave();
                    return Ok(Value::Array(values));
                }
                _ => return Err(JsonError::InvalidLiteral),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        if self.bump() != Some(b'"') {
            return Err(JsonError::InvalidLiteral);
        }
        let mut out = String::new();
        loop {
            match self.bump().ok_or(JsonError::UnterminatedString)? {
                b'"' => return Ok(out),
                b'\\' => match self.bump().ok_or(JsonError::UnterminatedString)? {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let code = self.hex4()?;
                        // Surrogates are not needed by either caller; refuse rather than guess.
                        let ch = char::from_u32(u32::from(code)).ok_or(JsonError::InvalidEscape)?;
                        out.push(ch);
                    }
                    _ => return Err(JsonError::InvalidEscape),
                },
                byte if byte < 0x20 => return Err(JsonError::ControlCharacterInString),
                byte if byte < 0x80 => out.push(byte as char),
                byte => {
                    // Copy one UTF-8 sequence from the original bytes.
                    let start = self.index - 1;
                    let width = utf8_width(byte);
                    let end = start + width;
                    if end > self.bytes.len() {
                        return Err(JsonError::UnterminatedString);
                    }
                    let text = std::str::from_utf8(&self.bytes[start..end])
                        .map_err(|_| JsonError::UnterminatedString)?;
                    out.push_str(text);
                    self.index = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, JsonError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self.bump().ok_or(JsonError::UnterminatedString)?;
            let digit = (byte as char)
                .to_digit(16)
                .ok_or(JsonError::InvalidEscape)?;
            value = (value << 4) | digit as u16;
        }
        Ok(value)
    }

    fn boolean(&mut self) -> Result<Value, JsonError> {
        if self.bytes[self.index..].starts_with(b"true") {
            self.index += 4;
            Ok(Value::Bool(true))
        } else if self.bytes[self.index..].starts_with(b"false") {
            self.index += 5;
            Ok(Value::Bool(false))
        } else {
            Err(JsonError::InvalidLiteral)
        }
    }

    fn null(&mut self) -> Result<Value, JsonError> {
        if self.bytes[self.index..].starts_with(b"null") {
            self.index += 4;
            Ok(Value::Null)
        } else {
            Err(JsonError::InvalidLiteral)
        }
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.index;
        if self.peek() == Some(b'-') {
            self.index += 1;
        }
        let mut digits = 0;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.index += 1;
            digits += 1;
        }
        if digits == 0 {
            return Err(JsonError::InvalidNumber);
        }
        if self.peek() == Some(b'.') {
            self.index += 1;
            let mut fraction = 0;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
                fraction += 1;
            }
            if fraction == 0 {
                return Err(JsonError::InvalidNumber);
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            let mut exponent = 0;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
                exponent += 1;
            }
            if exponent == 0 {
                return Err(JsonError::InvalidNumber);
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.index])
            .map_err(|_| JsonError::InvalidNumber)?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| JsonError::InvalidNumber)
    }
}

fn utf8_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_object_and_round_trips() {
        let value = parse(r#"{"a":1,"b":[true,null,"x\n"],"c":{"d":-2.5}}"#).unwrap();
        assert_eq!(value.get("a").and_then(Value::as_u64), Some(1));
        assert_eq!(
            value.get("b").and_then(Value::as_array).map(<[Value]>::len),
            Some(3)
        );
        assert_eq!(
            value.to_json(),
            r#"{"a":1,"b":[true,null,"x\n"],"c":{"d":-2.5}}"#
        );
    }

    #[test]
    fn rejects_trailing_data_and_duplicate_keys() {
        assert_eq!(parse("{} {}"), Err(JsonError::TrailingData));
        assert_eq!(
            parse(r#"{"a":1,"a":2}"#),
            Err(JsonError::DuplicateKey("a".into()))
        );
    }

    #[test]
    fn rejects_deep_nesting_and_control_characters() {
        let deep = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert_eq!(parse(&deep), Err(JsonError::TooDeep));
        assert_eq!(
            parse("\"a\u{1}b\""),
            Err(JsonError::ControlCharacterInString)
        );
    }

    #[test]
    fn refuses_non_integral_numbers_as_integers() {
        let value = parse("1.5").unwrap();
        assert_eq!(value.as_u64(), None);
        assert_eq!(value.as_i64(), None);
    }
}
