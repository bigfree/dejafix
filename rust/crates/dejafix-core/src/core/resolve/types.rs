use std::path::{Path, PathBuf};

pub trait Probe {
    fn has_memory(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn expand_user(&self, text: &str) -> PathBuf;
    fn resolve(&self, path: &Path) -> Option<PathBuf>;
    fn lookup(&self, text: &str) -> Option<PathBuf>;
    fn discover(&self) -> Option<PathBuf>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Pinned,
    Explicit,
    Roots,
    Active,
    Cwd,
    Only,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub root: PathBuf,
    pub source: Source,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("--root {} has no .dejafix/ — run `djf init` there first.", .0.display())]
    PinnedWithoutMemory(PathBuf),
    #[error(
        "This server is pinned to '{name}' ({}) and cannot write to '{explicit}'. Start dejafix without --root to serve several projects, or use that project's own server.",
        pinned.display()
    )]
    PinnedMismatch {
        name: String,
        pinned: PathBuf,
        explicit: String,
    },
    #[error(
        "'{name}' is registered at {}, but that folder has no .dejafix/ any more. Run `djf init` there, or `djf project remove {name}`.",
        path.display()
    )]
    RegisteredWithoutMemory { name: String, path: PathBuf },
    #[error("{} has no .dejafix/ — run `djf init` there first.", .0.display())]
    NoMemory(PathBuf),
    #[error("No project '{explicit}' — not a registered id, alias, or path. Registered: {known}")]
    Unknown { explicit: String, known: String },
    #[error(
        "Your client offered several dejafix projects ({0}). Pass project=\"<name>\" on the call so the write lands in the right one."
    )]
    SeveralRoots(String),
    #[error(
        "No project selected. Pass project=\"<name>\", or run `djf project use <name>` to set an active one. Registered: {0}"
    )]
    NothingSelected(String),
}
