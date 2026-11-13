use std::borrow::Cow;
use std::sync::LazyLock;

use jiff::SignedDuration;
use jiff::tz::TimeZone;
use regex::Regex;

use super::types::SnoozeError;
use crate::core::text::{PYTHON_SPACE_CLASS, decimal_value, python_strip};
use crate::core::time::normalize_timestamp;

static DURATION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(&format!(r"\A(\d+){PYTHON_SPACE_CLASS}*([mhdMHD])\z")).ok());

pub fn parse_snooze(text: &str) -> Result<SignedDuration, SnoozeError> {
    let invalid = || SnoozeError::Invalid(text.to_owned());
    let found = DURATION
        .as_ref()
        .and_then(|pattern| pattern.captures(python_strip(text)))
        .ok_or_else(invalid)?;
    let value = found
        .get(1)
        .and_then(|digits| decimal_value(digits.as_str()))
        .ok_or_else(invalid)?
        .saturating_u32();
    if value == 0 {
        return Err(SnoozeError::NotPositive);
    }
    let unit = match found.get(2).map(|unit| unit.as_str()) {
        Some("m" | "M") => 60,
        Some("h" | "H") => 3_600,
        _ => 86_400,
    };
    Ok(SignedDuration::from_secs(i64::from(value) * unit))
}

pub fn snoozed_until(
    now: jiff::Timestamp,
    duration: SignedDuration,
    text: &str,
) -> Result<jiff::Timestamp, SnoozeError> {
    jiff::Timestamp::from_second(now.as_second())
        .and_then(|start| start.checked_add(duration))
        .map_err(|_| SnoozeError::Invalid(text.to_owned()))
}

#[must_use]
pub fn expiry_iso(expiry: jiff::Timestamp) -> String {
    TimeZone::UTC
        .to_datetime(expiry)
        .strftime("%Y-%m-%dT%H:%M:%S+00:00")
        .to_string()
}

#[must_use]
pub fn snooze_note(duration: &str, expiry: jiff::Timestamp) -> String {
    format!(
        "precheck warnings snoozed for {duration} (until {})",
        expiry_iso(expiry)
    )
}

#[must_use]
pub fn read_expiry(text: &str) -> Option<jiff::Timestamp> {
    let stripped = python_strip(text);
    if stripped.is_empty() {
        return None;
    }
    let parsed = normalize_timestamp(Cow::Borrowed(stripped), &TimeZone::UTC).ok()?;
    parsed
        .utc()?
        .to_zoned(TimeZone::UTC)
        .ok()
        .map(|zoned| zoned.timestamp())
}

#[must_use]
pub fn remaining(expiry: jiff::Timestamp, now: jiff::Timestamp) -> String {
    let micros = expiry.duration_since(now).as_micros();
    let minutes = micros.div_euclid(60_000_000).max(1);
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h{rest:02}m")
    }
}
