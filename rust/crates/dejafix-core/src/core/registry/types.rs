use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: String,
    pub path: PathBuf,
    pub alias: Option<String>,
    pub default_brain: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    pub active_project: Option<String>,
    pub projects: Vec<ProjectRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    Clean(Registry),
    Dirty(Registry),
    Legacy(Registry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterResult {
    Existing,
    Added,
    Realiased,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("No project '{0}'.")]
    NotFound(String),
    #[error("No project '{identifier}'. Registered: {known}")]
    NotFoundListing { identifier: String, known: String },
    #[error("Alias '{0}' must be lowercase letters, digits and hyphens.")]
    BadAlias(String),
    #[error("Alias '{alias}' is already used by {}.", path.display())]
    AliasTaken { alias: String, path: PathBuf },
    #[error("{} has no .dejafix/ — run `djf init` there first.", root.display())]
    NoMemory { root: PathBuf },
    #[error("Cannot resolve {0}")]
    Unresolvable(String),
}
