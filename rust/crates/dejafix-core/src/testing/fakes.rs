use jiff::tz::TimeZone;

use super::types::NoResolve;
use crate::core::path::PathResolver;
use crate::core::time::Clock;

pub const FIXED_NOW: i64 = 1_800_000_000;

impl PathResolver for NoResolve {
    fn resolve(&self, _: &str) -> Option<String> {
        None
    }
}

#[must_use]
pub fn fixed_clock() -> Clock {
    Clock {
        now: jiff::Timestamp::from_second(FIXED_NOW).unwrap_or(jiff::Timestamp::UNIX_EPOCH),
        zone: TimeZone::UTC,
    }
}
