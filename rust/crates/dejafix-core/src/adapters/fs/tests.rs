use std::fs;

use super::resolve::is_network_path;
use super::{FsResolver, StoreError, path_context, read_events_file};
use crate::core::log::EventLog;
use crate::core::path::{PathResolver, project_relative};
use crate::testing::fixed_clock;

#[test]
fn reading_a_missing_log_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let error = read_events_file(dir.path()).unwrap_err();
    assert!(matches!(error, StoreError::Read { ref path, .. } if path.ends_with("events.jsonl")));
    assert!(error.to_string().contains("events.jsonl"), "{error}");
}

#[test]
fn a_missing_root_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let error = path_context(&dir.path().join("gone")).unwrap_err();
    assert!(matches!(error, StoreError::Root { .. }));
}

#[cfg(unix)]
#[test]
fn symlinks_resolve_and_a_missing_tail_is_kept() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = dir.path().join("link");
    symlink(&target, &link).unwrap();
    let real = fs::canonicalize(&target).unwrap();
    let resolved = FsResolver
        .resolve(link.join("missing/a.py").to_str().unwrap())
        .unwrap();
    assert_eq!(resolved, format!("{}/missing/a.py", real.display()));
    let context = path_context(&link).unwrap();
    assert_eq!(context.root, real.display().to_string());
    assert!(!context.fold_case);
    let spelled = link.join("src/a.py");
    assert_eq!(
        project_relative(spelled.to_str().unwrap(), &context, &FsResolver).as_str(),
        "src/a.py"
    );
}

#[test]
fn a_deep_missing_path_resolves_through_its_deepest_existing_ancestor() {
    let dir = tempfile::tempdir().unwrap();
    let real = fs::canonicalize(dir.path()).unwrap();
    let tail: Vec<String> = (0..2_000).map(|n| format!("d{n}")).collect();
    let deep = dir.path().join(tail.join("/"));
    let resolved = FsResolver.resolve(deep.to_str().unwrap()).unwrap();
    assert_eq!(resolved, format!("{}/{}", real.display(), tail.join("/")));
}

#[cfg(unix)]
#[test]
fn a_relative_path_resolves_against_the_working_directory() {
    let resolved = FsResolver.resolve("no-such-dir/a.py").unwrap();
    let cwd = fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    assert_eq!(resolved, format!("{}/no-such-dir/a.py", cwd.display()));
}

#[test]
fn a_fixture_loads_without_skips() {
    use crate::testing::{FIXTURE_SEED, write_project};

    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path(), 100, FIXTURE_SEED).unwrap();
    let bytes = read_events_file(&dir.path().join(".dejafix")).unwrap();
    let context = path_context(dir.path()).unwrap();
    let log = EventLog::parse(&bytes, &fixed_clock(), &context, &FsResolver);
    assert_eq!(log.events().len(), 100);
    assert_eq!(log.skipped(), []);
    assert!(log.indexed_files().count() > 0);
}

#[test]
fn unc_spellings_are_network_paths() {
    for path in [
        "//server/share/a.py",
        "\\\\server\\share\\a.py",
        "/\\server/a",
    ] {
        assert!(is_network_path(path), "{path}");
    }
    for path in ["/server/a.py", "C:/x", "C:\\x", "rel/a.py", ""] {
        assert!(!is_network_path(path), "{path}");
    }
}
