use jiff::tz::TimeZone;

use super::types::{NoResolve, SeqIds};
use crate::core::event::EventId;
use crate::core::path::PathResolver;
use crate::core::time::Clock;
use crate::core::write::Ids;

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

impl Ids for SeqIds {
    fn event_id(&mut self) -> EventId<'static> {
        self.0 += 1;
        EventId::from_string(format!("evt_{:020}", self.0))
    }

    fn entry_id(&mut self, prefix: &'static str) -> String {
        self.0 += 1;
        format!("{prefix}_{:012}", self.0)
    }
}
