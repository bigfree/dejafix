use std::borrow::Cow;

use serde_json::value::RawValue;

use super::types::Record;
use crate::core::event::{Object, raw_text};
use crate::core::json::is_python_truthy;
use crate::core::text::python_lines;

const BOM: &str = "\u{feff}";

pub fn records(text: &str) -> Vec<Record<'_>> {
    let body = text.strip_prefix(BOM).unwrap_or(text);
    python_lines(body.as_bytes())
        .filter_map(|(_, line)| std::str::from_utf8(line).ok())
        .filter_map(record)
        .collect()
}

fn record(line: &str) -> Option<Record<'_>> {
    let Object(entries) = serde_json::from_str(line).ok()?;
    let mut found = Record::default();
    for (key, raw) in &entries {
        let value = || text(raw);
        match key.as_ref() {
            "type" => found.kind = value(),
            "id" => found.id = value(),
            "issue_id" => found.issue_id = value(),
            "outcome" => found.outcome = value(),
            "summary" => found.summary = value(),
            "location" => found.location = value(),
            "timestamp" => found.timestamp = value(),
            "capture_source" => found.capture_source = value(),
            "supersedes" => found.supersedes = is_python_truthy(raw).then(value),
            "files" => found.files = strings(raw),
            _ => {}
        }
    }
    Some(found)
}

fn text(raw: &RawValue) -> Cow<'_, str> {
    raw_text(raw).unwrap_or(Cow::Borrowed(""))
}

fn strings(raw: &RawValue) -> Vec<Cow<'_, str>> {
    if !raw.get().starts_with('[') {
        return Vec::new();
    }
    serde_json::from_str::<Vec<&RawValue>>(raw.get())
        .map(|items| items.into_iter().filter_map(raw_text).collect())
        .unwrap_or_default()
}
