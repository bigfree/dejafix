use std::fmt::Write as _;

use serde_json::Value;

use crate::core::global::{Entry, is_truthy};
use crate::core::json::python_str;

const SHOWN: usize = 15;

fn text_field<'e>(entry: &'e Entry, key: &str) -> &'e str {
    entry.get(key).and_then(Value::as_str).unwrap_or("")
}

fn shown_field(entry: &Entry, key: &str, missing: &str) -> String {
    entry
        .get(key)
        .map_or_else(|| missing.to_owned(), python_str)
}

#[must_use]
pub fn gotchas_reply(gotchas: &[Entry], library: Option<&str>) -> String {
    let library = library.filter(|library| !library.is_empty());
    let wanted = library.map(str::to_lowercase);
    let found: Vec<&Entry> = gotchas
        .iter()
        .filter(|entry| {
            wanted
                .as_deref()
                .is_none_or(|wanted| text_field(entry, "library").to_lowercase().contains(wanted))
        })
        .collect();
    if found.is_empty() {
        return match library {
            Some(library) => format!("No global gotchas found for {library}."),
            None => "No global gotchas found.".to_owned(),
        };
    }
    let mut reply = format!("Global gotchas ({}):", found.len());
    for entry in found.iter().take(SHOWN) {
        let _ = write!(
            reply,
            "\n  [{}] {}",
            shown_field(entry, "library", "unknown"),
            shown_field(entry, "gotcha", "")
        );
        if entry.get("source_project").is_some_and(is_truthy) {
            let _ = write!(
                reply,
                " (from {})",
                shown_field(entry, "source_project", "")
            );
        }
    }
    reply
}
