use std::fs;

use super::{
    FsResolver, RegistryFiles, StoreError, manifest_info, path_context, read_events_file,
    resolve_path,
};
use crate::core::log::EventLog;
use crate::core::path::{PathResolver, project_relative};
use crate::core::text::platform_newlines;
use crate::testing::fixed_clock;

fn p(path: &str) -> String {
    if cfg!(windows) {
        format!("C:{}", path.replace('/', "\\"))
    } else {
        path.to_owned()
    }
}

fn quoted(path: &str) -> String {
    serde_json::json!(p(path)).to_string()
}

fn nl(text: &str) -> String {
    platform_newlines(text).into_owned()
}

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
    let real = super::resolve::posix(&fs::canonicalize(dir.path()).unwrap());
    let tail: Vec<String> = (0..2_000).map(|n| format!("d{n}")).collect();
    let deep = dir.path().join(tail.join("/"));
    let resolved = FsResolver.resolve(deep.to_str().unwrap()).unwrap();
    assert_eq!(resolved, format!("{real}/{}", tail.join("/")));
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

mod store {
    use std::fs;
    use std::thread;

    use super::super::{
        GlobalStore, GotchaWrite, StoreError, clear_marker, find_memory_dir, lock_events,
        read_marker, write_marker, write_text_if_changed,
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
        assert_eq!(found.root, crate::canonical_path(&root).unwrap());
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
    fn gotchas_are_appended_under_the_lock_and_a_corrupt_store_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = GlobalStore::new(dir.path());
        assert_eq!(store.gotchas(), Some(Vec::new()));
        let line = "{\"id\": \"got_1\", \"library\": \"react\"}".to_owned();
        let seen = store
            .edit_gotchas(|existing| (GotchaWrite::Append(vec![line.clone()]), existing.len()))
            .unwrap();
        assert_eq!(seen, 0);
        let seen = store
            .edit_gotchas(|existing| (GotchaWrite::Keep, existing.len()))
            .unwrap();
        assert_eq!(seen, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("global/library_gotchas.jsonl")).unwrap(),
            super::nl("{\"id\": \"got_1\", \"library\": \"react\"}\n")
        );
        fs::write(dir.path().join("global/library_gotchas.jsonl"), "[1]\n").unwrap();
        assert_eq!(store.gotchas(), None);
        assert!(matches!(
            store.edit_gotchas(|_| (GotchaWrite::Keep, ())),
            Err(StoreError::Read { .. })
        ));
    }
}

fn registry_home() -> (tempfile::TempDir, RegistryFiles) {
    let dir = tempfile::tempdir().unwrap();
    let files = RegistryFiles::new(&dir.path().join("pm"));
    (dir, files)
}

const NOW: &str = "2027-01-15T08:00:00Z";

#[test]
fn a_0_2_x_registry_keeps_its_list_and_gains_a_sidecar() {
    let (_dir, files) = registry_home();
    fs::create_dir_all(files.list.parent().unwrap()).unwrap();
    let list = format!(
        "[\n {},\n {}\n]",
        quoted("/code/alpha"),
        quoted("/code/beta")
    );
    fs::write(&files.list, &list).unwrap();
    let registry = files.load(NOW).unwrap();
    assert_eq!(registry.projects.len(), 2);
    assert_eq!(
        fs::read_to_string(&files.list).unwrap(),
        nl(&format!("{list}\n"))
    );
    let meta = fs::read_to_string(&files.meta).unwrap();
    assert!(meta.starts_with(&nl(&format!(
        "{{\n \"schema_version\": 1,\n \"active_project\": null,\n \"by_path\": {{\n  {}: {{\n   \"id\": \"alpha\",",
        quoted("/code/alpha")
    ))));
    assert!(meta.ends_with(&nl("}\n")));
    let leftovers: Vec<_> = fs::read_dir(files.list.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".pm-"))
        .collect();
    assert_eq!(leftovers.len(), 0);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&files.meta).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn a_utf8_bom_and_a_corrupt_file_are_tolerated() {
    let (_dir, files) = registry_home();
    fs::create_dir_all(files.list.parent().unwrap()).unwrap();
    fs::write(&files.list, format!("\u{feff}[{}]", quoted("/code/alpha"))).unwrap();
    assert_eq!(files.load(NOW).unwrap().projects[0].id, "alpha");
    fs::write(&files.list, "{not json").unwrap();
    fs::write(&files.meta, "[1]").unwrap();
    assert_eq!(files.load(NOW).unwrap().projects.len(), 0);
    assert_eq!(files.read_meta().len(), 0);
}

