use std::process::Command;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    git_dirs, head_short, hooks_dir, last_changes, last_message, read_hook, remove_hook,
    repository_state, resolve_djf_binary, staged_files, status_files, working_files, write_hook,
};
use crate::core::capture::Change;

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

#[test]
fn git_returns_nothing_outside_a_repository_or_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(super::git(dir.path(), &["rev-parse", "HEAD"]), None);
    assert_eq!(
        super::git(&dir.path().join("missing"), &["--version"]),
        None
    );
    let version = super::git(dir.path(), &["--version"]).unwrap();
    assert!(version.starts_with(b"git version"));
}

#[test]
fn git_reads_large_output_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    for index in 0..3 {
        git(
            dir.path(),
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                &"x".repeat(40_000 + index),
            ],
        );
    }
    let log = super::git(dir.path(), &["log", "--format=%B"]).unwrap();
    assert!(log.len() > 120_000);
}

fn fake_binary(directory: &Path, name: &str, mode: u32) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(name);
    fs::write(&path, "#!/bin/sh\n").unwrap();
    set_mode(&path, mode);
    path
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

fn path_of(directory: &Path) -> OsString {
    std::env::join_paths([directory]).unwrap()
}

#[test]
fn the_hooks_directory_sits_under_dot_git() {
    assert_eq!(
        hooks_dir(Path::new("/repo")),
        Path::new("/repo").join(".git").join("hooks")
    );
}

#[test]
fn the_djf_next_to_the_executable_beats_path() {
    let dir = tempfile::tempdir().unwrap();
    let adjacent = fake_binary(&dir.path().join("venv").join("bin"), "djf", 0o755);
    fake_binary(&dir.path().join("other"), "djf", 0o755);
    let path_var = path_of(&dir.path().join("other"));
    let resolved = resolve_djf_binary(adjacent.parent(), Some(&path_var), false);
    assert_eq!(resolved, adjacent.to_string_lossy());
}

#[test]
fn the_dejafix_alias_is_the_second_choice_next_to_the_executable() {
    let dir = tempfile::tempdir().unwrap();
    let alias = fake_binary(dir.path(), "dejafix", 0o755);
    let resolved = resolve_djf_binary(Some(dir.path()), None, false);
    assert_eq!(resolved, alias.to_string_lossy());
}

#[cfg(unix)]
#[test]
fn a_djf_that_is_not_executable_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let adjacent = fake_binary(&dir.path().join("venv"), "djf", 0o644);
    let on_path = fake_binary(&dir.path().join("local"), "djf", 0o755);
    let path_var = path_of(on_path.parent().unwrap());
    let resolved = resolve_djf_binary(adjacent.parent(), Some(&path_var), false);
    assert_eq!(resolved, on_path.to_string_lossy());
}

#[test]
fn path_answers_when_nothing_is_adjacent() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("venv")).unwrap();
    let on_path = fake_binary(&dir.path().join("local"), "djf", 0o755);
    let path_var = path_of(on_path.parent().unwrap());
    let resolved = resolve_djf_binary(Some(&dir.path().join("venv")), Some(&path_var), false);
    assert_eq!(resolved, on_path.to_string_lossy());
}

#[test]
fn path_prefers_djf_over_the_alias_in_any_directory() {
    let dir = tempfile::tempdir().unwrap();
    let first = fake_binary(&dir.path().join("first"), "dejafix", 0o755);
    let second = fake_binary(&dir.path().join("second"), "djf", 0o755);
    let path_var =
        std::env::join_paths([first.parent().unwrap(), second.parent().unwrap()]).unwrap();
    assert_eq!(
        resolve_djf_binary(None, Some(&path_var), false),
        second.to_string_lossy()
    );
}

#[test]
fn nothing_found_is_the_bare_name() {
    let dir = tempfile::tempdir().unwrap();
    let path_var = path_of(dir.path());
    assert_eq!(
        resolve_djf_binary(Some(dir.path()), Some(&path_var), false),
        "djf"
    );
    assert_eq!(resolve_djf_binary(None, None, false), "djf");
}

#[test]
fn a_directory_named_djf_is_not_a_binary() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("djf")).unwrap();
    assert_eq!(resolve_djf_binary(Some(dir.path()), None, false), "djf");
}

#[test]
fn windows_finds_scripts_djf_exe_with_forward_slashes() {
    let dir = tempfile::tempdir().unwrap();
    let binary = fake_binary(&dir.path().join("venv").join("Scripts"), "djf.exe", 0o644);
    let resolved = resolve_djf_binary(binary.parent(), None, true);
    assert!(resolved.ends_with("/Scripts/djf.exe"));
    assert!(!resolved.contains('\\'));
    assert_eq!(resolved, binary.to_string_lossy().replace('\\', "/"));
}

