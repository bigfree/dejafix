use serde_json::{Map, Value};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stack {
    pub tags: Vec<String>,
    pub libraries: Vec<String>,
    pub frameworks: Vec<String>,
    pub manifest_files: Vec<&'static str>,
}

pub type Entry = Map<String, Value>;
