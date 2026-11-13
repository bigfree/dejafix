use crate::core::event::{Event, EventId, IssueId, Outcome};
use crate::core::log::EventRefError;
use crate::core::time::Clock;

pub trait Ids {
    fn event_id(&mut self) -> EventId<'static>;
    fn entry_id(&mut self, prefix: &'static str) -> String;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OsIds;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Log {
        text: String,
        location: Option<String>,
    },
    Attempt {
        text: String,
        outcomes: Vec<Outcome>,
        location: Option<String>,
        issue: Option<String>,
        auto_issue: bool,
    },
    Fix {
        text: String,
        location: Option<String>,
        issue: Option<String>,
    },
    Decision {
        text: String,
        location: Option<String>,
        supersedes: Option<String>,
    },
    Note {
        text: String,
        location: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct WriteContext<'c> {
    pub clock: &'c Clock,
    pub git_commit: Option<&'c str>,
    pub marker: Option<&'c str>,
    pub redact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerState {
    Absent,
    Valid(String),
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerChange {
    Keep,
    Set(IssueId<'static>),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    Logged(IssueId<'static>),
    Attempted {
        outcome: Outcome,
        issue: IssueId<'static>,
    },
    Fixed(IssueId<'static>),
    Decided {
        superseded: Option<(EventId<'static>, String)>,
    },
    Noted,
}

#[derive(Debug, Clone)]
pub struct WritePlan {
    pub events: Vec<Event<'static>>,
    pub notices: Vec<String>,
    pub marker: MarkerChange,
    pub outcome: WriteOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("Event summary cannot be empty")]
    EmptySummary,
    #[error("Use only one of --worked, --failed, or --partial.")]
    TwoOutcomes,
    #[error(
        "Issue #{0} was not found. Run `djf search <query>` or `djf brief` to find the issue ID."
    )]
    UnknownIssue(String),
    #[error(
        "No active issue. Run `djf log \"<summary>\"` first, pass `--issue <id>` to attach explicitly, or rerun with `--auto-issue` to auto-create a parent issue from this attempt's text."
    )]
    NoActiveIssue,
    #[error("No open issue found. Run `djf log <text>` first.")]
    NoOpenIssue,
    #[error(transparent)]
    EventRef(#[from] EventRefError),
    #[error(
        "{count} issues are open, so pass issue_id (the id log_issue returned). Open issues, newest first: {shown}{more}."
    )]
    SeveralOpen {
        count: usize,
        shown: String,
        more: String,
    },
}
