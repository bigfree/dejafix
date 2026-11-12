use std::fs;
use std::path::Path;

use super::store::read_text;
use super::write::write_atomic;
use crate::core::text::python_strip;

const MARKER_FILE: &str = ".current_issue";

#[must_use]
pub fn read_marker(memory_dir: &Path) -> Option<String> {
    let text = read_text(&memory_dir.join(MARKER_FILE)).ok()?;
    let marker = python_strip(&text);
    (!marker.is_empty()).then(|| marker.to_owned())
}

pub fn write_marker(memory_dir: &Path, issue: &str) {
    let _ = write_atomic(&memory_dir.join(MARKER_FILE), issue.as_bytes());
}

pub fn clear_marker(memory_dir: &Path) {
    let _ = fs::remove_file(memory_dir.join(MARKER_FILE));
}
