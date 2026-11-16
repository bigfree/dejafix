use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkSpec {
    pub project: String,
    pub relation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectLinks {
    pub boundary: Vec<String>,
    pub links: Vec<LinkSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("config.toml is not valid TOML: {0}")]
    Invalid(String),
    #[error("config.toml: '{0}' must be a list")]
    WrongType(&'static str),
    #[error("config.toml is larger than {0} bytes, so it was not read")]
    TooLarge(usize),
    #[error("config.toml could not be read: {0}")]
    Unreadable(String),
}

#[derive(Debug, Clone)]
pub struct Glob {
    pub(super) regex: regex::Regex,
    pub(super) prefix: String,
    pub(super) ignore_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub id: String,
    pub alias: Option<String>,
    pub folder: String,
    pub identity: String,
    pub path: PathBuf,
    pub worktree: bool,
    pub config: ProjectLinks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Declared,
    Reverse,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub id: String,
    pub identity: String,
    pub path: PathBuf,
    pub direction: Direction,
    pub relation: String,
    pub boundary: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkResolution {
    pub links: Vec<Link>,
    pub unresolved: Vec<String>,
    pub duplicates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkedBlock {
    pub id: String,
    pub relation: String,
    pub lines: Vec<String>,
    pub hidden: usize,
    pub query: String,
    pub score: f64,
}

pub(super) struct LessonDoc<'l> {
    pub(super) id: String,
    pub(super) summary: &'l str,
    pub(super) files: Vec<String>,
}