#[test]
fn a_dev_format_registry_is_converted_and_backed_up_once() {
    let (_dir, files) = registry_home();
    fs::create_dir_all(files.list.parent().unwrap()).unwrap();
    let original = format!(
        r#"{{"active_project": "alpha", "projects": [{{"id": "alpha", "path": {}}}]}}"#,
        quoted("/code/alpha")
    );
    fs::write(&files.list, &original).unwrap();
    let registry = files.load(NOW).unwrap();
    assert_eq!(registry.active_project.as_deref(), Some("alpha"));
    let backup = files.list.with_file_name("projects.json.bak");
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    assert_eq!(
        fs::read_to_string(&files.list).unwrap(),
        nl(&format!("[\n {}\n]\n", quoted("/code/alpha")))
    );
    fs::write(&files.list, r#"{"projects": []}"#).unwrap();
    files.load(NOW).unwrap();
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);
}

#[test]
fn the_version_stamp_keeps_other_keys_in_place() {
    let (_dir, files) = registry_home();
    assert_eq!(files.seen_version(), None);
    files.record_version("1.0.0");
    assert_eq!(
        fs::read_to_string(&files.meta).unwrap(),
        nl("{\n \"last_version\": \"1.0.0\"\n}\n")
    );
    fs::write(
        &files.meta,
        r#"{"by_path": {}, "last_version": "0.2.0", "z": 1}"#,
    )
    .unwrap();
    assert_eq!(files.seen_version(), Some(serde_json::json!("0.2.0")));
    files.record_version("1.0.1");
    assert_eq!(
        fs::read_to_string(&files.meta).unwrap(),
        nl("{\n \"by_path\": {},\n \"last_version\": \"1.0.1\",\n \"z\": 1\n}\n")
    );
}

#[test]
fn an_unwritable_registry_reports_the_path() {
    let (dir, files) = registry_home();
    fs::write(dir.path().join("pm"), "a file, not a directory").unwrap();
    let error = files
        .save(&crate::core::registry::Registry::default())
        .unwrap_err();
    assert!(matches!(error, StoreError::Write { .. }));
    files.record_version("1.0.0");
}

#[cfg(unix)]
#[test]
fn paths_resolve_like_python_expanduser_and_resolve() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(root.join("real/sub")).unwrap();
    symlink(root.join("real"), root.join("link")).unwrap();
    let home = root.join("home");
    assert_eq!(resolve_path("~", Some(&home), &root), Some(home.clone()));
    assert_eq!(
        resolve_path("~/x", Some(&home), &root),
        Some(home.join("x"))
    );
    assert_eq!(
        resolve_path("~x", Some(&home), &root),
        Some(root.join("~x"))
    );
    assert_eq!(
        resolve_path("link/sub", None, &root),
        Some(root.join("real/sub"))
    );
    assert_eq!(
        resolve_path("link/missing/../gone/./y", None, &root),
        Some(root.join("real/gone/y"))
    );
    assert_eq!(resolve_path("", None, &root), Some(root.clone()));
    assert_eq!(
        resolve_path(".", None, &root.join("real")),
        Some(root.join("real"))
    );
}

fn manifest_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(dir.path().join(name), text).unwrap();
    }
    dir
}

