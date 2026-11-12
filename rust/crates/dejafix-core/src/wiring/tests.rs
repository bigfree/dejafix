use std::fs;
use std::path::Path;

use super::{
    BridgeOutcome, InitOptions, MemoryFile, Project, RegistryOp, RegistryReply, SearchOutput,
    Settings, WiringError, add_plan_idea, init_project, other_projects, read_memory_file, record,
    regenerate, register_project, registry_op, search_project, show, upgrade_notice,
};
use crate::core::event::Outcome;
use crate::core::registry::RegistryError;
use crate::core::scaffold::{INSTRUCTIONS_VERSION, ai_instructions, instructions_version};
use crate::core::search::SearchQuery;
use crate::core::write::{Command, ValidationError, WriteOutcome};
use crate::testing::{FIXTURE_SEED, SeqIds, fixed_clock, write_project};

fn settings(home: &Path) -> Settings {
    Settings {
        dejafix_home: home.to_path_buf(),
        redact: true,
        clock: fixed_clock(),
    }
}

fn empty_project(root: &Path) {
    fs::create_dir_all(root.join(".dejafix/issues")).unwrap();
    fs::write(root.join(".dejafix/config.toml"), "").unwrap();
}

fn note(text: &str) -> Command {
    Command::Note {
        text: text.into(),
        location: None,
    }
}

#[test]
fn a_note_appends_one_line_and_regenerates_the_summary() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let recorded = record(
        &project,
        note("models live in src/models"),
        &mut SeqIds::default(),
    )
    .unwrap();
    assert_eq!(recorded.outcome, WriteOutcome::Noted);
    let log = fs::read_to_string(dir.path().join(".dejafix/events.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 1);
    let summary = show(&project).unwrap();
    assert!(summary.contains("- models live in src/models\n"));
    assert!(summary.contains("_Last updated: 2027-01-15_"));
}

#[test]
fn log_attempt_fix_keep_the_marker_and_issue_file_in_step() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let mut ids = SeqIds::default();
    let logged = record(
        &project,
        Command::Log {
            text: "Login fails".into(),
            location: Some("auth.py:42".into()),
        },
        &mut ids,
    )
    .unwrap();
    assert_eq!(logged.outcome, WriteOutcome::Logged("0001".into()));
    assert_eq!(
        fs::read_to_string(dir.path().join(".dejafix/.current_issue")).unwrap(),
        "0001"
    );
    let attempted = record(
        &project,
        Command::Attempt {
            text: "Clear cookies".into(),
            outcomes: vec![Outcome::Failed],
            location: None,
            issue: None,
            auto_issue: false,
        },
        &mut ids,
    )
    .unwrap();
    assert_eq!(
        attempted.outcome,
        WriteOutcome::Attempted {
            outcome: Outcome::Failed,
            issue: "0001".into()
        }
    );
    record(
        &project,
        Command::Fix {
            text: "Reset session".into(),
            location: None,
            issue: None,
        },
        &mut ids,
    )
    .unwrap();
    assert!(!dir.path().join(".dejafix/.current_issue").exists());
    let issue = fs::read_to_string(dir.path().join(".dejafix/issues/0001-login-fails.md")).unwrap();
    assert_eq!(issue.lines().count(), 5);
    assert!(
        show(&project)
            .unwrap()
            .contains("- [DONE] #0001 Login fails [auth.py:42] (fixed)")
    );
}

#[test]
fn a_stale_marker_is_cleared_even_when_the_command_fails() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    fs::write(dir.path().join(".dejafix/.current_issue"), "0042").unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let error = record(
        &project,
        Command::Fix {
            text: "x".into(),
            location: None,
            issue: None,
        },
        &mut SeqIds::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WiringError::Validation(ValidationError::NoOpenIssue)
    ));
    assert!(!dir.path().join(".dejafix/.current_issue").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".dejafix/events.jsonl")).unwrap(),
        ""
    );
}

