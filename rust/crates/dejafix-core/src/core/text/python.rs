use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use super::types::PythonLines;

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static DECIMAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\d+\z").expect("valid decimal pattern"));

#[must_use]
pub fn is_python_space(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}

#[must_use]
pub fn python_strip(text: &str) -> &str {
    text.trim_matches(is_python_space)
}

pub fn python_strip_matches<'s>(text: &'s str, characters: &[char]) -> &'s str {
    text.trim_matches(|character| characters.contains(&character))
}

pub fn is_python_decimal(text: &str) -> bool {
    DECIMAL.is_match(text)
}

pub fn zfill(text: &str, width: usize) -> Cow<'_, str> {
    let length = text.chars().count();
    if length >= width {
        return Cow::Borrowed(text);
    }
    let mut padded = "0".repeat(width - length);
    padded.push_str(text);
    Cow::Owned(padded)
}

#[must_use]
pub fn python_lines(bytes: &[u8]) -> PythonLines<'_> {
    PythonLines {
        bytes,
        start: 0,
        number: 0,
    }
}

impl<'a> Iterator for PythonLines<'a> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.bytes.get(self.start..)?;
        if rest.is_empty() {
            return None;
        }
        let mut from = 0;
        let (length, separator) = loop {
            let Some(found) = rest
                .get(from..)
                .and_then(|tail| tail.iter().position(|byte| may_separate(*byte)))
            else {
                break (rest.len(), 0);
            };
            let index = from + found;
            if let Some(width) = separator_at(rest, index) {
                break (index, width);
            }
            from = index + 1;
        };
        self.start += length + separator;
        self.number += 1;
        Some((self.number, rest.get(..length).unwrap_or_default()))
    }
}

fn may_separate(byte: u8) -> bool {
    matches!(
        byte,
        b'\n' | b'\r' | 0x0b | 0x0c | 0x1c..=0x1e | 0xc2 | 0xe2
    )
}

fn separator_at(bytes: &[u8], index: usize) -> Option<usize> {
    let tail = bytes.get(index..)?;
    match tail {
        [b'\r', b'\n', ..] | [0xc2, 0x85, ..] => Some(2),
        [b'\n' | b'\r' | 0x0b | 0x0c | 0x1c | 0x1d | 0x1e, ..] => Some(1),
        [0xe2, 0x80, 0xa8 | 0xa9, ..] => Some(3),
        _ => None,
    }
}
