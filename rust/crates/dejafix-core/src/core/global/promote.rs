use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use super::types::{Entry, Stack};
use crate::core::event::{EventKind, Outcome};
use crate::core::text::{contains_word, is_python_space, python_split};

pub const PROMOTABLE_SEED: [&str; 27] = [
    "sqlalchemy",
    "pydantic",
    "fastapi",
    "flask",
    "django",
    "react",
    "vue",
    "next",
    "express",
    "vite",
    "pandas",
    "numpy",
    "torch",
    "tensorflow",
    "celery",
    "redis",
    "docker",
    "nginx",
    "pytest",
    "jest",
    "vitest",
    "alembic",
    "prisma",
    "drizzle",
    "tailwind",
    "webpack",
    "esbuild",
];

const GOTCHA_PREFIXES: [&str; 10] = [
    "gotcha:", "lesson:", "warning:", "caution:", "pitfall:", "avoid:", "don't ", "do not ",
    "never ", "bug:",
];

const STACK_ALIASES: [(&str, &str); 2] = [("nextjs", "next"), ("tailwind", "tailwindcss")];

#[must_use]
pub fn promotable_set(cached: &[String]) -> BTreeSet<String> {
    PROMOTABLE_SEED
        .iter()
        .map(|library| (*library).to_owned())
        .chain(
            cached
                .iter()
                .filter(|library| !library.is_empty())
                .map(|library| library.to_lowercase()),
        )
        .collect()
}

#[must_use]
pub fn promotable_libraries(stack: &Stack) -> Vec<String> {
    stack.libraries.iter().chain(&stack.tags).cloned().collect()
}

#[must_use]
pub fn is_gotcha_signal(summary: &str, kind: EventKind, outcome: Option<Outcome>) -> bool {
    match kind {
        EventKind::Attempt => matches!(outcome, Some(Outcome::Failed | Outcome::Partial)),
        EventKind::Decision | EventKind::Note => {
            let head = summary.trim_start_matches(is_python_space).to_lowercase();
            GOTCHA_PREFIXES
                .iter()
                .any(|prefix| head.starts_with(prefix))
        }
        EventKind::Issue | EventKind::Hypothesis | EventKind::Fix => false,
    }
}

#[must_use]
pub fn similar(left: &str, right: &str) -> bool {
    let left_lower = left.to_lowercase();
    let right_lower = right.to_lowercase();
    let left_words: HashSet<&str> = python_split(&left_lower).collect();
    let right_words: HashSet<&str> = python_split(&right_lower).collect();
    let smaller = left_words.len().min(right_words.len());
    if smaller == 0 {
        return false;
    }
    let overlap = left_words.intersection(&right_words).count();
    overlap * 5 > smaller * 3
}

#[must_use]
pub fn promotion(
    summary: &str,
    kind: EventKind,
    outcome: Option<Outcome>,
    promotable: &BTreeSet<String>,
    project_libraries: &[String],
    existing: &[Entry],
) -> Option<String> {
    if !is_gotcha_signal(summary, kind, outcome) {
        return None;
    }
    let lowered = summary.to_lowercase();
    let mut matched: Vec<&String> = promotable
        .iter()
        .filter(|library| contains_word(&lowered, library))
        .collect();
    if matched.is_empty() {
        return None;
    }
    if !project_libraries.is_empty() {
        let mut declared: HashSet<String> = project_libraries
            .iter()
            .map(|library| library.to_lowercase())
            .collect();
        for (tag, package) in STACK_ALIASES {
            if declared.contains(tag) {
                declared.insert(package.to_owned());
            }
            if declared.contains(package) {
                declared.insert(tag.to_owned());
            }
        }
        matched.retain(|library| declared.contains(library.as_str()));
        if matched.is_empty() {
            return None;
        }
    }
    let longest = matched
        .iter()
        .map(|library| library.chars().count())
        .max()?;
    let library = matched
        .into_iter()
        .find(|library| library.chars().count() == longest)?;
    for entry in existing {
        if entry.get("library").and_then(Value::as_str) != Some(library.as_str()) {
            continue;
        }
        match entry.get("gotcha") {
            None => {}
            Some(Value::String(gotcha)) => {
                if similar(gotcha, summary) {
                    return None;
                }
            }
            Some(_) => return None,
        }
    }
    Some(library.clone())
}

#[must_use]
pub fn gotcha_entry(
    library: &str,
    gotcha: &str,
    id: String,
    discovered: String,
    source_project: &str,
) -> Entry {
    let mut entry = Entry::new();
    entry.insert("id".into(), Value::String(id));
    entry.insert("library".into(), Value::String(library.to_lowercase()));
    entry.insert("gotcha".into(), Value::String(gotcha.to_owned()));
    entry.insert("tags".into(), Value::Array(Vec::new()));
    entry.insert(
        "source_project".into(),
        Value::String(source_project.to_owned()),
    );
    entry.insert("version_range".into(), Value::Null);
    entry.insert("discovered".into(), Value::String(discovered));
    entry
}