#[test]
fn descriptions_come_from_pyproject_then_package_json_then_cargo() {
    let py = manifest_dir(&[
        ("pyproject.toml", "[project]\ndescription = \"The thing\"\n"),
        ("package.json", r#"{"description": "Node thing"}"#),
    ]);
    assert_eq!(
        manifest_info(py.path()).description.as_deref(),
        Some("The thing")
    );
    let node = manifest_dir(&[(
        "package.json",
        "\u{feff}{\"description\": \"  Node thing \"}",
    )]);
    assert_eq!(
        manifest_info(node.path()).description.as_deref(),
        Some("Node thing")
    );
    let cargo = manifest_dir(&[
        ("package.json", r#"{"description": " "}"#),
        ("Cargo.toml", "[package]\n  description = 'Rust thing'\n"),
    ]);
    assert_eq!(
        manifest_info(cargo.path()).description.as_deref(),
        Some("Rust thing")
    );
    let blank = manifest_dir(&[
        ("pyproject.toml", "description = \"  \"\n"),
        ("package.json", r#"{"description": "ignored"}"#),
    ]);
    assert_eq!(manifest_info(blank.path()).description.as_deref(), Some(""));
    let listed = manifest_dir(&[("package.json", "[1]")]);
    assert_eq!(manifest_info(listed.path()).description, None);
}

#[test]
fn entry_points_come_from_pyproject_scripts_and_npm_scripts() {
    let dir = manifest_dir(&[
        (
            "pyproject.toml",
            "[project.scripts]\nfoo = \"pkg.cli:main\"\nbar = 'pkg.other:run'\n[tool.something]\nfoo = \"should-not-appear\"\n",
        ),
        (
            "package.json",
            r#"{"scripts":{"dev":"vite","build":"vite build","test":"jest","custom":"x","start":5}}"#,
        ),
    ]);
    assert_eq!(
        manifest_info(dir.path()).entry_points,
        [
            "`foo` → `pkg.cli:main`",
            "`bar` → `pkg.other:run`",
            "`npm run dev` → `vite`",
            "`npm run build` → `vite build`",
            "`npm run test` → `jest`",
        ]
    );
    let odd = manifest_dir(&[("package.json", r#"{"scripts": ["test", "start"]}"#)]);
    assert_eq!(manifest_info(odd.path()).entry_points.len(), 0);
    let text = manifest_dir(&[("package.json", r#"{"scripts": "restart"}"#)]);
    assert_eq!(manifest_info(text.path()).entry_points.len(), 0);
    let many = (0..12).fold(String::new(), |mut text, n| {
        text.push_str(&["s", &n.to_string(), " = \"x\"\n"].concat());
        text
    });
    let capped = manifest_dir(&[("pyproject.toml", &format!("[project.scripts]\n{many}"))]);
    assert_eq!(manifest_info(capped.path()).entry_points.len(), 8);
}

#[test]
fn main_folders_skip_dependencies_and_dot_folders() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "src",
        "tests",
        "docs",
        "node_modules",
        ".github",
        "core",
        "target",
    ] {
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    fs::write(dir.path().join("file.txt"), "").unwrap();
    assert_eq!(
        manifest_info(dir.path()).main_folders,
        [
            ("core".to_owned(), ""),
            ("docs".to_owned(), "documentation"),
            ("src".to_owned(), "package or application code"),
            ("tests".to_owned(), "test suite"),
        ]
    );
    for n in 0..20 {
        fs::create_dir(dir.path().join(format!("z{n:02}"))).unwrap();
    }
    assert_eq!(manifest_info(dir.path()).main_folders.len(), 15);
    assert_eq!(
        manifest_info(&dir.path().join("gone")).main_folders.len(),
        0
    );
}

#[test]
fn no_gotcha_file_is_created_when_nothing_is_written() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store
        .edit_gotchas(|_| (super::GotchaWrite::Keep, ()))
        .unwrap();
    assert!(!store.dir.join("library_gotchas.jsonl").exists());
    assert!(!store.dir.join(".promotable.json").exists());
    store
        .edit_gotchas(|existing| {
            assert_eq!(existing, Vec::<crate::core::global::Entry>::new());
            (super::GotchaWrite::Append(vec!["{}".to_owned()]), ())
        })
        .unwrap();
    assert!(store.dir.join("library_gotchas.jsonl").exists());
}

#[cfg(unix)]
#[test]
fn global_removal_replaces_the_file_and_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    let path = store.path("library_gotchas.jsonl");
    fs::write(
        &path,
        "{\"gotcha\": \"a\", \"id\": \"got_a\", \"library\": \"x\"}\n{\"gotcha\": \"b\", \"id\": \"got_b\", \"library\": \"x\"}\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(store.remove("got_a").unwrap());
    assert!(!store.remove("got_zzz").unwrap());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "{\"gotcha\": \"b\", \"id\": \"got_b\", \"library\": \"x\"}\n"
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    store
        .replace_gotchas(&["{\"id\": \"got_c\"}".to_owned()])
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "{\"id\": \"got_c\"}\n");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn a_replace_succeeds_while_the_store_is_open_and_locks_a_separate_file() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    let path = store.path("library_gotchas.jsonl");
    fs::write(&path, "{\"id\": \"got_old\"}\n").unwrap();
    let reader = fs::File::open(&path).unwrap();
    store
        .replace_gotchas(&["{\"id\": \"got_new\"}".to_owned()])
        .unwrap();
    drop(reader);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        nl("{\"id\": \"got_new\"}\n")
    );
    assert_eq!(
        fs::read(store.path("library_gotchas.jsonl.lock")).unwrap(),
        Vec::<u8>::new()
    );
}

#[test]
fn an_append_waiting_for_the_lock_lands_in_the_replaced_file() {
    use std::sync::mpsc;
    use std::time::Duration;
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    fs::write(
        store.path("library_gotchas.jsonl"),
        "{\"id\": \"got_old\"}\n",
    )
    .unwrap();
    let (started, waiting) = mpsc::channel();
    let appender = {
        let store = store.clone();
        std::thread::spawn(move || {
            waiting.recv().unwrap();
            store
                .edit_gotchas(|existing| {
                    let ids: Vec<String> = existing
                        .iter()
                        .filter_map(|entry| entry.get("id")?.as_str().map(str::to_owned))
                        .collect();
                    (
                        super::GotchaWrite::Append(vec!["{\"id\": \"got_late\"}".to_owned()]),
                        ids,
                    )
                })
                .unwrap()
        })
    };
    store
        .edit_gotchas(|_| {
            started.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            (
                super::GotchaWrite::Replace(vec!["{\"id\": \"got_new\"}".to_owned()]),
                (),
            )
        })
        .unwrap();
    assert_eq!(appender.join().unwrap(), ["got_new"]);
    assert_eq!(
        fs::read_to_string(store.path("library_gotchas.jsonl")).unwrap(),
        nl("{\"id\": \"got_new\"}\n{\"id\": \"got_late\"}\n")
    );
}

#[test]
fn libraries_are_empty_until_saved_and_a_corrupt_file_is_an_error() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    assert_eq!(
        store.libraries().unwrap(),
        crate::core::global::Libraries::default()
    );
    let mut libraries = crate::core::global::Libraries::default();
    libraries.add_alias("next", "nextjs");
    store.save_libraries(&libraries).unwrap();
    assert_eq!(
        fs::read_to_string(store.path("libraries.json")).unwrap(),
        nl("{\n  \"next\": [\n    \"nextjs\"\n  ]\n}\n")
    );
    assert_eq!(store.libraries().unwrap(), libraries);
    fs::write(store.path("libraries.json"), "[1]").unwrap();
    assert!(matches!(
        store.libraries(),
        Err(super::StoreError::Read { .. })
    ));
}

#[test]
fn the_version_one_backup_is_written_once() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    fs::write(store.path("library_gotchas.jsonl"), "first\n").unwrap();
    let backup = store.backup_v1().unwrap();
    assert_eq!(backup, store.path("library_gotchas.v1.jsonl"));
    fs::write(store.path("library_gotchas.jsonl"), "second\n").unwrap();
    assert_eq!(store.backup_v1().unwrap(), backup);
    assert_eq!(fs::read_to_string(&backup).unwrap(), "first\n");
}

mod containment {
    use std::fs;

    use super::super::{
        MemoryDir, StoreError, find_memory_dir, lock_events, points_outside, read_events_file,
    };

    #[cfg(unix)]
    #[test]
    fn a_memory_folder_linked_out_of_the_project_is_refused() {
        use std::os::unix::fs::symlink;
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("repo");
        let outside = base.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("config.toml"), "").unwrap();
        fs::write(outside.join("summary.md"), "SECRET\n").unwrap();
        symlink(&outside, root.join(".dejafix")).unwrap();
        assert!(points_outside(&root.join(".dejafix/summary.md"), &root));
        assert!(matches!(
            find_memory_dir(&root),
            Err(StoreError::OutsideProject { .. })
        ));
        assert!(matches!(
            MemoryDir::at(&root),
            Err(StoreError::OutsideProject { .. })
        ));
        assert!(matches!(
            lock_events(&root.join(".dejafix")),
            Err(StoreError::OutsideProject { .. })
        ));
        assert!(!outside.join("events.jsonl").exists());
    }

    #[cfg(unix)]
    #[test]
    fn an_event_log_linked_out_of_the_project_is_neither_read_nor_appended() {
        use std::os::unix::fs::symlink;
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("repo");
        let secret = base.path().join("zshrc");
        fs::create_dir_all(root.join(".dejafix")).unwrap();
        fs::write(root.join(".dejafix/config.toml"), "").unwrap();
        fs::write(&secret, "export TOKEN=1\n").unwrap();
        symlink(&secret, root.join(".dejafix/events.jsonl")).unwrap();
        let memory = root.join(".dejafix");
        assert!(matches!(
            lock_events(&memory),
            Err(StoreError::OutsideProject { .. })
        ));
        assert!(matches!(
            read_events_file(&memory),
            Err(StoreError::OutsideProject { .. })
        ));
        assert_eq!(fs::read_to_string(&secret).unwrap(), "export TOKEN=1\n");
    }

    #[cfg(unix)]
    #[test]
    fn links_that_stay_inside_the_project_still_work() {
        use std::os::unix::fs::symlink;
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("repo");
        fs::create_dir_all(root.join("memory")).unwrap();
        fs::write(root.join("memory/config.toml"), "").unwrap();
        symlink(root.join("memory"), root.join(".dejafix")).unwrap();
        let memory = find_memory_dir(&root).unwrap();
        let (mut locked, bytes) = lock_events(&memory.dir).unwrap();
        assert_eq!(bytes, b"");
        locked.append(b"{}\n").unwrap();
        drop(locked);
        assert_eq!(
            fs::read_to_string(root.join("memory/events.jsonl")).unwrap(),
            "{}\n"
        );
    }

    #[test]
    fn a_missing_file_is_judged_by_its_nearest_existing_folder() {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("repo");
        fs::create_dir_all(&root).unwrap();
        assert!(!points_outside(&root.join(".dejafix/events.jsonl"), &root));
        assert!(!points_outside(&root.join(".gitignore"), &root));
        assert!(points_outside(&base.path().join("other/x"), &root));
        assert!(MemoryDir::at(&root).is_ok());
    }
}

#[test]
fn removing_a_gotcha_keeps_the_version_two_field_order() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    let kept = "{\"v\": 2, \"id\": \"got_1\", \"library\": \"zod\", \"keywords\": [\"a\", \"b\"], \"versions\": null, \"gotcha\": \"zod strips unknown keys by default.\"}";
    let legacy = "{\"gotcha\": \"old\", \"id\": \"got_3\", \"library\": \"zod\"}";
    fs::write(
        store.path("library_gotchas.jsonl"),
        format!("{kept}\n{{\"v\": 2, \"id\": \"got_2\", \"library\": \"zod\", \"keywords\": [], \"versions\": null, \"gotcha\": \"x\"}}\n{legacy}\n"),
    )
    .unwrap();
    assert!(store.remove("got_2").unwrap());
    assert_eq!(
        fs::read_to_string(store.path("library_gotchas.jsonl")).unwrap(),
        nl(&format!("{kept}\n{legacy}\n"))
    );
}

#[test]
fn an_existing_empty_store_is_kept_when_nothing_is_written() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    fs::write(store.path("library_gotchas.jsonl"), "").unwrap();
    store
        .edit_gotchas(|_| (super::GotchaWrite::Keep, ()))
        .unwrap();
    assert!(store.path("library_gotchas.jsonl").exists());
}

