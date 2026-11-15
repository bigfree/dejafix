use std::collections::HashSet;

use serde_json::Value;

use super::types::{Entry, Relevant, Stack};

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

pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}