#[test]
fn a_missing_hook_reads_as_nothing_and_a_present_one_as_text() {
    let dir = tempfile::tempdir().unwrap();
    let hook = dir.path().join("pre-commit");
    assert_eq!(read_hook(&hook).unwrap(), None);
    fs::write(&hook, "a\r\nb\r").unwrap();
    assert_eq!(read_hook(&hook).unwrap().as_deref(), Some("a\nb\n"));
    fs::write(&hook, [0xff_u8, 0xfe]).unwrap();
    assert!(read_hook(&hook).is_err());
}

#[test]
fn writing_a_new_hook_creates_it_executable() {
    let dir = tempfile::tempdir().unwrap();
    let hook = dir.path().join("post-commit");
    write_hook(&hook, "#!/bin/sh\necho hi\n", false).unwrap();
    assert_eq!(
        fs::read_to_string(&hook).unwrap().replace("\r\n", "\n"),
        "#!/bin/sh\necho hi\n"
    );
    assert_executable(&hook);
}

#[test]
fn creating_a_hook_that_exists_fails() {
    let dir = tempfile::tempdir().unwrap();
    let hook = dir.path().join("post-commit");
    fs::write(&hook, "mine").unwrap();
    assert!(write_hook(&hook, "new", false).is_err());
    assert_eq!(fs::read_to_string(&hook).unwrap(), "mine");
}

#[test]
fn rewriting_an_existing_hook_makes_it_executable() {
    let dir = tempfile::tempdir().unwrap();
    let hook = dir.path().join("pre-commit");
    fs::write(&hook, "old").unwrap();
    set_mode(&hook, 0o644);
    write_hook(&hook, "new\n", true).unwrap();
    assert_eq!(
        fs::read_to_string(&hook).unwrap().replace("\r\n", "\n"),
        "new\n"
    );
    assert_executable(&hook);
}

#[cfg(unix)]
#[test]
fn a_symlinked_hook_is_written_through() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("shared-hook.sh");
    fs::write(&target, "old\n").unwrap();
    set_mode(&target, 0o644);
    let hook = dir.path().join("pre-commit");
    std::os::unix::fs::symlink(&target, &hook).unwrap();
    write_hook(&hook, "new\n", true).unwrap();
    assert!(
        fs::symlink_metadata(&hook)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "new\n");
    assert_executable(&target);
}

#[test]
fn removing_a_hook_deletes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let hook = dir.path().join("pre-commit");
    fs::write(&hook, "x").unwrap();
    remove_hook(&hook).unwrap();
    assert!(!hook.exists());
    assert!(remove_hook(&hook).is_err());
}

#[cfg(unix)]
fn assert_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111);
}

#[cfg(not(unix))]
fn assert_executable(_path: &Path) {}

fn commit_files(dir: &std::path::Path, message: &str, files: &[(&str, &str)]) {
    for (path, text) in files {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
        git(dir, &["add", path]);
    }
    git(dir, &["commit", "-q", "-m", message]);
}

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    dir
}

fn change(status: char, path: &str) -> Change {
    Change {
        status,
        path: path.to_owned(),
    }
}

#[test]
fn last_message_is_stripped_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(last_message(dir.path()), None);
    let dir = repository();
    assert_eq!(last_message(dir.path()), None);
    git(
        dir.path(),
        &["commit", "-q", "--allow-empty", "-m", "fix: x\n\nbody"],
    );
    assert_eq!(last_message(dir.path()).as_deref(), Some("fix: x\n\nbody"));
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "--allow-empty",
            "--cleanup=verbatim",
            "-m",
            "\u{a0}y\u{a0}\n\u{1c}",
        ],
    );
    assert_eq!(last_message(dir.path()).as_deref(), Some("y"));
}

#[test]
fn last_changes_of_the_first_commit_fall_back_to_root() {
    let dir = repository();
    commit_files(dir.path(), "fix: first", &[("src/app.py", "x\n")]);
    assert_eq!(last_changes(dir.path()), vec![change('A', "src/app.py")]);
}

#[test]
fn last_changes_lists_a_rename_at_the_new_path() {
    let dir = repository();
    commit_files(
        dir.path(),
        "add: handler",
        &[("src/old_name.py", "x = 1\n")],
    );
    git(dir.path(), &["mv", "src/old_name.py", "src/new_name.py"]);
    git(dir.path(), &["commit", "-q", "-m", "fix: rename"]);
    assert_eq!(
        last_changes(dir.path()),
        vec![change('R', "src/new_name.py")]
    );
}