#[test]
fn appending_an_event_stays_inside_an_isolated_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"react": "18", "zod": "3"}}"#,
    )
    .unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    record(
        &project,
        note("gotcha: react strict mode mounts twice"),
        &mut SeqIds::default(),
    )
    .unwrap();
    record(
        &project,
        note("gotcha: react strict mode mounts twice in dev"),
        &mut SeqIds::default(),
    )
    .unwrap();
    let gotchas = fs::read_to_string(home.path().join("global/library_gotchas.jsonl")).unwrap();
    assert_eq!(gotchas.lines().count(), 1);
    assert!(gotchas.contains("\"library\": \"react\""));
    assert!(gotchas.contains("\"discovered\": \"2027-01-15T08:00:00Z\""));
    assert!(
        fs::read_to_string(home.path().join("global/.promotable.json"))
            .unwrap()
            .contains("\"zod\"")
    );
}

#[test]
fn regenerate_writes_each_issue_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    write_project(dir.path(), 300, FIXTURE_SEED).unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let path = regenerate(&project).unwrap();
    assert_eq!(path, dir.path().join(".dejafix/summary.md"));
    let issues = dir.path().join(".dejafix/issues");
    let first: Vec<_> = fs::read_dir(&issues)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().modified().unwrap())
        .collect();
    assert_ne!(first.len(), 0);
    std::thread::sleep(std::time::Duration::from_millis(20));
    regenerate(&project).unwrap();
    let second: Vec<_> = fs::read_dir(&issues)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().modified().unwrap())
        .collect();
    assert_eq!(first, second);
}

#[test]
fn a_bad_supersede_reference_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    write_project(dir.path(), 50, FIXTURE_SEED).unwrap();
    let before = fs::read(dir.path().join(".dejafix/events.jsonl")).unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let command = Command::Decision {
        text: "x".into(),
        location: None,
        supersedes: Some("evt_nope".into()),
    };
    assert!(record(&project, command, &mut SeqIds::default()).is_err());
    assert_eq!(
        fs::read(dir.path().join(".dejafix/events.jsonl")).unwrap(),
        before
    );
}

#[test]
fn search_reports_matches_or_the_python_message() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    record(&project, note("startup is slow"), &mut SeqIds::default()).unwrap();
    let query = SearchQuery {
        text: "STARTUP",
        regex: false,
        failed_only: false,
    };
    assert_eq!(
        search_project(&project, &query).unwrap(),
        SearchOutput::Matches(vec![
            "2027-01-15T08:00:00Z [evt_00000000000000000001] note: startup is slow".into()
        ])
    );
    let query = SearchQuery {
        text: "a|b",
        regex: false,
        failed_only: false,
    };
    assert!(
        matches!(search_project(&project, &query).unwrap(), SearchOutput::Nothing(message) if message.contains("--regex"))
    );
}

#[test]
fn a_failed_regeneration_still_reports_the_recorded_event() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    fs::create_dir_all(dir.path().join(".dejafix/summary.md/blocked")).unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let recorded = record(&project, note("still recorded"), &mut SeqIds::default()).unwrap();
    assert_eq!(recorded.outcome, WriteOutcome::Noted);
    assert!(matches!(recorded.incomplete, Some(WiringError::Store(_))));
    let log = fs::read_to_string(dir.path().join(".dejafix/events.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_symlinked_issues_directory_is_never_written_through() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    empty_project(dir.path());
    fs::remove_dir(dir.path().join(".dejafix/issues")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".dejafix/issues")).unwrap();
    let project = Project::open(dir.path(), settings(home.path())).unwrap();
    let recorded = record(
        &project,
        Command::Log {
            text: "review".into(),
            location: None,
        },
        &mut SeqIds::default(),
    )
    .unwrap();
    assert!(recorded.incomplete.is_none());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(show(&project).unwrap().contains("#0001 review"));
}

