use std::borrow::Cow;
use std::collections::HashSet;

use crate::core::event::Event;

#[derive(Debug, Clone, Copy)]
pub struct ContextInput<'i> {
    pub budget: u32,
    pub focus: Option<&'i str>,
    pub recent_days: i64,
    pub now: jiff::Timestamp,
    pub git_files: &'i [String],
    pub project_map: Option<&'i str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Level {
    Full,
    Compressed,
    Ultra,
    Emergency,
}

pub(super) struct Signals<'s> {
    pub(super) now: jiff::Timestamp,
    pub(super) cutoff: jiff::Timestamp,
    pub(super) fallback: jiff::Timestamp,
    pub(super) focus: Option<&'s str>,
    pub(super) git: HashSet<&'s str>,
}

pub(super) struct Scored<'l, 'e> {
    pub(super) score: f64,
    pub(super) event: &'l Event<'e>,
}

pub(super) struct FileNotes<'f> {
    pub(super) file: Cow<'f, str>,
    pub(super) count: usize,
    pub(super) first: String,
}
