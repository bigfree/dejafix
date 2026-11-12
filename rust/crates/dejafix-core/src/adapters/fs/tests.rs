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

mod store {
    use std::fs;
    use std::thread;

    use super::super::{
        GlobalStore, StoreError, clear_marker, find_memory_dir, lock_events, read_marker,
        write_marker, write_text_if_changed,
    };
    use crate::adapters::fs::write::write_atomic;

    fn project(dir: &std::path::Path) {
        fs::create_dir_all(dir.join(".dejafix")).unwrap();
        fs::write(dir.join(".dejafix/config.toml"), "").unwrap();
    }

    #[test]
    fn the_memory_dir_is_found_here_or_in_a_parent_but_never_the_global_store() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src/deep")).unwrap();
        project(&root);
        assert_eq!(find_memory_dir(&root).unwrap().root, root);
        let found = find_memory_dir(&root.join("src/deep")).unwrap();
        assert_eq!(found.root, fs::canonicalize(&root).unwrap());
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".dejafix/global")).unwrap();
        fs::create_dir_all(home.join("work")).unwrap();
        let error = find_memory_dir(&home.join("work")).unwrap_err();
        assert!(matches!(error, StoreError::NotFound { .. }));
        assert!(error.to_string().starts_with(&format!(
            "No .dejafix directory found in {} or any parent. If running over MCP",
            home.join("work").display()
        )));
    }

    #[test]
    fn locked_writers_never_interleave_read_and_append() {
        let dir = tempfile::tempdir().unwrap();
        let mem = dir.path().to_path_buf();
        let workers: Vec<_> = (0..2)
            .map(|worker| {
                let mem = mem.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        let (mut log, bytes) = lock_events(&mem).unwrap();
                        let count = String::from_utf8_lossy(&bytes).lines().count();
                        log.append(format!("{worker} {count}\n").as_bytes())
                            .unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let text = fs::read_to_string(mem.join("events.jsonl")).unwrap();
        for (index, line) in text.lines().enumerate() {
            assert_eq!(line.split(' ').nth(1).unwrap(), index.to_string());
        }
        assert_eq!(text.lines().count(), 200);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_keep_the_mode_and_leave_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("summary.md");
        write_atomic(&path, b"one").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["summary.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_atomic_write_replaces_a_symlink_instead_of_following_it() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::write(&outside, "keep").unwrap();
        let link = dir.path().join("summary.md");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
    }

    #[test]
    fn an_unterminated_last_line_is_closed_before_appending() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("events.jsonl"), "{\"a\": 1}").unwrap();
        let (mut log, _) = lock_events(dir.path()).unwrap();
        log.append(b"{\"b\": 2}\n").unwrap();
        log.append(b"{\"c\": 3}\n").unwrap();
        drop(log);
        let text = fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            ["{\"a\": 1}", "{\"b\": 2}", "{\"c\": 3}"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_replaced_by_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("summary.md");
        std::os::unix::fs::symlink(dir.path().join("gone"), &link).unwrap();
        write_atomic(&link, b"new").unwrap();
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
        assert!(!dir.path().join("gone").exists());
    }

    #[test]
    fn unchanged_contents_are_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001-x.md");
        assert!(write_text_if_changed(&path, "a\n").unwrap());
        assert!(!write_text_if_changed(&path, "a\n").unwrap());
        assert!(write_text_if_changed(&path, "b\n").unwrap());
    }

    #[test]
    fn the_marker_is_a_bare_id() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_marker(dir.path()), None);
        write_marker(dir.path(), "0007");
        assert_eq!(
            fs::read_to_string(dir.path().join(".current_issue")).unwrap(),
            "0007"
        );
        fs::write(dir.path().join(".current_issue"), " 0008 \n").unwrap();
        assert_eq!(read_marker(dir.path()).as_deref(), Some("0008"));
        fs::write(dir.path().join(".current_issue"), "  ").unwrap();
        assert_eq!(read_marker(dir.path()), None);
        clear_marker(dir.path());
        clear_marker(dir.path());
        assert!(!dir.path().join(".current_issue").exists());
    }

    #[test]
    fn the_promotable_cache_grows_only_with_new_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let store = GlobalStore::new(dir.path());
        let cache = dir.path().join("global/.promotable.json");
        store
            .record_promotable(&["React".into(), String::new()])
            .unwrap();
        assert!(!cache.exists());
        store.record_promotable(&["zod".into()]).unwrap();
        let text = fs::read_to_string(&cache).unwrap();
        assert!(text.starts_with("{\n  \"libraries\": [\n    \"alembic\","));
        assert!(text.contains("\"zod\"") && !text.ends_with('\n'));
        assert_eq!(store.promotable().unwrap().len(), 28);
        fs::write(&cache, "{broken").unwrap();
        assert_eq!(store.promotable(), Some(Vec::new()));
        fs::write(&cache, "[1]").unwrap();
        assert_eq!(store.promotable(), None);
    }

    #[test]
    fn gotchas_are_appended_once_as_sorted_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let store = GlobalStore::new(dir.path());
        assert_eq!(store.gotchas(), Some(Vec::new()));
        let entry = serde_json::json!({"library": "react", "id": "got_1"})
            .as_object()
            .cloned()
            .unwrap();
        assert!(
            store
                .append_gotcha_if_new(|existing| {
                    assert_eq!(existing.len(), 0);
                    Some(entry.clone())
                })
                .unwrap()
        );
        assert!(
            !store
                .append_gotcha_if_new(|existing| {
                    assert_eq!(existing.len(), 1);
                    None
                })
                .unwrap()
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("global/library_gotchas.jsonl")).unwrap(),
            "{\"id\": \"got_1\", \"library\": \"react\"}\n"
        );
        fs::write(dir.path().join("global/library_gotchas.jsonl"), "[1]\n").unwrap();
        assert_eq!(store.gotchas(), None);
        assert!(!store.append_gotcha_if_new(|_| Some(entry.clone())).unwrap());
    }
}
