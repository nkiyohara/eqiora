use std::io::{self, Read};

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

pub(super) const MAX_LINE_BYTES: usize = 67_108_864;

pub(super) enum InputEvent {
    Line(Line),
    End,
    ReadFailure,
}

pub(super) enum Line {
    Message(Decoded),
    ParseFailure,
    Invalid { id: Option<Value> },
    Overlong,
}

pub(super) struct Decoded {
    pub(super) value: Value,
    pub(super) request_progress_token_is_integer: bool,
}

#[derive(Default)]
struct ScanState {
    duplicate: bool,
    too_deep: bool,
    top_id_count: usize,
    top_id: Option<Value>,
}

struct ValueSeed<'a> {
    state: &'a mut ScanState,
    parent_depth: usize,
}

struct ValueVisitor<'a> {
    state: &'a mut ScanState,
    parent_depth: usize,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor {
            state: self.state,
            parent_depth: self.parent_depth,
        })
    }
}

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.parent_depth + 1;
        if depth > 64 {
            self.state.too_deep = true;
        }
        let mut output = Vec::new();
        while let Some(value) = sequence.next_element_seed(ValueSeed {
            state: self.state,
            parent_depth: depth,
        })? {
            output.push(value);
        }
        Ok(Value::Array(output))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.parent_depth + 1;
        if depth > 64 {
            self.state.too_deep = true;
        }
        let mut output = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if depth == 1 && key == "id" {
                self.state.top_id_count += 1;
            }
            if output.contains_key(&key) {
                self.state.duplicate = true;
            }
            let value = object.next_value_seed(ValueSeed {
                state: self.state,
                parent_depth: depth,
            })?;
            if depth == 1 && key == "id" {
                self.state.top_id = Some(value.clone());
            }
            output.insert(key, value);
        }
        Ok(Value::Object(output))
    }
}

pub(super) fn read_lines<R, F>(mut reader: R, mut emit: F)
where
    R: Read,
    F: FnMut(InputEvent) -> bool,
{
    let mut buffer = [0_u8; 8192];
    let mut line = Vec::new();
    let mut draining = false;
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => {
                emit(InputEvent::End);
                return;
            }
            Ok(count) => count,
            Err(_) => {
                emit(InputEvent::ReadFailure);
                return;
            }
        };
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                let event = if draining {
                    Line::Overlong
                } else {
                    decode_line(std::mem::take(&mut line))
                };
                draining = false;
                line.clear();
                if !emit(InputEvent::Line(event)) {
                    return;
                }
            } else if !draining {
                line.push(*byte);
                if line.len() > MAX_LINE_BYTES + 1 {
                    line.clear();
                    draining = true;
                }
            }
        }
    }
}

fn decode_line(mut bytes: Vec<u8>) -> Line {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > MAX_LINE_BYTES {
        return Line::Overlong;
    }
    if bytes.contains(&b'\r') {
        return Line::Invalid { id: None };
    }
    if std::str::from_utf8(&bytes).is_err() {
        return Line::ParseFailure;
    }
    let mut state = ScanState::default();
    let mut decoder = serde_json::Deserializer::from_slice(&bytes);
    let decoded = (ValueSeed {
        state: &mut state,
        parent_depth: 0,
    })
    .deserialize(&mut decoder);
    let value = match decoded {
        Ok(value) if decoder.end().is_ok() => value,
        Ok(_) => return Line::ParseFailure,
        Err(_) if state.too_deep => {
            let id = if state.top_id_count == 1 {
                safe_id(state.top_id.as_ref())
            } else {
                None
            };
            return Line::Invalid { id };
        }
        Err(_) => return Line::ParseFailure,
    };
    if state.duplicate || state.too_deep {
        let id = if state.top_id_count == 1 {
            safe_id(value.get("id"))
        } else {
            None
        };
        return Line::Invalid { id };
    }
    let request_progress_token_is_integer = raw_object_member(&bytes, "params")
        .and_then(|params| raw_object_member(params, "_meta"))
        .and_then(|metadata| raw_object_member(metadata, "progressToken"))
        .is_some_and(exact_decimal_is_integer);
    Line::Message(Decoded {
        value,
        request_progress_token_is_integer,
    })
}

