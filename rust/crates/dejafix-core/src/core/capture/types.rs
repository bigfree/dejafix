use regex::Regex;

use crate::core::event::{Confidence, EventKind, Outcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classified {
    pub kind: EventKind,
    pub outcome: Option<Outcome>,
    pub prefix: &'static str,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub status: char,
    pub path: String,
}

pub(super) struct Rule {
    pub(super) regex: Regex,
    pub(super) classified: Classified,
}