fn options() -> InitOptions {
    InitOptions {
        claude_md: true,
        stack_detect: true,
        global: true,
        global_tags: None,
    }
}

struct Initialised {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    home: std::path::PathBuf,
}

fn initialised(name: &str) -> Initialised {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let root = base.join(name);
    fs::create_dir(&root).unwrap();
    Initialised {
        _dir: dir,
        root,
        home: base.join("pm"),
    }
}

#[test]
fn init_creates_the_memory_files_and_registers_the_project() {
    let project = initialised("checkout-api");
    let report = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    let mem = project.root.join(".dejafix");
    assert_eq!(report.mem_dir, mem);
    for file in [
        "config.toml",
        "summary.md",
        "AI_INSTRUCTIONS.md",
        "PROJECT_MAP.md",
        "plan.md",
        "events.jsonl",
    ] {
        assert!(mem.join(file).is_file(), "{file}");
    }
    assert!(mem.join("issues").is_dir());
    assert!(!report.refreshed_instructions);
    assert_eq!(
        report.bridges,
        [
            ("CLAUDE.md", BridgeOutcome::Created),
            ("AGENTS.md", BridgeOutcome::Created)
        ]
    );
    assert_eq!(report.project_map, None);
    assert_eq!(report.inherited, None);
    assert!(
        fs::read_to_string(project.root.join("CLAUDE.md"))
            .unwrap()
            .contains("**checkout-api**")
    );
    let gitignore = fs::read_to_string(project.root.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".dejafix/events.jsonl") && !gitignore.contains("plan.md"));
    let list = fs::read_to_string(project.home.join("projects.json")).unwrap();
    assert!(list.contains(&*project.root.to_string_lossy()));
    let again = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert_eq!(again.bridges.len(), 0);
    assert_eq!(
        fs::read_to_string(project.home.join("projects.json")).unwrap(),
        list
    );
    assert_eq!(other_projects(&settings(&project.home), &project.root), 0);
}

#[test]
fn init_names_the_bridge_after_the_registered_alias() {
    let project = initialised("Shop_Front");
    init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    let claude = fs::read_to_string(project.root.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("**shop-front**"));
    registry_op(
        &settings(&project.home),
        &project.root,
        None,
        RegistryOp::Alias {
            identifier: "shop-front".into(),
            alias: "shop".into(),
        },
    )
    .unwrap();
    let report = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert_eq!(
        report.bridges,
        [
            ("CLAUDE.md", BridgeOutcome::Refreshed),
            ("AGENTS.md", BridgeOutcome::Refreshed)
        ]
    );
    assert!(
        fs::read_to_string(project.root.join("AGENTS.md"))
            .unwrap()
            .contains("**shop**")
    );
}

#[test]
fn an_unregistrable_home_falls_back_to_the_folder_slug() {
    let project = initialised("My App");
    fs::write(&project.home, "not a directory").unwrap();
    let mut no_claude = options();
    no_claude.global = false;
    init_project(&project.root, None, &settings(&project.home), &no_claude).unwrap();
    assert!(
        fs::read_to_string(project.root.join("CLAUDE.md"))
            .unwrap()
            .contains("**my-app**")
    );
}

