use std::process::Command;

use super::head_short;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn head_short_reads_the_commit_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(head_short(dir.path()), None);
    git(dir.path(), &["init", "-q"]);
    assert_eq!(head_short(dir.path()), None);
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "x"]);
    let commit = head_short(dir.path()).unwrap();
    assert!(commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit()));
}