fn raw_object_member<'a>(input: &'a [u8], member: &str) -> Option<&'a [u8]> {
    let mut at = skip_json_whitespace(input, 0);
    if input.get(at) != Some(&b'{') {
        return None;
    }
    at += 1;
    loop {
        at = skip_json_whitespace(input, at);
        if input.get(at) == Some(&b'}') {
            return None;
        }
        let key_start = at;
        let key_end = json_string_end(input, key_start)?;
        let key = serde_json::from_slice::<String>(&input[key_start..key_end]).ok()?;
        at = skip_json_whitespace(input, key_end);
        if input.get(at) != Some(&b':') {
            return None;
        }
        at = skip_json_whitespace(input, at + 1);
        let value_start = at;
        let value_end = json_value_end(input, value_start)?;
        if key == member {
            return Some(&input[value_start..value_end]);
        }
        at = skip_json_whitespace(input, value_end);
        match input.get(at) {
            Some(b',') => at += 1,
            Some(b'}') => return None,
            _ => return None,
        }
    }
}

fn skip_json_whitespace(input: &[u8], mut at: usize) -> usize {
    while input
        .get(at)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        at += 1;
    }
    at
}

fn json_string_end(input: &[u8], start: usize) -> Option<usize> {
    if input.get(start) != Some(&b'"') {
        return None;
    }
    let mut at = start + 1;
    while let Some(byte) = input.get(at) {
        match byte {
            b'"' => return Some(at + 1),
            b'\\' => at += 2,
            _ => at += 1,
        }
    }
    None
}

fn json_value_end(input: &[u8], start: usize) -> Option<usize> {
    match input.get(start)? {
        b'"' => json_string_end(input, start),
        b'{' | b'[' => {
            let mut expected = vec![if input[start] == b'{' { b'}' } else { b']' }];
            let mut at = start + 1;
            while let Some(byte) = input.get(at) {
                match byte {
                    b'"' => at = json_string_end(input, at)?,
                    b'{' => {
                        expected.push(b'}');
                        at += 1;
                    }
                    b'[' => {
                        expected.push(b']');
                        at += 1;
                    }
                    b'}' | b']' if expected.last() == Some(byte) => {
                        expected.pop();
                        at += 1;
                        if expected.is_empty() {
                            return Some(at);
                        }
                    }
                    _ => at += 1,
                }
            }
            None
        }
        _ => {
            let mut at = start;
            while input.get(at).is_some_and(|byte| {
                !matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b'}' | b']')
            }) {
                at += 1;
            }
            (at > start).then_some(at)
        }
    }
}

fn exact_decimal_is_integer(input: &[u8]) -> bool {
    let unsigned = input.strip_prefix(b"-").unwrap_or(input);
    let exponent_at = unsigned.iter().position(|byte| matches!(byte, b'e' | b'E'));
    let coefficient = &unsigned[..exponent_at.unwrap_or(unsigned.len())];
    if coefficient
        .iter()
        .filter(|byte| **byte != b'.')
        .all(|byte| *byte == b'0')
    {
        return true;
    }
    let fractional_digits = coefficient
        .iter()
        .position(|byte| *byte == b'.')
        .map_or(0, |dot| coefficient.len() - dot - 1);
    let trailing_zeros = coefficient
        .iter()
        .rev()
        .filter(|byte| **byte != b'.')
        .take_while(|byte| **byte == b'0')
        .count();
    let Some(exponent_at) = exponent_at else {
        return trailing_zeros >= fractional_digits;
    };
    let exponent = &unsigned[exponent_at + 1..];
    let (negative, digits) = match exponent.first() {
        Some(b'-') => (true, &exponent[1..]),
        Some(b'+') => (false, &exponent[1..]),
        _ => (false, exponent),
    };
    if negative {
        trailing_zeros >= fractional_digits
            && decimal_at_most(digits, trailing_zeros - fractional_digits)
    } else {
        trailing_zeros >= fractional_digits
            || decimal_at_least(digits, fractional_digits - trailing_zeros)
    }
}

fn decimal_at_least(digits: &[u8], bound: usize) -> bool {
    compare_decimal_with_usize(digits, bound).is_ge()
}

fn decimal_at_most(digits: &[u8], bound: usize) -> bool {
    compare_decimal_with_usize(digits, bound).is_le()
}

fn compare_decimal_with_usize(digits: &[u8], bound: usize) -> std::cmp::Ordering {
    let digits = digits
        .iter()
        .position(|digit| *digit != b'0')
        .map_or(&digits[digits.len()..], |first| &digits[first..]);
    let bound = bound.to_string();
    digits
        .len()
        .cmp(&bound.len())
        .then_with(|| digits.cmp(bound.as_bytes()))
}

pub(super) fn safe_id(value: Option<&Value>) -> Option<Value> {
    match value {
        Some(Value::String(text)) if text.chars().count() <= 128 && text.len() <= 128 => {
            Some(Value::String(text.clone()))
        }
        Some(Value::Number(number)) if number.as_i64().is_some() => {
            Some(Value::Number(number.clone()))
        }
        _ => None,
    }
}

pub(super) fn reader_failure() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "MCP input failure")
}