#[test]
fn init_refreshes_an_older_marker_and_keeps_the_global_block() {
    let project = initialised("app");
    let mem = project.root.join(".dejafix");
    fs::create_dir_all(&mem).unwrap();
    let stale = "<!-- dejafix-instructions: 2 -->\n## Global Memory — Inherited Knowledge\n\n- kept\n\n## Rules summary\nold\n";
    fs::write(mem.join("AI_INSTRUCTIONS.md"), stale).unwrap();
    let mut no_global = options();
    no_global.global = false;
    let report = init_project(&project.root, None, &settings(&project.home), &no_global).unwrap();
    assert!(report.refreshed_instructions);
    let text = fs::read_to_string(mem.join("AI_INSTRUCTIONS.md")).unwrap();
    assert_eq!(instructions_version(&text), INSTRUCTIONS_VERSION);
    assert!(text.contains("- kept\n"));
    assert_eq!(
        fs::read_to_string(mem.join("AI_INSTRUCTIONS.md.bak")).unwrap(),
        stale
    );
    assert!(
        fs::read_to_string(project.root.join(".gitignore"))
            .unwrap()
            .contains(".dejafix/AI_INSTRUCTIONS.md.bak\n")
    );
    fs::write(mem.join("AI_INSTRUCTIONS.md"), "old, no marker\n").unwrap();
    init_project(&project.root, None, &settings(&project.home), &no_global).unwrap();
    assert_eq!(
        fs::read_to_string(mem.join("AI_INSTRUCTIONS.md")).unwrap(),
        ai_instructions()
    );
}

#[test]
fn a_current_instructions_file_is_never_rewritten() {
    let project = initialised("app");
    init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    let path = project.root.join(".dejafix/AI_INSTRUCTIONS.md");
    let edited = format!("{}\n## House rules\n\nAlways squash.\n", ai_instructions());
    fs::write(&path, &edited).unwrap();
    let report = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert!(!report.refreshed_instructions);
    assert_eq!(fs::read_to_string(&path).unwrap(), edited);
    assert!(!path.with_file_name("AI_INSTRUCTIONS.md.bak").exists());
}

fn with_global_gotcha(home: &Path) {
    fs::create_dir_all(home.join("global")).unwrap();
    fs::write(
        home.join("global/library_gotchas.jsonl"),
        "{\"gotcha\": \"strict mode mounts twice\", \"library\": \"react\", \"source_project\": \"web\"}\n",
    )
    .unwrap();
    fs::write(
        home.join("global/patterns.jsonl"),
        "{\"pattern\": \"python only\", \"tags\": [\"python\"]}\n",
    )
    .unwrap();
}

#[test]
fn global_memory_and_the_stack_map_are_inherited() {
    let project = initialised("shop");
    fs::write(
        project.root.join("package.json"),
        r#"{"description": "Shop", "dependencies": {"react": "18"}}"#,
    )
    .unwrap();
    fs::create_dir(project.root.join("src")).unwrap();
    with_global_gotcha(&project.home);
    let report = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert_eq!(report.project_map.as_deref(), Some(" (react)"));
    let inherited = report.inherited.unwrap();
    assert_eq!((inherited.gotchas, inherited.patterns), (1, 0));
    assert_eq!(inherited.tags, ["javascript", "react"]);
    let instructions =
        fs::read_to_string(project.root.join(".dejafix/AI_INSTRUCTIONS.md")).unwrap();
    assert!(instructions.contains("- **react**: strict mode mounts twice (from web)\n"));
    let map = fs::read_to_string(project.root.join(".dejafix/PROJECT_MAP.md")).unwrap();
    assert!(
        map.contains("## Project purpose\nShop\n")
            && map.contains("- `src/` — package or application code")
    );
    assert!(project.home.join("global/.promotable.json").is_file());
    let again = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert_eq!(again.project_map, None);
    let twice = fs::read_to_string(project.root.join(".dejafix/AI_INSTRUCTIONS.md")).unwrap();
    assert_eq!(twice, instructions);
    let mut python = options();
    python.global_tags = Some(" python , x".into());
    let filtered = init_project(&project.root, None, &settings(&project.home), &python).unwrap();
    assert_eq!(
        filtered.inherited.map(|inherited| inherited.patterns),
        Some(1)
    );
}

#[test]
fn nothing_is_inherited_without_global_files_or_a_stack() {
    let project = initialised("plain");
    with_global_gotcha(&project.home);
    let report = init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert_eq!(report.inherited, None);
    assert_eq!(report.project_map, None);
    let other = initialised("other");
    fs::write(other.root.join("Cargo.toml"), "[package]\n").unwrap();
    let report = init_project(&other.root, None, &settings(&other.home), &options()).unwrap();
    assert_eq!(report.inherited, None);
    assert!(other.home.join("global").is_dir());
}

