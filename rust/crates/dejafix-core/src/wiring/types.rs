use std::path::PathBuf;

use super::hooks::HooksReport;
use crate::adapters::fs::{ClientConfig, MemoryDir, StoreError};
use crate::core::doctor::Reverted;
use crate::core::hooks::HookKind;
use crate::core::log::Skipped;
use crate::core::path::PathContext;
use crate::core::registry::{ProjectRecord, RegistryError};
use crate::core::time::Clock;
use crate::core::write::{ValidationError, WriteOutcome};

#[derive(Debug, Clone)]
pub struct Settings {
    pub dejafix_home: PathBuf,
    pub redact: bool,
    pub clock: Clock,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub memory: MemoryDir,
    pub settings: Settings,
    pub(super) context: PathContext,
}

#[derive(Debug)]
pub struct Recorded {
    pub events: Vec<String>,
    pub outcome: WriteOutcome,
    pub notices: Vec<String>,
    pub hints: Vec<String>,
    pub incomplete: Option<WiringError>,
    pub unreadable: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Searched {
    pub output: SearchOutput,
    pub unreadable: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueScope {
    All,
    Touched(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regenerated {
    pub path: PathBuf,
    pub removed: usize,
    pub not_removed: Vec<String>,
    pub unreadable: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOutput {
    Matches(Vec<String>),
    Nothing(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum WiringError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitOptions {
    pub claude_md: bool,
    pub stack_detect: bool,
    pub global: bool,
    pub global_tags: Option<String>,
    pub hooks: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitHooks {
    Installed(HooksReport),
    Configured {
        path: PathBuf,
        snippets: Vec<(HookKind, String)>,
    },
    NoGit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeOutcome {
    Created,
    Appended,
    Refreshed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub mem_dir: PathBuf,
    pub refreshed_instructions: bool,
    pub bridges: Vec<(&'static str, BridgeOutcome)>,
    pub project_map: Option<String>,
    pub hooks: Option<InitHooks>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFile {
    Instructions,
    ProjectMap,
    Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryOp {
    List,
    Use(Option<String>),
    Remove(String),
    Alias {
        identifier: String,
        alias: String,
    },
    Tag {
        identifier: String,
        tag: String,
        remove: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedProject {
    pub record: ProjectRecord,
    pub has_memory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryReply {
    Listed {
        active: Option<String>,
        projects: Vec<ListedProject>,
    },
    Activated(Option<ProjectRecord>),
    Removed(ProjectRecord),
    Aliased(ProjectRecord),
    Tagged(ProjectRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeNotice {
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOptions {
    pub fix: bool,
    pub depth: usize,
    pub roots: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorruptLog {
    pub path: PathBuf,
    pub lines: Vec<Skipped>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DoctorReport {
    pub roots: usize,
    pub found: usize,
    pub missing: Vec<PathBuf>,
    pub register_failures: Vec<(PathBuf, String)>,
    pub stale: Vec<ProjectRecord>,
    pub prune_failure: Option<String>,
    pub unreadable: Vec<ClientConfig>,
    pub pinned: Vec<ClientConfig>,
    pub reverted: Vec<Reverted>,
    pub corrupt: Vec<CorruptLog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundProject {
    pub path: PathBuf,
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanReport {
    NotADirectory(PathBuf),
    Scanned {
        roots: Vec<PathBuf>,
        found: Vec<FoundProject>,
        failures: Vec<(PathBuf, String)>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct GotchaChanges {
    pub(super) library: Option<String>,
    pub(super) keywords: Vec<String>,
    pub(super) versions: Option<String>,
}
