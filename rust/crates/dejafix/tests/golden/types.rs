use std::collections::{BTreeMap, HashMap};

use regex::Regex;

pub type Step<'a> = &'a [&'a str];

pub type Arguments<'a> =
    Box<dyn Fn(&std::path::Path) -> Result<Vec<String>, Box<dyn std::error::Error>> + 'a>;

pub type Payload<'a> =
    Box<dyn Fn(&std::path::Path) -> Result<String, Box<dyn std::error::Error>> + 'a>;

pub enum Action<'a> {
    Djf(Arguments<'a>),
    EditHook(Payload<'a>),
}

#[derive(Clone, Copy)]
pub enum Implementation {
    Python,
    Rust,
}

#[derive(Debug, PartialEq)]
pub struct StepOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub struct Run {
    pub steps: Vec<StepOutput>,
    pub files: BTreeMap<String, String>,
}

pub struct Normalizer {
    pub(crate) pattern: Regex,
    pub(crate) labels: HashMap<String, String>,
    pub(crate) since: String,
    pub(crate) today: String,
    pub(crate) ids: HashMap<String, usize>,
    pub(crate) versions: Option<Regex>,
}

pub struct Random(pub(crate) u64);
