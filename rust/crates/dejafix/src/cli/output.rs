use std::fmt::Write as _;
use std::io::{self, Write};

use dejafix_core::WiringError;

use super::types::{Outcome, Style};

pub const OK: &str = "✓";
pub const FAIL: &str = "✗";
pub const WARN: &str = "⚠";
pub const RUNNING: &str = "●";
pub const ARROW: &str = "→";
pub const RULE_DOUBLE: &str = "═";

impl Style {
    fn code(self) -> &'static str {
        match self {
            Self::Red => "31",
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Cyan => "36",
        }
    }
}

pub fn secho(out: &mut impl Write, text: &str, style: Style, color: bool) -> io::Result<()> {
    if color {
        writeln!(out, "\x1b[{}m{text}\x1b[0m", style.code())
    } else {
        writeln!(out, "{text}")
    }
}

impl From<Result<(), WiringError>> for Outcome {
    fn from(result: Result<(), WiringError>) -> Self {
        match result {
            Ok(()) => Self::Done,
            Err(error) => Self::Failed(error),
        }
    }
}

pub fn json_escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{8}' => escaped.push_str("\\b"),
            '\u{c}' => escaped.push_str("\\f"),
            ' '..='~' => escaped.push(character),
            other => {
                let mut units = [0_u16; 2];
                for unit in other.encode_utf16(&mut units) {
                    let _ = write!(escaped, "\\u{unit:04x}");
                }
            }
        }
    }
    escaped
}
