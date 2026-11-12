use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{Formatter, Serializer};

use super::types::{PythonIndentFormatter, PythonJsonFormatter, Sorted};

impl PythonIndentFormatter {
    fn newline<W: ?Sized + Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b"\n")?;
        for _ in 0..self.depth * self.indent {
            writer.write_all(b" ")?;
        }
        Ok(())
    }

    fn open<W: ?Sized + Write>(&mut self, writer: &mut W, bracket: &[u8]) -> io::Result<()> {
        self.depth += 1;
        self.has_value = false;
        writer.write_all(bracket)
    }

    fn close<W: ?Sized + Write>(&mut self, writer: &mut W, bracket: &[u8]) -> io::Result<()> {
        self.depth = self.depth.saturating_sub(1);
        if self.has_value {
            self.newline(writer)?;
        }
        writer.write_all(bracket)
    }

    fn item<W: ?Sized + Write>(&self, writer: &mut W, first: bool) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        self.newline(writer)
    }
}

impl Formatter for PythonIndentFormatter {
    fn begin_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.open(writer, b"[")
    }

    fn end_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.close(writer, b"]")
    }

    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.item(writer, first)
    }

    fn end_array_value<W: ?Sized + Write>(&mut self, _writer: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }

    fn begin_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.open(writer, b"{")
    }

    fn end_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.close(writer, b"}")
    }

    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.item(writer, first)
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }

    fn end_object_value<W: ?Sized + Write>(&mut self, _writer: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }

    fn write_f64<W: ?Sized + Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        PythonJsonFormatter.write_f64(writer, value)
    }

    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        PythonJsonFormatter.write_string_fragment(writer, fragment)
    }
}

pub fn write_python_json_indented<W: Write>(
    writer: &mut W,
    value: &Value,
    indent: usize,
    sort_keys: bool,
) -> Result<(), serde_json::Error> {
    let formatter = PythonIndentFormatter {
        indent,
        depth: 0,
        has_value: false,
    };
    let mut serializer = Serializer::with_formatter(writer, formatter);
    if sort_keys {
        Sorted(value).serialize(&mut serializer)
    } else {
        value.serialize(&mut serializer)
    }
}
