use std::path::{Path, PathBuf};

use super::run::git_status_with_pathspec_magic;

#[must_use]
pub fn is_ignored(root: &Path, path: &str) -> bool {
    git_status_with_pathspec_magic(root, &["check-ignore", "--quiet", "--", path]).is_ok()
}

#[must_use]
pub fn exclude_file(root: &Path) -> PathBuf {
    root.join(".git").join("info").join("exclude")
}