#[test]
fn plan_ideas_and_memory_files_are_read_from_the_memory_directory() {
    let project = initialised("app");
    init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    let events = fs::read(project.root.join(".dejafix/events.jsonl")).unwrap();
    let sub = project.root.join("src");
    fs::create_dir(&sub).unwrap();
    let path = add_plan_idea(&sub, "ship the plan").unwrap();
    assert_eq!(path, project.root.join(".dejafix/plan.md"));
    let (_, plan) = read_memory_file(&project.root, MemoryFile::Plan).unwrap();
    assert!(plan.contains("## Ideas\n- ship the plan\n"));
    assert_eq!(
        fs::read(project.root.join(".dejafix/events.jsonl")).unwrap(),
        events
    );
    let (_, map) = read_memory_file(&project.root, MemoryFile::ProjectMap).unwrap();
    assert!(map.contains("Status: not created yet"));
    fs::remove_file(project.root.join(".dejafix/AI_INSTRUCTIONS.md")).unwrap();
    assert!(matches!(
        read_memory_file(&project.root, MemoryFile::Instructions),
        Err(WiringError::Store(
            crate::adapters::fs::StoreError::Read { .. }
        ))
    ));
    let elsewhere = initialised("none");
    assert!(matches!(
        add_plan_idea(&elsewhere.root, "x"),
        Err(WiringError::Store(
            crate::adapters::fs::StoreError::NotFound { .. }
        ))
    ));
}

#[test]
fn registry_operations_round_trip() {
    let alpha = initialised("alpha");
    let settings = settings(&alpha.home);
    init_project(&alpha.root, None, &settings, &options()).unwrap();
    let beta = alpha.root.with_file_name("beta");
    fs::create_dir_all(beta.join(".dejafix")).unwrap();
    let record = register_project(&settings, &beta, None, ".", Some("b")).unwrap();
    assert_eq!((record.id.as_str(), record.name()), ("beta", "b"));
    assert_eq!(other_projects(&settings, &alpha.root), 1);
    let plain = alpha.root.with_file_name("plain");
    fs::create_dir(&plain).unwrap();
    assert!(matches!(
        register_project(&settings, &plain, None, ".", None),
        Err(WiringError::Registry(RegistryError::NoMemory { .. }))
    ));
    let op = |op| registry_op(&settings, &alpha.root, None, op).unwrap();
    assert!(
        matches!(op(RegistryOp::Use(Some("B".into()))), RegistryReply::Activated(Some(ref record)) if record.id == "beta")
    );
    assert!(
        matches!(op(RegistryOp::Tag { identifier: ".".into(), tag: "Web".into(), remove: false }), RegistryReply::Tagged(ref record) if record.tags == ["web"])
    );
    assert!(
        matches!(op(RegistryOp::Tag { identifier: "alpha".into(), tag: "web".into(), remove: false }), RegistryReply::Tagged(ref record) if record.tags == ["web"])
    );
    fs::remove_dir_all(beta.join(".dejafix")).unwrap();
    let RegistryReply::Listed { active, projects } = op(RegistryOp::List) else {
        panic!("not listed");
    };
    assert_eq!(active.as_deref(), Some("beta"));
    assert_eq!(
        projects
            .iter()
            .map(|project| project.has_memory)
            .collect::<Vec<_>>(),
        [true, false]
    );
    assert!(
        matches!(op(RegistryOp::Remove("b".into())), RegistryReply::Removed(ref record) if record.id == "beta")
    );
    assert!(matches!(
        op(RegistryOp::Use(None)),
        RegistryReply::Activated(None)
    ));
    assert!(
        matches!(op(RegistryOp::Tag { identifier: "alpha".into(), tag: "web".into(), remove: true }), RegistryReply::Tagged(ref record) if record.tags.is_empty())
    );
    assert!(matches!(
        registry_op(
            &settings,
            &alpha.root,
            None,
            RegistryOp::Remove("beta".into())
        ),
        Err(WiringError::Registry(RegistryError::NotFound(_)))
    ));
}

