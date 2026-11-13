use serde::de::{Deserialize, Deserializer, Error};
use serde_json::Value;

const TRUE_WORDS: [&str; 6] = ["1", "on", "t", "true", "y", "yes"];
const FALSE_WORDS: [&str; 6] = ["0", "off", "f", "false", "n", "no"];

fn digits_with_underscores(text: &str) -> Option<String> {
    let mut previous_digit = false;
    let mut digits = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '0'..='9' => {
                digits.push(character);
                previous_digit = true;
            }
            '_' if previous_digit => previous_digit = false,
            _ => return None,
        }
    }
    (previous_digit && !digits.is_empty()).then_some(digits)
}

fn integer_text(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    let (negative, unsigned) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
        Some(_) => return None,
        None => (unsigned, ""),
    };
    if !fraction.chars().all(|character| character == '0') {
        return None;
    }
    let magnitude: i64 = digits_with_underscores(whole)?.parse().ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

pub(super) fn lax_integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64().or_else(|| {
            number
                .as_f64()
                .filter(|float| float.is_finite() && float.fract() == 0.0)
                .and_then(|float| format!("{float:.0}").parse().ok())
        }),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::String(text) => integer_text(text),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn lax_boolean(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(_) => match lax_integer(value) {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        Value::String(text) => {
            let lowered = text.to_lowercase();
            if TRUE_WORDS.contains(&lowered.as_str()) {
                Some(true)
            } else if FALSE_WORDS.contains(&lowered.as_str()) {
                Some(false)
            } else {
                None
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    lax_integer(&Value::deserialize(deserializer)?)
        .ok_or_else(|| D::Error::custom("input should be a valid integer"))
}

pub(super) fn boolean<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    lax_boolean(&Value::deserialize(deserializer)?)
        .ok_or_else(|| D::Error::custom("input should be a valid boolean"))
}