fn libraries_of(value: &serde_json::Value) -> crate::core::global::Libraries {
    crate::core::global::Libraries::from_value(value).unwrap()
}

#[test]
fn an_export_carries_libraries_and_an_import_into_a_fresh_home_restores_them() {
    let source_home = tempfile::tempdir().unwrap();
    let source = super::GlobalStore::new(source_home.path());
    let empty = source.export("2026-11-15T00:00:00Z").unwrap();
    assert!(empty.get("libraries").is_none());
    let libraries = libraries_of(&serde_json::json!({
        "next": ["next.js", "nextjs"],
        "nx": ["@nx/js"],
        "prisma": []
    }));
    source.save_libraries(&libraries).unwrap();
    let exported = source.export("2026-11-15T00:00:00Z").unwrap();
    assert_eq!(
        exported.get("libraries"),
        Some(&serde_json::json!({"next": ["next.js", "nextjs"], "nx": ["@nx/js"], "prisma": []}))
    );
    let keys: Vec<&str> = exported
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "version",
            "exported_at",
            "patterns",
            "gotchas",
            "preferences",
            "libraries"
        ]
    );
    let target_home = tempfile::tempdir().unwrap();
    let target = super::GlobalStore::new(target_home.path());
    assert_eq!(
        target.import(exported.as_object().unwrap()).unwrap(),
        (0, 0)
    );
    assert_eq!(target.libraries().unwrap(), libraries);
    assert!(!target.path("library_gotchas.jsonl").exists());
}

