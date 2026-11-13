use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use jiff::tz::TimeZone;

use crate::adapters::git::RepoState;
use crate::core::event::Outcome;
use crate::core::log::EventLog;
use crate::core::path::PathContext;

#[derive(Debug, Clone)]
pub struct McpSettings {
    pub pinned: Option<PathBuf>,
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub dejafix_home: PathBuf,
    pub redact: bool,
    pub zone: TimeZone,
}

#[derive(Debug)]
pub struct McpState {
    pub(super) settings: McpSettings,
    pub(super) cache: Mutex<LogCache>,
    pub(super) git: Mutex<GitMemo>,
}

#[derive(Debug, Default)]
pub(super) struct GitMemo {
    pub(super) churn: HashMap<(PathBuf, String), Remembered<usize>>,
    pub(super) times: HashMap<(PathBuf, String, Option<String>), Remembered<Vec<String>>>,
}

#[derive(Debug)]
pub(super) struct Remembered<T> {
    pub(super) state: RepoState,
    pub(super) at: jiff::Timestamp,
    pub(super) answer: T,
}

pub(super) struct MemoGit<'m, G> {
    pub(super) inner: G,
    pub(super) root: &'m std::path::Path,
    pub(super) state: std::sync::OnceLock<Option<RepoState>>,
    pub(super) memo: &'m Mutex<GitMemo>,
    pub(super) now: jiff::Timestamp,
}

#[derive(Debug, Default)]
pub(super) struct LogCache {
    pub(super) entries: HashMap<PathBuf, CacheEntry>,
    pub(super) tick: u64,
}

#[derive(Debug)]
pub(super) struct CacheEntry {
    pub(super) len: u64,
    pub(super) modified: Option<SystemTime>,
    pub(super) used: u64,
    pub(super) log: Arc<CachedLog>,
}

#[derive(Debug)]
pub struct CachedLog {
    pub log: EventLog<'static>,
    pub context: PathContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCall {
    StartSession,
    GetInstructions,
    GetSummary,
    GetProjectMap,
    GetPlan,
    PrecheckFile(String),
    PrecheckFiles(Vec<String>),
    GetIssue(String),
    SearchEvents {
        query: String,
        limit: usize,
        full: bool,
    },
    GetScore,
    GetContext {
        tokens: u32,
        focus: Option<String>,
    },
    GetGlobalGotchas(Option<String>),
    Write(WriteCall),
    ListProjects,
    CurrentProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteCall {
    LogIssue {
        summary: String,
        location: Option<String>,
    },
    RecordAttempt {
        summary: String,
        outcome: Outcome,
        location: Option<String>,
        issue_id: Option<String>,
    },
    RecordFix {
        summary: String,
        location: Option<String>,
        issue_id: Option<String>,
    },
    AddDecision {
        summary: String,
        location: Option<String>,
        supersedes: Option<String>,
    },
    AddNote {
        summary: String,
        location: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReply {
    pub text: String,
    pub error: bool,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ToolError {
    #[error("ResolutionError: {0}")]
    Resolution(String),
    #[error("DejafixError: {0}")]
    Dejafix(String),
    #[error("{class}: {message}")]
    Io {
        class: &'static str,
        message: String,
    },
    #[error("ValueError: {0}")]
    Value(String),
    #[error("JSONDecodeError: {0}")]
    Json(String),
    #[error("RegistryError: {0}")]
    Registry(String),
}

pub(super) struct FsProbe<'s> {
    pub(super) cwd: &'s std::path::Path,
    pub(super) home: Option<&'s std::path::Path>,
}