#[test]
fn the_upgrade_notice_fires_once_and_never_on_a_first_run() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    assert_eq!(upgrade_notice(&settings, "1.0.0"), None);
    assert_eq!(upgrade_notice(&settings, "1.0.0"), None);
    let notice = upgrade_notice(&settings, "1.0.1").unwrap();
    assert_eq!(notice.version, "1.0.1");
    assert_eq!(upgrade_notice(&settings, "1.0.1"), None);
    fs::write(
        dir.path().join("projects.meta.json"),
        r#"{"last_version": null}"#,
    )
    .unwrap();
    assert_eq!(upgrade_notice(&settings, "1.0.1"), None);
    fs::write(
        dir.path().join("projects.meta.json"),
        r#"{"last_version": 3}"#,
    )
    .unwrap();
    assert!(upgrade_notice(&settings, "1.0.1").is_some());
}

#[cfg(unix)]
#[test]
fn an_unsaved_registration_does_not_name_the_bridge() {
    use std::os::unix::fs::PermissionsExt;

    let first = initialised("web");
    init_project(&first.root, None, &settings(&first.home), &options()).unwrap();
    let second = first.root.parent().unwrap().join("other/web");
    fs::create_dir_all(&second).unwrap();
    fs::set_permissions(&first.home, fs::Permissions::from_mode(0o500)).unwrap();
    let report = init_project(&second, None, &settings(&first.home), &options());
    fs::set_permissions(&first.home, fs::Permissions::from_mode(0o700)).unwrap();
    report.unwrap();
    let claude = fs::read_to_string(second.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("**web**"), "{claude}");
    assert!(!claude.contains("web-2"));
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_project_is_never_copied_into_it() {
    use std::os::unix::fs::symlink;

    let project = initialised("app");
    let secret = project.root.with_file_name("secret.txt");
    fs::write(&secret, "TOKEN=hunter2\n").unwrap();
    symlink(&secret, project.root.join("CLAUDE.md")).unwrap();
    symlink(&secret, project.root.join(".gitignore")).unwrap();
    fs::write(project.root.join("NOTES.md"), "# Notes\n").unwrap();
    symlink(
        project.root.join("NOTES.md"),
        project.root.join("AGENTS.md"),
    )
    .unwrap();
    init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    for file in ["CLAUDE.md", ".gitignore"] {
        let text = fs::read_to_string(project.root.join(file)).unwrap();
        assert!(!text.contains("hunter2"), "{file}: {text}");
    }
    assert!(
        fs::read_to_string(project.root.join("AGENTS.md"))
            .unwrap()
            .starts_with("# Notes\n")
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "TOKEN=hunter2\n");
    let plan = project.root.join(".dejafix/plan.md");
    fs::remove_file(&plan).unwrap();
    symlink(&secret, &plan).unwrap();
    add_plan_idea(&project.root, "x").unwrap();
    assert!(!fs::read_to_string(&plan).unwrap().contains("hunter2"));
    let instructions = project.root.join(".dejafix/AI_INSTRUCTIONS.md");
    fs::remove_file(&instructions).unwrap();
    symlink(&secret, &instructions).unwrap();
    with_global_gotcha(&project.home);
    fs::write(
        project.root.join("package.json"),
        r#"{"dependencies": {"react": "18"}}"#,
    )
    .unwrap();
    init_project(&project.root, None, &settings(&project.home), &options()).unwrap();
    assert!(
        !project
            .root
            .join(".dejafix/AI_INSTRUCTIONS.md.bak")
            .exists()
    );
    assert_eq!(fs::read_to_string(&secret).unwrap(), "TOKEN=hunter2\n");
}