#[test]
fn last_changes_lists_deletions_and_modifications() {
    let dir = repository();
    commit_files(dir.path(), "add: two", &[("a.py", "1\n"), ("b.py", "1\n")]);
    std::fs::remove_file(dir.path().join("a.py")).unwrap();
    std::fs::write(dir.path().join("b.py"), "2\n").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "fix: change"]);
    assert_eq!(
        last_changes(dir.path()),
        vec![change('D', "a.py"), change('M', "b.py")]
    );
}

#[test]
fn last_changes_of_a_merge_lists_only_the_merged_branch() {
    let dir = repository();
    commit_files(dir.path(), "add: base", &[("src/base.py", "b\n")]);
    git(dir.path(), &["checkout", "-qb", "feature"]);
    commit_files(dir.path(), "add: feature", &[("src/feature.py", "f\n")]);
    git(dir.path(), &["checkout", "-q", "-"]);
    commit_files(dir.path(), "add: mainline", &[("src/other.py", "o\n")]);
    git(
        dir.path(),
        &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"],
    );
    assert_eq!(
        last_changes(dir.path()),
        vec![change('A', "src/feature.py")]
    );
}

#[test]
fn last_changes_keeps_a_non_ascii_path_as_written() {
    let dir = repository();
    commit_files(dir.path(), "fix: accents", &[("src/caf\u{e9}.py", "x\n")]);
    assert_eq!(
        last_changes(dir.path()),
        vec![change('A', "src/caf\u{e9}.py")]
    );
}

#[test]
fn staged_and_working_files_are_listed_apart() {
    let dir = repository();
    commit_files(dir.path(), "add: base", &[("a.py", "1\n"), ("b.py", "1\n")]);
    assert_eq!(staged_files(dir.path()), Vec::<String>::new());
    assert_eq!(working_files(dir.path()), Vec::<String>::new());
    std::fs::write(dir.path().join("a.py"), "2\n").unwrap();
    git(dir.path(), &["add", "a.py"]);
    std::fs::write(dir.path().join("b.py"), "2\n").unwrap();
    assert_eq!(staged_files(dir.path()), ["a.py"]);
    assert_eq!(working_files(dir.path()), ["b.py"]);
}

#[test]
fn diff_queries_are_empty_outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(last_message(dir.path()), None);
    assert_eq!(last_changes(dir.path()), Vec::<Change>::new());
    assert_eq!(staged_files(dir.path()), Vec::<String>::new());
    assert_eq!(working_files(dir.path()), Vec::<String>::new());
}

fn dated_commit(dir: &Path, path: &str, text: &str, message: &str, when: &str) {
    fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
    fs::write(dir.join(path), text).unwrap();
    git(dir, &["add", path]);
    let status = Command::new("git")
        .args(["commit", "-qm", message])
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .env("GIT_AUTHOR_DATE", when)
        .env("GIT_COMMITTER_DATE", when)
        .status()
        .unwrap();
    assert!(status.success());
}

fn offset_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    dated_commit(
        dir.path(),
        "src/f.py",
        "v0\n",
        "init",
        "2026-01-01T10:00:00Z",
    );
    for (index, when) in [
        "2026-05-01T15:00:00+06:00",
        "2026-05-01T16:00:00+06:00",
        "2026-05-01T07:00:00-06:00",
        "2026-05-01T08:00:00-06:00",
        "2026-05-01T09:00:00-06:00",
    ]
    .into_iter()
    .enumerate()
    {
        dated_commit(
            dir.path(),
            "src/f.py",
            &format!("v{}\n", index + 1),
            "edit",
            when,
        );
    }
    dir
}

#[test]
fn commit_times_are_canonical_zulu_whatever_the_committer_offset() {
    use crate::core::stale::FileHistory;

    let dir = offset_repository();
    let history = super::GitHistory {
        root: dir.path().to_path_buf(),
    };
    let times = history.commit_times("src/f.py", None).unwrap();
    assert_eq!(
        times.get(..3).unwrap(),
        [
            "2026-05-01T15:00:00Z",
            "2026-05-01T14:00:00Z",
            "2026-05-01T13:00:00Z"
        ]
    );
    assert!(
        times
            .iter()
            .all(|time| time.ends_with('Z') && !time.contains('+'))
    );
    let after = times
        .iter()
        .filter(|time| time.as_str() > "2026-05-01T12:00:00Z")
        .count();
    assert_eq!(after, 3);
    assert!(history.exists("src/f.py") && !history.exists("src/gone.py"));
}

