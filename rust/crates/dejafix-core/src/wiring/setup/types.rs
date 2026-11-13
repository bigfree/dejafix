use std::path::PathBuf;

use crate::adapters::fs::{ClientPaths, StoreError};
use crate::core::migrate::MigrateStep;
use crate::wiring::hooks::HooksReport;
use crate::wiring::types::BridgeOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateOptions {
    pub apply: bool,
    pub old_home: PathBuf,
    pub cwd: PathBuf,
    pub extra_roots: Vec<String>,
    pub clients: ClientPaths,
    pub legacy_env: Vec<String>,
    pub djf: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectState {
    Symlink,
    BothExist,
    NoLegacy,
    Watcher(u32),
    Migrated,
    Steps(Vec<MigrateStep>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrateEvent {
    Banner {
        apply: bool,
    },
    HomeMissing(PathBuf),
    Home {
        from: PathBuf,
        to: PathBuf,
    },
    HomeSkip {
        name: String,
        dest: PathBuf,
    },
    HomeMove(String),
    Project {
        label: String,
        state: ProjectState,
    },
    Bridge(&'static str, BridgeOutcome),
    Hooks(HooksReport),
    Leftovers {
        configs: Vec<(&'static str, PathBuf)>,
        env: Vec<String>,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Offline,
    Unreachable,
    Newer(String),
    Latest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    pub rename: bool,
    pub stale: Vec<PathBuf>,
    pub instructions: bool,
    pub gitignore: bool,
    pub bridges: Vec<&'static str>,
    pub hooks: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LegacyProject {
    pub name: String,
    pub root: PathBuf,
}

#[derive(Debug)]
pub(super) enum Halt {
    Store(StoreError),
    Stopped,
}

impl From<StoreError> for Halt {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
