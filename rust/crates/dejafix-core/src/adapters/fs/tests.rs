use std::fs;

use super::resolve::is_network_path;
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
        assert!(text.starts_with(&super::nl("{\n  \"libraries\": [\n    \"alembic\",")));
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
            super::nl("{\"id\": \"got_1\", \"library\": \"react\"}\n")
        );
        fs::write(dir.path().join("global/library_gotchas.jsonl"), "[1]\n").unwrap();
        assert_eq!(store.gotchas(), None);
        assert!(!store.append_gotcha_if_new(|_| Some(entry.clone())).unwrap());
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
fn no_gotcha_file_is_created_when_nothing_is_promoted() {
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    assert!(!store.append_gotcha_if_new(|_| None).unwrap());
    assert!(!store.dir.join("library_gotchas.jsonl").exists());
    let mut entry = serde_json::Map::new();
    entry.insert("library".to_owned(), serde_json::json!("react"));
    assert!(
        store
            .append_gotcha_if_new(|existing| existing.is_empty().then_some(entry))
            .unwrap()
    );
    assert!(store.dir.join("library_gotchas.jsonl").exists());
}

#[cfg(unix)]
#[test]
fn global_removal_rewrites_in_place_so_a_locked_append_is_never_lost() {
    use std::os::unix::fs::MetadataExt;
    let home = tempfile::tempdir().unwrap();
    let store = super::GlobalStore::new(home.path());
    store.ensure().unwrap();
    let path = store.path("library_gotchas.jsonl");
    fs::write(
        &path,
        "{\"gotcha\": \"a\", \"id\": \"got_a\", \"library\": \"x\"}\n{\"gotcha\": \"b\", \"id\": \"got_b\", \"library\": \"x\"}\n",
    )
    .unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    assert!(store.remove("got_a").unwrap());
    assert!(!store.remove("got_zzz").unwrap());
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "{\"gotcha\": \"b\", \"id\": \"got_b\", \"library\": \"x\"}\n"
    );
}
