use std::borrow::Cow;

use jiff::tz::TimeZone;
use serde_json::Value;

use super::inherit::is_truthy;
use super::types::Entry;
use crate::core::time::normalize_timestamp;

const MICROS_PER_SECOND: i128 = 1_000_000;
const MICROS_PER_DAY: i128 = 86_400 * MICROS_PER_SECOND;

fn entry_micros(entry: &Entry, zone: &TimeZone) -> i128 {
    let stamp = ["created", "discovered"]
        .iter()
        .filter_map(|key| entry.get(*key))
        .find(|value| is_truthy(value));
    let Some(Value::String(text)) = stamp else {
        return 0;
    };
    normalize_timestamp(Cow::Borrowed(text), zone)
        .ok()
        .and_then(|stamp| stamp.utc())
        .and_then(|civil| TimeZone::UTC.to_timestamp(civil).ok())
        .map_or(0, |at| i128::from(at.as_second()) * MICROS_PER_SECOND)
}

#[must_use]
pub fn is_prunable(
    entry: &Entry,
    now: jiff::Timestamp,
    zone: &TimeZone,
    older_than: i64,
    confidence: Option<&str>,
) -> bool {
    let cutoff = i128::from(now.as_microsecond()) - i128::from(older_than) * MICROS_PER_DAY;
    entry_micros(entry, zone) < cutoff
        && confidence
            .is_none_or(|wanted| entry.get("confidence").and_then(Value::as_str) == Some(wanted))
}
