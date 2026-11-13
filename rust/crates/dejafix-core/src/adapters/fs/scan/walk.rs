use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, FileType, Metadata};
use std::path::{Path, PathBuf};

use super::types::Identity;
use crate::adapters::fs::canonical_path;

const MEMORY_DIR: &str = ".dejafix";

const SKIP: [&str; 17] = [
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    "site-packages",
    "Library",
    "AppData",
    ".cache",
    ".Trash",
    "dist",
    "build",
    ".next",
    "target",
    "vendor",
    "Pods",
];

const HOME_DIRS: [&str; 14] = [
    "Developer",
    "Documents",
    "Desktop",
    "code",
    "Code",
    "src",
    "source",
    "projects",
    "Projects",
    "repos",
    "repo",
    "work",
    "git",
    "dev",
];

const CLOUD_PATTERNS: [&str; 17] = [
    "OneDrive*",
    "Dropbox*",
    "Google Drive*",
    "GoogleDrive*",
    "gdrive",
    "Library/CloudStorage/*",
    "Library/Mobile Documents/com~apple~CloudDocs",
    "iCloudDrive*",
    "Nextcloud*",
    "ownCloud*",
    "Box",
    "Box Sync",
    "MEGA*",
    "pCloud*",
    "Sync",
    "Seafile*",
    "Yandex.Disk*",
];

const DRIVE_LETTERS: std::ops::RangeInclusive<char> = 'D'..='Z';

fn is_dir_following_links(entry: &DirEntry, kind: FileType) -> bool {
    if kind.is_symlink() {
        fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir())
    } else {
        kind.is_dir()
    }
}

fn is_walked(name: &OsStr) -> bool {
    name == MEMORY_DIR
        || (!name.as_encoded_bytes().starts_with(b".") && !SKIP.iter().any(|skip| name == *skip))
}

fn walk(here: &Path, depth: usize, max_depth: usize, found: &mut Vec<PathBuf>) {
    if depth >= max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(here) else {
        return;
    };
    let mut children = Vec::new();
    let mut is_project = false;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !is_walked(&name) || !is_dir_following_links(&entry, kind) {
            continue;
        }
        if name == MEMORY_DIR {
            is_project = true;
        } else if !kind.is_symlink() {
            children.push(entry.path());
        }
    }
    if is_project {
        found.push(here.to_path_buf());
        return;
    }
    for child in children {
        walk(&child, depth + 1, max_depth, found);
    }
}

#[must_use]
pub fn find_projects(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let root = canonical_path(root).unwrap_or_else(|| root.to_path_buf());
    let mut found = Vec::new();
    walk(&root, 0, max_depth, &mut found);
    found
}

#[cfg(unix)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the Windows variant canonicalises and can fail; both share one signature"
)]
fn identity_of(_path: &Path, meta: &Metadata) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    Some(Identity {
        device: meta.dev(),
        inode: meta.ino(),
    })
}

#[cfg(not(unix))]
fn identity_of(path: &Path, _meta: &Metadata) -> Option<Identity> {
    fs::canonicalize(path).ok().map(Identity)
}

#[must_use]
pub fn identity(path: &Path) -> Option<Identity> {
    identity_of(path, &fs::metadata(path).ok()?)
}

fn directory_identity(path: &Path) -> Option<Identity> {
    let meta = fs::metadata(path).ok()?;
    if meta.is_dir() {
        identity_of(path, &meta)
    } else {
        None
    }
}

pub fn dedupe_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| directory_identity(path).is_some_and(|key| seen.insert(key)))
        .collect()
}

fn name_matches(name: &OsStr, prefix: &str) -> bool {
    if cfg!(windows) {
        name.to_string_lossy()
            .to_lowercase()
            .starts_with(&prefix.to_lowercase())
    } else {
        name.as_encoded_bytes().starts_with(prefix.as_bytes())
    }
}

fn glob(home: &Path, pattern: &str) -> Vec<PathBuf> {
    let (directory, last) = match pattern.rsplit_once('/') {
        Some((parent, last)) => (home.join(parent), last),
        None => (home.to_path_buf(), pattern),
    };
    let Some(prefix) = last.strip_suffix('*') else {
        let path = directory.join(last);
        return if fs::symlink_metadata(&path).is_ok() {
            vec![path]
        } else {
            Vec::new()
        };
    };
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut matches: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| name_matches(&entry.file_name(), prefix))
        .map(|entry| entry.path())
        .collect();
    matches.sort();
    matches
}

fn fixed_drives() -> impl Iterator<Item = PathBuf> {
    DRIVE_LETTERS
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|drive| cfg!(windows) && drive.try_exists().unwrap_or(false))
}

#[must_use]
pub fn default_roots(home: &Path) -> Vec<PathBuf> {
    let homes = HOME_DIRS.iter().map(|name| home.join(name));
    let clouds = CLOUD_PATTERNS
        .iter()
        .flat_map(|pattern| glob(home, pattern));
    dedupe_paths(homes.chain(clouds).chain(fixed_drives()))
}
