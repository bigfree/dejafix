use serde_json::{Map, Value};

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
        tag: Option<String>,
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
    },
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
