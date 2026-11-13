use std::path::Path;

use super::run::git;

#[must_use]
pub fn head_short(root: &Path) -> Option<String> {
    let output = git(root, &["rev-parse", "--short", "HEAD"])?;
    let text = String::from_utf8_lossy(&output);
    let commit = text.trim();
    (!commit.is_empty()).then(|| commit.to_owned())
}
