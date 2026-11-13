use std::fs;
use std::path::{Path, PathBuf};

use crate::core::migrate::has_legacy_path;
use crate::core::text::universal_newlines;

const TEXT_SUFFIXES: [&str; 5] = ["md", "jsonl", "json", "toml", "bak"];

#[must_use]
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

#[must_use]
pub fn read_universal(path: &Path) -> Option<String> {
    let text = String::from_utf8(fs::read(path).ok()?).ok()?;
    Some(universal_newlines(&text).into_owned())
}

fn is_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|suffix| suffix.to_str())
        .is_some_and(|suffix| TEXT_SUFFIXES.contains(&suffix))
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect(&path, found);
        } else if kind.is_file() && is_text_file(&path) {
            found.push(path);
        }
    }
}

#[must_use]
pub fn files_with_legacy_paths(memory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(memory, &mut found);
    found.sort();
    found
        .into_iter()
        .filter(|path| read_universal(path).is_some_and(|text| has_legacy_path(&text)))
        .filter_map(|path| path.strip_prefix(memory).ok().map(Path::to_path_buf))
        .collect()
}

#[must_use]
pub fn holds_data(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_dir() {
        return true;
    }
    fs::read_dir(path).map_or(true, |entries| {
        entries
            .map(|entry| entry.map(|entry| entry.path()))
            .any(|entry| entry.map_or(true, |child| holds_data(&child)))
    })
}
