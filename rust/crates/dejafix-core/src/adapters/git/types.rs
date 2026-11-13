use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct GitHistory {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitFailure {
    Exit,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoState {
    pub(super) head: String,
    pub(super) reference: Option<String>,
    pub(super) packed: Option<(u64, Option<SystemTime>)>,
    pub(super) tables: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDirs {
    pub dir: PathBuf,
    pub common: PathBuf,
}