#[test]
fn counts_match_a_per_event_git_log() {
    use crate::core::stale::FileHistory;

    let dir = offset_repository();
    let history = super::GitHistory {
        root: dir.path().to_path_buf(),
    };
    let times = history
        .commit_times("src/f.py", Some("2026-01-15T00:00:00Z"))
        .unwrap();
    for cutoff in [
        "2026-01-15T00:00:00Z",
        "2026-05-01T09:30:00Z",
        "2026-05-01T14:30:00Z",
        "2026-12-01T00:00:00Z",
    ] {
        let direct = Command::new("git")
            .args([
                "log",
                &format!("--since={cutoff}"),
                "--oneline",
                "--",
                "src/f.py",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let expected = String::from_utf8(direct.stdout).unwrap().lines().count();
        let counted = times.iter().filter(|time| time.as_str() >= cutoff).count();
        assert_eq!(counted, expected, "{cutoff}");
    }
}

#[test]
fn history_outside_a_repository_cannot_judge() {
    use crate::core::stale::FileHistory;

    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/f.py"), "x").unwrap();
    let history = super::GitHistory {
        root: dir.path().to_path_buf(),
    };
    assert_eq!(history.commit_times("src/f.py", None), None);
}

#[test]
fn a_failing_git_is_told_apart_from_a_missing_one() {
    use super::types::GitFailure;

    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        super::git_status(dir.path(), &["rev-parse", "HEAD"]),
        Err(GitFailure::Exit)
    );
    assert_eq!(
        super::git_status(&dir.path().join("missing"), &["--version"]),
        Err(GitFailure::Unavailable)
    );
}

fn repository_with_changes() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("tracked.txt"), "one").unwrap();
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "x"]);
    fs::write(dir.path().join("tracked.txt"), "two").unwrap();
    fs::write(dir.path().join("new.txt"), "n").unwrap();
    dir
}

#[test]
fn status_files_lists_staged_and_untracked_files() {
    let dir = repository_with_changes();
    git(dir.path(), &["add", "tracked.txt"]);
    assert_eq!(status_files(dir.path()), ["tracked.txt", "new.txt"]);
}

#[test]
fn status_files_cut_the_first_line_after_the_trimmed_output_like_python() {
    let dir = repository_with_changes();
    assert_eq!(status_files(dir.path()), ["racked.txt", "new.txt"]);
}

#[test]
fn status_files_are_empty_for_a_clean_tree_or_no_repository() {
    let dir = tempfile::tempdir().unwrap();
    let none = Vec::<String>::new();
    assert_eq!(status_files(dir.path()), none);
    git(dir.path(), &["init", "-q"]);
    assert_eq!(status_files(dir.path()), none);
    assert_eq!(status_files(&dir.path().join("missing")), none);
}

fn state_of(root: &Path) -> Option<super::RepoState> {
    git_dirs(root).as_ref().and_then(repository_state)
}

#[test]
fn the_repository_state_changes_with_every_commit_and_branch_switch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert_eq!(git_dirs(root), None);
    git(root, &["init", "-q", "-b", "main"]);
    let empty = state_of(root).unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "one"]);
    let one = state_of(root).unwrap();
    assert_ne!(empty, one);
    assert_eq!(state_of(root), Some(one.clone()));
    git(root, &["commit", "-q", "--allow-empty", "-m", "two"]);
    let two = state_of(root).unwrap();
    assert_ne!(one, two);
    git(root, &["checkout", "-q", "--detach", "HEAD~1"]);
    let detached = state_of(root).unwrap();
    assert_ne!(two, detached);
    git(root, &["checkout", "-q", "main"]);
    git(root, &["pack-refs", "--all"]);
    let packed = state_of(root).unwrap();
    assert_ne!(two, packed);
    let nested = root.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(state_of(&nested), Some(packed));
}

#[test]
fn a_worktree_state_follows_its_own_head_through_the_git_file() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&main, &["worktree", "add", "-q", "-b", "side", "../side"]);
    let side = dir.path().join("side");
    assert!(side.join(".git").is_file());
    let before = state_of(&side).unwrap();
    let main_before = state_of(&main).unwrap();
    git(&side, &["commit", "-q", "--allow-empty", "-m", "side work"]);
    assert_ne!(state_of(&side).unwrap(), before);
    assert_eq!(state_of(&main).unwrap(), main_before);
}

#[test]
fn a_reftable_repository_state_changes_with_every_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "--ref-format=reftable", "-b", "main"]);
    git(root, &["commit", "-q", "--allow-empty", "-m", "one"]);
    let one = state_of(root).unwrap();
    git(root, &["commit", "-q", "--allow-empty", "-m", "two"]);
    assert_ne!(state_of(root).unwrap(), one);
}

#[test]
fn git_dirs_follow_the_git_dir_git_itself_uses() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    let dirs = git_dirs(&root.join(".")).unwrap();
    assert_eq!(
        fs::canonicalize(&dirs.dir).unwrap(),
        fs::canonicalize(root.join(".git")).unwrap()
    );
    assert_eq!(
        fs::canonicalize(&dirs.common).unwrap(),
        fs::canonicalize(root.join(".git")).unwrap()
    );
}
