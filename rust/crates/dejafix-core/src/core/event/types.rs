use std::borrow::Cow;

use serde_json::value::RawValue;

use jiff::tz::TimeZone;

use crate::core::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Issue,
    Hypothesis,
    Attempt,
    Fix,
    Decision,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Worked,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureSource {
    GitPostCommit,
    GitPostRevert,
    GitPostMerge,
    ChurnDetector,
    CiParser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId<'a>(pub(super) Cow<'a, str>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IssueId<'a>(pub(super) Cow<'a, str>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Summary<'a>(pub(super) Cow<'a, str>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin<'a> {
    pub auto_captured: bool,
    pub source: Option<CaptureSource>,
    pub confidence: Option<Confidence>,
    pub git_commit: Option<Cow<'a, str>>,
    pub git_message: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Default)]
pub struct Extra<'a>(pub(super) Vec<(Cow<'a, str>, Cow<'a, RawValue>)>);

#[derive(Debug, Clone)]
pub struct Event<'a> {
    pub id: EventId<'a>,
    pub at: Timestamp<'a>,
    pub kind: EventKind,
    pub summary: Summary<'a>,
    pub issue: Option<IssueId<'a>>,
    pub outcome: Option<Outcome>,
    pub files: Vec<Cow<'a, str>>,
    pub location: Option<Cow<'a, str>>,
    pub command: Option<Cow<'a, str>>,
    pub notes: Option<Cow<'a, str>>,
    pub supersedes: Option<EventId<'a>>,
    pub origin: Origin<'a>,
    pub extra: Extra<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LineError {
    #[error("line is not valid UTF-8")]
    Utf8,
    #[error("line is not valid JSON")]
    Json,
    #[error("line is not a JSON object")]
    NotAnObject,
    #[error("missing or unknown event type")]
    Kind,
    #[error("summary is missing, not a string or empty")]
    Summary,
    #[error("unknown outcome")]
    Outcome,
    #[error("unknown capture source")]
    CaptureSource,
    #[error("unknown capture confidence")]
    Confidence,
    #[error("timestamp is not a string or leaves the years 1-9999")]
    Timestamp,
    #[error("field {0} has a shape Rust does not read")]
    Unsupported(&'static str),
}

pub(crate) struct LineContext<'c> {
    pub(super) zone: &'c TimeZone,
    pub(super) now: Timestamp<'static>,
}

pub(crate) struct Object<'a>(pub(crate) Vec<(Cow<'a, str>, &'a RawValue)>);

pub(super) struct CowStr<'a>(pub(super) Cow<'a, str>);

pub(super) struct ObjectVisitor;

pub(super) struct CowStrVisitor;

pub(super) enum Value<'v, 'a> {
    Str(&'v str),
    Strs(&'v [Cow<'a, str>]),
    Bool,
    Raw(&'a RawValue),
}