#[test]
fn an_import_merges_aliases_and_keeps_existing_mappings_on_conflict() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store
        .save_libraries(&libraries_of(&serde_json::json!({
            "next": ["nextjs"],
            "vue": []
        })))
        .unwrap();
    let data = serde_json::json!({
        "libraries": {
            "next": ["next.js"],
            "nuxt": ["nextjs", "nuxtjs"],
            "vue": ["vuejs"],
            "nextjs": ["next-js"],
            "react": ["vue"],
            "bad name": ["x"]
        }
    });
    assert_eq!(store.import(data.as_object().unwrap()).unwrap(), (0, 0));
    assert_eq!(
        store.libraries().unwrap(),
        libraries_of(&serde_json::json!({
            "next": ["next-js", "next.js", "nextjs"],
            "nuxt": ["nuxtjs"],
            "react": [],
            "vue": ["vuejs"]
        }))
    );
}

#[test]
fn an_import_without_libraries_leaves_libraries_json_alone() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    let legacy = serde_json::json!({
        "version": "1.0",
        "patterns": [],
        "gotchas": [{"id": "got_legacy000001", "library": "zod", "gotcha": "legacy"}],
        "preferences": ""
    });
    assert_eq!(store.import(legacy.as_object().unwrap()).unwrap(), (0, 1));
    assert!(!store.path("libraries.json").exists());
    store
        .save_libraries(&libraries_of(&serde_json::json!({"next": ["nextjs"]})))
        .unwrap();
    let before = fs::read_to_string(store.path("libraries.json")).unwrap();
    for data in [
        legacy.clone(),
        serde_json::json!({"libraries": {"next": ["nextjs"]}}),
        serde_json::json!({"libraries": ["not", "an", "object"]}),
    ] {
        store.import(data.as_object().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(store.path("libraries.json")).unwrap(),
            before
        );
    }
}
