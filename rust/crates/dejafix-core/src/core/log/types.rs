use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::core::event::{Event, EventId, IssueId, LineError};
use crate::core::path::{PathContext, RelativePath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skipped {
    pub line: usize,
    pub error: LineError,
}

#[derive(Debug, Default)]
pub struct EventLog<'a> {
    pub(super) events: Vec<Event<'a>>,
    pub(super) skipped: Vec<Skipped>,
    pub(super) closed: HashSet<IssueId<'a>>,
    pub(super) open: Vec<usize>,
    pub(super) retired: HashSet<EventId<'a>>,
    pub(super) files: HashMap<RelativePath<'a>, Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventRefError {
    #[error("Empty event reference")]
    Empty,
    #[error(
        "No event matches '{0}'. Use `djf search <text>` to find the event id (shown as evt_...)."
    )]
    NotFound(String),
    #[error(
        "Event reference '{reference}' is ambiguous ({count} matches) — use more characters of the id."
    )]
    Ambiguous { reference: String, count: usize },
}

pub(super) struct PathTable<'c, 'a, R> {
    pub(super) context: &'c PathContext,
    pub(super) resolver: &'c R,
    pub(super) by_spelling: HashMap<Cow<'a, str>, usize>,
    pub(super) by_path: HashMap<RelativePath<'a>, usize>,
    pub(super) paths: Vec<RelativePath<'a>>,
    pub(super) scratch: Vec<usize>,
}
