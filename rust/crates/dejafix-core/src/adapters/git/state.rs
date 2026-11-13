use std::fs;
use std::path::{Path, PathBuf};

use super::run::git;
use super::types::{GitDirs, RepoState};

const REF_PREFIX: &str = "ref:";

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

#[must_use]
pub fn git_dirs(root: &Path) -> Option<GitDirs> {
    let output = git(
        root,
        &["rev-parse", "--absolute-git-dir", "--git-common-dir"],
    )?;
    let text = String::from_utf8(output).ok()?;
    let mut lines = text.lines().map(str::trim);
    let dir = PathBuf::from(lines.next()?);
    let common = root.join(lines.next()?);
    Some(GitDirs { dir, common })
}

#[must_use]
pub fn repository_state(dirs: &GitDirs) -> Option<RepoState> {
    let GitDirs { dir, common } = dirs;
    let head = read_trimmed(&dir.join("HEAD"))?;
    let reference = head
        .strip_prefix(REF_PREFIX)
        .map(str::trim)
        .and_then(|name| {
            read_trimmed(&common.join(name)).or_else(|| read_trimmed(&dir.join(name)))
        });
    let packed = fs::metadata(common.join("packed-refs"))
        .ok()
        .map(|metadata| (metadata.len(), metadata.modified().ok()));
    let tables = read_trimmed(&common.join("reftable").join("tables.list"));
    Some(RepoState {
        head,
        reference,
        packed,
        tables,
    })
}
