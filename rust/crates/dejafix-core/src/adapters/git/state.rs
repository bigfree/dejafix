use std::fs;
use std::path::{Path, PathBuf};

use super::types::RepoState;

const GIT: &str = ".git";
const GITDIR_PREFIX: &str = "gitdir:";
const REF_PREFIX: &str = "ref:";

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn git_dir(root: &Path) -> Option<PathBuf> {
    root.ancestors().find_map(|folder| {
        let candidate = folder.join(GIT);
        let metadata = fs::metadata(&candidate).ok()?;
        if metadata.is_dir() {
            return Some(candidate);
        }
        let pointer = read_trimmed(&candidate)?;
        let target = pointer.strip_prefix(GITDIR_PREFIX)?.trim();
        Some(folder.join(target))
    })
}

#[must_use]
pub fn repository_state(root: &Path) -> Option<RepoState> {
    let dir = git_dir(root)?;
    let common =
        read_trimmed(&dir.join("commondir")).map_or_else(|| dir.clone(), |common| dir.join(common));
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
    Some(RepoState {
        head,
        reference,
        packed,
    })
}
