use crate::core::event::EventKind;
use crate::core::hooks::HookKind;
use crate::core::precheck::{SnoozeError, Warning};
use crate::core::stale::FileHistory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HooksAction {
    Install { djf: String },
    Uninstall,
    Unknown(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HooksReport {
    pub installed: Vec<HookKind>,
    pub refreshed: Vec<HookKind>,
    pub skipped: Vec<HookKind>,
    pub unreadable: Vec<HookKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HooksOutcome {
    NoHooksDir,
    Installed(HooksReport),
    Removed(Vec<HookKind>),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    Skipped,
    Captured {
        kind: EventKind,
        summary: String,
        notices: Vec<String>,
    },
    Merged {
        first_line: String,
        notices: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrecheckRequest {
    pub level: String,
    pub working: bool,
    pub files: Vec<String>,
    pub quiet: bool,
    pub snooze: Option<String>,
    pub unsnooze: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrecheckOutcome {
    NoProject,
    Unsnoozed(bool),
    Snoozed {
        expiry: jiff::Timestamp,
        notices: Vec<String>,
    },
    BadSnooze(SnoozeError),
    Snoozing(String),
    NoFiles,
    Unreadable,
    Clean,
    Warnings {
        warnings: Vec<Warning>,
        blocking: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditHookRun {
    pub output: String,
    pub event: String,
    pub tool: String,
    pub session: String,
    pub agent: String,
    pub cwd: String,
    pub files: usize,
    pub error: String,
}

pub(crate) trait GitAnswers: FileHistory {
    fn churn(&self, relative: &str) -> Option<usize>;
}
