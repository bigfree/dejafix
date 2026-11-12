use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{Formatter, Serializer};

use super::types::PythonJsonFormatter;

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

pub(crate) fn write_python_json<W: Write>(
    writer: &mut W,
    value: &Value,
) -> Result<(), serde_json::Error> {
    value.serialize(&mut Serializer::with_formatter(
        &mut *writer,
        PythonJsonFormatter,
    ))?;
    writer.write_all(b"\n").map_err(serde_json::Error::io)
}
