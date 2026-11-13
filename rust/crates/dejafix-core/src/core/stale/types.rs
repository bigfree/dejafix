use crate::core::event::Event;

pub trait FileHistory: Sync {
    fn exists(&self, relative: &str) -> bool;
    fn commit_times(&self, relative: &str, since: Option<&str>) -> Option<Vec<String>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitsSince {
    Missing,
    Count(usize),
}

#[derive(Debug, Clone)]
pub struct Stale<'l, 'a> {
    pub event: &'l Event<'a>,
    pub file: String,
    pub commits_since: CommitsSince,
}

pub(super) enum History {
    Missing,
    Unknown,
    Times(Vec<String>),
}
