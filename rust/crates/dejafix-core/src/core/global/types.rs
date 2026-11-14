use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::core::rank::Index;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stack {
    pub tags: Vec<String>,
    pub libraries: Vec<String>,
    pub frameworks: Vec<String>,
    pub manifest_files: Vec<&'static str>,
}

pub type Entry = Map<String, Value>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Relevant<'e> {
    pub patterns: Vec<&'e Entry>,
    pub gotchas: Vec<&'e Entry>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Palette {
    pub(super) bold: &'static str,
    pub(super) dim: &'static str,
    pub(super) cyan: &'static str,
    pub(super) yellow: &'static str,
    pub(super) green: &'static str,
    pub(super) reset: &'static str,
}

impl Palette {
    pub(super) const COLOR: Self = Self {
        bold: "\x1b[1m",
        dim: "\x1b[2m",
        cyan: "\x1b[36m",
        yellow: "\x1b[33m",
        green: "\x1b[32m",
        reset: "\x1b[0m",
    };

    pub(super) const PLAIN: Self = Self {
        bold: "",
        dim: "",
        cyan: "",
        yellow: "",
        green: "",
        reset: "",
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalAction {
    Status,
    List {
        tag: Option<String>,
        library: Option<String>,
        json: bool,
    },
    Add {
        text: Option<String>,
        tag: Option<String>,
    },
    AddGotcha {
        library: Option<String>,
        text: Option<String>,
        keywords: Vec<String>,
        versions: Option<String>,
    },
    Query {
        text: Option<String>,
        library: Option<String>,
        budget: u32,
    },
    Remove(Option<String>),
    Export,
    Import(Option<String>),
    Prune {
        older_than: i64,
        confidence: Option<String>,
    },
    Detect {
        json: bool,
    },
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Add,
    AddGotcha,
    Remove,
    Import,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalOutcome {
    Status {
        location: String,
        patterns: Vec<Entry>,
        gotchas: Vec<Entry>,
        preferences: bool,
    },
    List {
        patterns: Vec<Entry>,
        gotchas: Vec<Entry>,
        tag: Option<String>,
        library: Option<String>,
        json: bool,
    },
    Usage(Usage),
    PatternAdded {
        id: String,
        text: String,
    },
    GotchaAdded {
        id: String,
        library: String,
        text: String,
        warnings: Vec<String>,
    },
    GotchaRejected {
        failures: Vec<String>,
        warnings: Vec<String>,
    },
    Reply(String),
    Removed(String),
    NotFound(String),
    Exported(Value),
    Imported {
        patterns: usize,
        gotchas: usize,
    },
    FileNotFound(String),
    InvalidJson(String),
    Pruned {
        removed: usize,
        older_than: i64,
    },
    Detected {
        name: String,
        stack: Stack,
        inherited_patterns: usize,
        inherited_gotchas: usize,
        json: bool,
    },
    Unknown(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GotchaV2 {
    pub id: String,
    pub library: String,
    pub keywords: Vec<String>,
    pub versions: Option<String>,
    pub gotcha: String,
    pub source_project: Option<String>,
    pub discovered: Option<String>,
    pub reviewed: Option<String>,
    pub extra: Entry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StoredGotcha {
    V2(GotchaV2),
    V1(Entry),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Libraries(pub BTreeMap<String, Vec<String>>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GotchaInput {
    pub library: String,
    pub keywords: Vec<String>,
    pub gotcha: String,
    pub versions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckFailure {
    #[error("the text is not English ({0})")]
    NotEnglish(String),
    #[error("the library must be a package name in lower case: letters, digits and @ / . _ -")]
    Library,
    #[error("give 2 to 8 keywords, not {0}")]
    KeywordCount(usize),
    #[error("a keyword is empty")]
    EmptyKeyword,
    #[error("the keyword '{0}' has more than 4 words")]
    LongKeyword(String),
    #[error("the gotcha must be 30 to 600 characters long, not {0}")]
    Length(usize),
    #[error("it names a project path ({0})")]
    ProjectPath(String),
    #[error("it names a ticket number ({0})")]
    Ticket(String),
    #[error("it names an internal host ({0})")]
    InternalHost(String),
    #[error("it repeats {0}; change that record with `djf global edit` instead")]
    Duplicate(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckWarning {
    #[error("it does not look like English")]
    Doubtful,
    #[error("it names the project '{0}'")]
    ProjectName(String),
    #[error("it reads like a run status ('{0}')")]
    RunStatus(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckReport {
    pub failures: Vec<CheckFailure>,
    pub warnings: Vec<CheckWarning>,
    pub duplicate_of: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct GlobalDoc<'r> {
    pub record: &'r StoredGotcha,
    pub aliases: &'r [String],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    Added {
        id: String,
        library: String,
        warnings: Vec<CheckWarning>,
    },
    Rejected(CheckReport),
}

#[derive(Debug, Clone, Default)]
pub struct GlobalIndex {
    pub(super) index: Index,
    pub(super) records: Vec<StoredGotcha>,
    pub(super) canonical: Vec<Box<str>>,
    pub(super) libraries: Libraries,
}
