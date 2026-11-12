use std::collections::HashSet;
use std::fmt::Write;

use serde_json::Value;

use super::types::{Entry, Relevant, Stack};
use crate::core::json::python_str;

const HEADING: &str = "## Global Memory — Inherited Knowledge\n";
const INTRO: &str = "The following patterns and gotchas were inherited from your global memory\n(~/.dejafix/global/). They represent lessons learned from other projects.\n";

fn pattern_matches(entry: &Entry, project_tags: &HashSet<&str>) -> bool {
    match entry.get("tags") {
        None => true,
        Some(Value::Array(tags)) => {
            tags.is_empty()
                || tags
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|tag| project_tags.contains(tag))
        }
        Some(Value::String(tags)) => {
            tags.is_empty()
                || tags
                    .chars()
                    .any(|tag| project_tags.contains(&*tag.encode_utf8(&mut [0; 4])))
        }
        Some(Value::Object(tags)) => {
            tags.is_empty() || tags.keys().any(|tag| project_tags.contains(tag.as_str()))
        }
        Some(Value::Null | Value::Bool(_) | Value::Number(_)) => false,
    }
}

fn gotcha_matches(entry: &Entry, libraries: &HashSet<&str>, project_tags: &HashSet<&str>) -> bool {
    let library = match entry.get("library") {
        None => String::new(),
        Some(Value::String(library)) => library.to_lowercase(),
        Some(_) => return false,
    };
    libraries.contains(library.as_str()) || project_tags.contains(library.as_str())
}

#[must_use]
pub fn relevant_entries<'e>(
    stack: &Stack,
    filter_tags: Option<&[String]>,
    patterns: &'e [Entry],
    gotchas: &'e [Entry],
) -> Relevant<'e> {
    let project_tags: HashSet<&str> = match filter_tags {
        Some(tags) if !tags.is_empty() => tags.iter().map(String::as_str).collect(),
        _ => stack.tags.iter().map(String::as_str).collect(),
    };
    let libraries: HashSet<&str> = stack.libraries.iter().map(String::as_str).collect();
    Relevant {
        patterns: patterns
            .iter()
            .filter(|entry| pattern_matches(entry, &project_tags))
            .collect(),
        gotchas: gotchas
            .iter()
            .filter(|entry| gotcha_matches(entry, &libraries, &project_tags))
            .collect(),
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

fn field(entry: &Entry, key: &str, default: &str) -> String {
    entry
        .get(key)
        .map_or_else(|| default.to_owned(), python_str)
}

fn suffix(entry: &Entry, key: &str, render: impl Fn(&str) -> String) -> String {
    entry
        .get(key)
        .filter(|value| is_truthy(value))
        .map(|value| render(&python_str(value)))
        .unwrap_or_default()
}

#[must_use]
pub fn inherited_block(relevant: &Relevant<'_>) -> Option<String> {
    if relevant.patterns.is_empty() && relevant.gotchas.is_empty() {
        return None;
    }
    let mut block = format!("{HEADING}\n{INTRO}");
    if !relevant.gotchas.is_empty() {
        block.push_str("\n### Library Gotchas\n");
        for gotcha in &relevant.gotchas {
            let _ = write!(
                block,
                "\n- **{}**{}: {}{}",
                field(gotcha, "library", "unknown"),
                suffix(gotcha, "version_range", |version| format!(" [{version}]")),
                field(gotcha, "gotcha", ""),
                suffix(gotcha, "source_project", |source| format!(
                    " (from {source})"
                )),
            );
        }
        block.push('\n');
    }
    if !relevant.patterns.is_empty() {
        block.push_str("\n### Cross-Project Patterns\n");
        for pattern in &relevant.patterns {
            let _ = write!(
                block,
                "\n- {}{}",
                field(pattern, "pattern", ""),
                suffix(pattern, "source_project", |source| format!(
                    " (from {source})"
                )),
            );
        }
        block.push('\n');
    }
    Some(block)
}
