#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    FailedAttempts,
    OpenIssues,
    HighChurn,
    RecentRevert,
    RelevantDecision,
    PossiblyStale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub file: String,
    pub severity: Severity,
    pub kind: WarningKind,
    pub title: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Targets {
    pub files: Vec<String>,
    pub events: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnoozeError {
    #[error("Invalid duration '{0}' — use forms like 30m, 2h, or 1d.")]
    Invalid(String),
    #[error("Snooze duration must be positive.")]
    NotPositive,
}

pub(super) struct Styles {
    pub(super) bold: &'static str,
    pub(super) dim: &'static str,
    pub(super) yellow: &'static str,
    pub(super) red: &'static str,
    pub(super) cyan: &'static str,
    pub(super) reset: &'static str,
}

pub(super) type Found = Option<(Severity, WarningKind, String, Vec<String>)>;
