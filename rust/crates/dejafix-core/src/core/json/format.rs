use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{Formatter, Serializer};
use serde_json::value::RawValue;

use super::types::{PythonJsonFormatter, Sorted};

impl Formatter for PythonJsonFormatter {
    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }

    fn write_f64<W: ?Sized + Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        let scientific = format!("{value:e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
        let exponent: i32 = exponent.parse().unwrap_or(0);
        if (-4..16).contains(&exponent) {
            let fixed = value.to_string();
            let fraction = if fixed.contains('.') { "" } else { ".0" };
            write!(writer, "{fixed}{fraction}")
        } else {
            let sign = if exponent < 0 { '-' } else { '+' };
            write!(writer, "{mantissa}e{sign}{:02}", exponent.unsigned_abs())
        }
    }

    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut start = 0;
        let mut units = [0_u16; 2];
        for (index, character) in fragment.char_indices() {
            if (' '..='~').contains(&character) {
                continue;
            }
            writer.write_all(fragment.get(start..index).unwrap_or_default().as_bytes())?;
            for unit in character.encode_utf16(&mut units) {
                write!(writer, "\\u{unit:04x}")?;
            }
            start = index + character.len_utf8();
        }
        writer.write_all(fragment.get(start..).unwrap_or_default().as_bytes())
    }
}

pub(crate) fn write_python_json<W: Write, T: Serialize + ?Sized>(
    writer: &mut W,
    value: &T,
) -> Result<(), serde_json::Error> {
    value.serialize(&mut Serializer::with_formatter(writer, PythonJsonFormatter))
}

pub(crate) fn write_canonical<W: Write>(
    writer: &mut W,
    value: &Value,
) -> Result<(), serde_json::Error> {
    write_python_json(writer, &Sorted(value))
}

pub(crate) fn is_dropped_by_to_dict(raw: &RawValue) -> bool {
    let text = raw.get();
    match text.as_bytes().first() {
        Some(b'n' | b'f') => true,
        Some(b'[') => is_empty_container(text),
        Some(b'-' | b'0'..=b'9') => is_zero(text),
        _ => false,
    }
}

pub(crate) fn is_python_truthy(raw: &RawValue) -> bool {
    let text = raw.get();
    match text.as_bytes().first() {
        Some(b'n' | b'f') => false,
        Some(b'[' | b'{') => !is_empty_container(text),
        Some(b'"') => text != "\"\"",
        Some(b'-' | b'0'..=b'9') => !is_zero(text),
        _ => true,
    }
}

fn is_empty_container(text: &str) -> bool {
    text.get(1..)
        .unwrap_or_default()
        .trim_start_matches([' ', '\t', '\n', '\r'])
        .len()
        == 1
}

fn is_zero(text: &str) -> bool {
    serde_json::from_str::<f64>(text).is_ok_and(|number| number == 0.0)
}

impl Serialize for Sorted<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Array(items) => serializer.collect_seq(items.iter().map(Sorted)),
            Value::Object(map) => {
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
                serializer.collect_map(entries.into_iter().map(|(key, value)| (key, Sorted(value))))
            }
            scalar => scalar.serialize(serializer),
        }
    }
}

pub(crate) fn python_str(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::String(text) => text.clone(),
        Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            let mut out = Vec::new();
            match write_python_json(&mut out, value) {
                Ok(()) => String::from_utf8_lossy(&out).into_owned(),
                Err(_) => String::new(),
            }
        }
    }
}
