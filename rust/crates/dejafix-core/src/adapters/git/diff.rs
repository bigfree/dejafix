use std::path::Path;

use super::types::GitFailure;
use super::{git, git_status};
use crate::core::capture::{Change, parse_name_status};
use crate::core::text::python_strip;

#[must_use]
pub fn last_message(root: &Path) -> Option<String> {
    let output = git(root, &["log", "-1", "--pretty=%B"])?;
    let text = String::from_utf8_lossy(&output);
    let message = python_strip(&text);
    (!message.is_empty()).then(|| message.to_owned())
}

#[must_use]
pub fn last_changes(root: &Path) -> Vec<Change> {
    let against_parent = [
        "diff-tree",
        "-r",
        "-M",
        "-l1000",
        "-z",
        "--name-status",
        "HEAD^",
        "HEAD",
    ];
    let from_root = [
        "diff-tree",
        "--root",
        "--no-commit-id",
        "-r",
        "-M",
        "-l1000",
        "-z",
        "--name-status",
        "HEAD",
    ];
    let output = match git_status(root, &against_parent) {
        Ok(output) => Some(output),
        Err(GitFailure::Exit) => git(root, &from_root),
        Err(GitFailure::Unavailable) => None,
    };
    output.map_or_else(Vec::new, |bytes| parse_name_status(&bytes))
}

#[must_use]
pub fn staged_files(root: &Path) -> Vec<String> {
    listed(root, &["diff", "--cached", "--name-only"])
}

#[must_use]
pub fn working_files(root: &Path) -> Vec<String> {
    listed(root, &["diff", "--name-only"])
}

fn listed(root: &Path, args: &[&str]) -> Vec<String> {
    let Some(output) = git(root, args) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output);
    python_strip(&text)
        .split('\n')
        .map(python_strip)
        .filter(|file| !file.is_empty())
        .map(str::to_owned)
        .collect()
}
