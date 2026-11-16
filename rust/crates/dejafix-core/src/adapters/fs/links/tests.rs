use std::fs;

use super::*;
use crate::core::links::{CONFIG_LIMIT, ConfigError, ProjectLinks};

fn memory(root: &std::path::Path, config: &str) {
    fs::create_dir_all(root.join(".dejafix")).unwrap();
    fs::write(root.join(".dejafix/config.toml"), config).unwrap();
}

#[test]
fn a_missing_config_has_no_links() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_project_links(dir.path()), Ok(ProjectLinks::default()));
}

#[test]
fn the_config_is_parsed_and_capped() {
    let dir = tempfile::tempdir().unwrap();
    memory(dir.path(), "boundary = [\"src/**\"]\n");
    assert_eq!(read_project_links(dir.path()).unwrap().boundary, ["src/**"]);
    memory(
        dir.path(),
        &format!("x = \"{}\"\n", "y".repeat(CONFIG_LIMIT)),
    );
    assert_eq!(
        read_project_links(dir.path()),
        Err(ConfigError::TooLarge(CONFIG_LIMIT))
    );
}

#[test]
fn an_oversized_config_cut_inside_a_character_is_too_large() {
    let dir = tempfile::tempdir().unwrap();
    memory(
        dir.path(),
        &format!("x = \"{}\u{e9}\"\n", "a".repeat(CONFIG_LIMIT - 5)),
    );
    assert_eq!(
        read_project_links(dir.path()),
        Err(ConfigError::TooLarge(CONFIG_LIMIT))
    );
}

#[test]
fn a_config_that_is_not_a_file_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".dejafix/config.toml")).unwrap();
    assert!(matches!(
        read_project_links(dir.path()),
        Err(ConfigError::Unreadable(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_config_linked_from_outside_the_project_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside.toml");
    fs::write(&outside, "boundary = [\"x/**\"]\n").unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join(".dejafix")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".dejafix/config.toml")).unwrap();
    assert_eq!(read_project_links(&root), Ok(ProjectLinks::default()));
}

#[test]
fn origin_comes_from_the_git_folder_a_gitdir_file_or_the_common_dir() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    fs::create_dir_all(main.join(".git/worktrees/wt")).unwrap();
    fs::write(
        main.join(".git/config"),
        "[remote \"origin\"]\n\turl = git@h.example:o/main.git\n",
    )
    .unwrap();
    assert_eq!(
        read_origin(&main).as_deref(),
        Some("git@h.example:o/main.git")
    );
    let worktree = dir.path().join("wt");
    fs::create_dir_all(&worktree).unwrap();
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", main.join(".git/worktrees/wt").display()),
    )
    .unwrap();
    fs::write(main.join(".git/worktrees/wt/commondir"), "../..\n").unwrap();
    assert_eq!(
        read_origin(&worktree).as_deref(),
        Some("git@h.example:o/main.git")
    );
    let plain = dir.path().join("plain");
    fs::create_dir_all(plain.join("gitdir")).unwrap();
    fs::write(plain.join(".git"), "gitdir: gitdir\n").unwrap();
    fs::write(
        plain.join("gitdir/config"),
        "[remote \"origin\"]\nurl = https://h/o/plain\n",
    )
    .unwrap();
    assert_eq!(read_origin(&plain).as_deref(), Some("https://h/o/plain"));
    assert_eq!(read_origin(dir.path()), None);
}
