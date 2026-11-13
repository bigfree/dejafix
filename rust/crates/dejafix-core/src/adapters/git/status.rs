use std::path::Path;

use super::git;
use crate::core::text::{python_strip, universal_newlines};

const PREFIX_CHARS: usize = 3;

#[must_use]
pub fn status_files(root: &Path) -> Vec<String> {
    let Some(output) = git(root, &["status", "--porcelain", "--no-renames"]) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output);
    let text = universal_newlines(&text);
    python_strip(&text)
        .split('\n')
        .filter(|line| !python_strip(line).is_empty())
        .map(|line| python_strip(after_status(line)).to_owned())
        .collect()
}

fn after_status(line: &str) -> &str {
    line.char_indices()
        .nth(PREFIX_CHARS)
        .and_then(|(index, _)| line.get(index..))
        .unwrap_or("")
}
