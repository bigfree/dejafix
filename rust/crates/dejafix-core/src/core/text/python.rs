use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use super::types::{DecimalKey, PythonLines};

pub const PYTHON_SPACE_CLASS: &str = r"[\t\n\x0B\x0C\r\x1C-\x1F \x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";

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

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[\p{L}\p{N}_]\z").expect("valid word pattern"));

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static ONE_DECIMAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\p{Nd}\z").expect("valid digit pattern"));

pub fn is_python_decimal(text: &str) -> bool {
    DECIMAL.is_match(text)
}

#[must_use]
pub fn is_python_word(character: char) -> bool {
    if character.is_ascii() {
        return character.is_ascii_alphanumeric() || character == '_';
    }
    let mut buffer = [0; 4];
    WORD.is_match(character.encode_utf8(&mut buffer))
}

fn is_unicode_decimal(character: char) -> bool {
    let mut buffer = [0; 4];
    ONE_DECIMAL.is_match(character.encode_utf8(&mut buffer))
}

fn digit_value(character: char) -> Option<u8> {
    if let Some(value) = character.to_digit(10) {
        return u8::try_from(value).ok();
    }
    if !is_unicode_decimal(character) {
        return None;
    }
    let code = u32::from(character);
    let mut start = code;
    while let Some(previous) = start.checked_sub(1).and_then(char::from_u32)
        && is_unicode_decimal(previous)
    {
        start -= 1;
    }
    u8::try_from((code - start) % 10).ok()
}

#[must_use]
pub fn decimal_value(text: &str) -> Option<DecimalKey> {
    if text.is_empty() {
        return None;
    }
    let digits = text.chars().map(digit_value).collect::<Option<Vec<u8>>>()?;
    let first = digits
        .iter()
        .position(|digit| *digit != 0)
        .unwrap_or(digits.len());
    let digits = digits.get(first..).unwrap_or_default().to_vec();
    Some(DecimalKey {
        length: digits.len(),
        digits,
    })
}

#[must_use]
pub fn python_int(text: &str) -> Option<i64> {
    let text = python_strip(text);
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, text.get(1..)?),
        Some(b'+') => (false, text.get(1..)?),
        _ => (false, text),
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return None;
    }
    let magnitude = digits
        .chars()
        .filter(|character| *character != '_')
        .try_fold(0_i64, |value, character| {
            value
                .checked_mul(10)?
                .checked_add(i64::from(digit_value(character)?))
        })?;
    Some(if negative { -magnitude } else { magnitude })
}

pub fn python_split(text: &str) -> impl Iterator<Item = &str> {
    text.split(is_python_space).filter(|part| !part.is_empty())
}

#[must_use]
pub fn collapse_whitespace(text: &str) -> Cow<'_, str> {
    let collapsed = !text.starts_with(is_python_space)
        && !text.ends_with(is_python_space)
        && !text.char_indices().any(|(index, character)| {
            is_python_space(character)
                && (character != ' '
                    || text
                        .get(index + 1..)
                        .is_some_and(|rest| rest.starts_with(is_python_space)))
        });
    if collapsed {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for (index, part) in python_split(text).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(part);
    }
    Cow::Owned(out)
}

#[must_use]
pub fn char_prefix(text: &str, count: usize) -> &str {
    text.char_indices()
        .nth(count)
        .and_then(|(index, _)| text.get(..index))
        .unwrap_or(text)
}

#[must_use]
pub fn char_len(text: &str) -> usize {
    text.chars().count()
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

impl DecimalKey {
    #[must_use]
    pub fn saturating_u32(&self) -> u32 {
        self.digits.iter().fold(0_u32, |value, digit| {
            value.saturating_mul(10).saturating_add(u32::from(*digit))
        })
    }

    #[must_use]
    pub fn successor(&self, width: usize) -> String {
        let mut digits = self.digits.clone();
        let mut carry = true;
        for digit in digits.iter_mut().rev() {
            if !carry {
                break;
            }
            if *digit == 9 {
                *digit = 0;
            } else {
                *digit += 1;
                carry = false;
            }
        }
        if carry {
            digits.insert(0, 1);
        }
        let text: String = digits
            .iter()
            .map(|digit| char::from(b'0' + digit))
            .collect();
        zfill(&text, width).into_owned()
    }
}

#[must_use]
pub fn universal_newlines(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}

#[must_use]
pub fn platform_newlines(text: &str) -> Cow<'_, str> {
    if cfg!(windows) && text.contains('\n') {
        Cow::Owned(text.replace('\n', "\r\n"))
    } else {
        Cow::Borrowed(text)
    }
}

#[must_use]
pub fn contains_word(haystack: &str, needle: &str) -> bool {
    let (Some(first), Some(last)) = (needle.chars().next(), needle.chars().next_back()) else {
        return false;
    };
    haystack.match_indices(needle).any(|(at, _)| {
        let before = haystack.get(..at).and_then(|head| head.chars().next_back());
        let after = haystack
            .get(at + needle.len()..)
            .and_then(|tail| tail.chars().next());
        before.is_some_and(is_python_word) != is_python_word(first)
            && is_python_word(last) != after.is_some_and(is_python_word)
    })
}
