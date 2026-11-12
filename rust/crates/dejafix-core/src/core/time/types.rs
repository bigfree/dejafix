use std::borrow::Cow;

use jiff::civil::DateTime;
use jiff::tz::TimeZone;

#[derive(Debug, Clone)]
pub struct Clock {
    pub now: jiff::Timestamp,
    pub zone: TimeZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp<'a> {
    pub(super) text: Cow<'a, str>,
    pub(super) utc: Option<DateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("timestamp leaves the years 1-9999 in UTC")]
pub struct TimestampOverflow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Parsed {
    pub(super) civil: DateTime,
    pub(super) offset_micros: Option<i64>,
}

pub(super) struct Bytes<'b>(pub(super) &'b [u8]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ClockTime {
    pub(super) hour: i64,
    pub(super) minute: i64,
    pub(super) second: i64,
    pub(super) micros: i64,
}
