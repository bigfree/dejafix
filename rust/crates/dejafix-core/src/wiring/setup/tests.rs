use std::collections::BTreeMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{
    MigrateEvent, MigrateOptions, ProjectState, UpdateStatus, migrate, set_auto_check,
    update_status,
};
use crate::adapters::fs::ClientPaths;
use crate::core::migrate::MigrateStep;
use crate::testing::fixed_clock;
use crate::wiring::types::{Settings, WiringError};

const OLD_HOOK: &str = "#!/bin/sh\n\
echo user hook\n\
# >>> projectmem auto-capture >>>\n\
pjm auto-capture\n\
# <<< projectmem auto-capture <<<\n";
const OLD_BRIDGE: &str = "# CLAUDE.md\n\nMy own rules.\n\n\
<!-- >>> projectmem bridge >>> -->\n\
## projectmem (MANDATORY)\nEdit .projectmem/summary.md never.\n\
<!-- <<< projectmem bridge <<< -->\n";
const DJF: &str = "/opt/djf/bin/djf";

struct Machine {
    _dir: tempfile::TempDir,
    base: PathBuf,
    home: PathBuf,
    old_home: PathBuf,
    root: PathBuf,
}

impl Machine {
    fn new_home(&self) -> PathBuf {
        self.home.join(".dejafix")
    }

    fn settings(&self) -> Settings {
        Settings {
            dejafix_home: self.new_home(),
            redact: true,
            clock: fixed_clock(),
        }
    }

    fn options(&self, apply: bool, extra: &[&str]) -> MigrateOptions {
        MigrateOptions {
            apply,
            old_home: self.old_home.clone(),
            cwd: self.base.clone(),
            extra_roots: extra.iter().map(|root| (*root).to_owned()).collect(),
            clients: ClientPaths {
                home: self.home.clone(),
                xdg_config_home: None,
                appdata: None,
                local_appdata: None,
            },
            legacy_env: Vec::new(),
            djf: DJF.to_owned(),
        }
    }

    fn run(&self, options: &MigrateOptions) -> Result<Vec<MigrateEvent>, WiringError> {
        let mut events = Vec::new();
        migrate(&self.settings(), options, &mut |event| {
            events.push(event);
            ControlFlow::Continue(())
        })?;
        Ok(events)
    }

    fn apply(&self) -> Vec<MigrateEvent> {
        self.run(&self.options(true, &[])).unwrap()
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn bare() -> Machine {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    Machine {
        old_home: home.join(".projectmem"),
        root: base.join("repo"),
        home,
        base,
        _dir: dir,
    }
}

fn legacy() -> Machine {
    let machine = bare();
    let old_home = &machine.old_home;
    write(&old_home.join("global/library_gotchas.jsonl"), "{}\n");
    let root = &machine.root;
    let memory = root.join(".projectmem");
    fs::create_dir_all(memory.join("issues")).unwrap();
    write(
        &memory.join("events.jsonl"),
        "{\"location\": \".projectmem/summary.md\", \"summary\": \"keep me\"}\n",
    );
    write(
        &memory.join("AI_INSTRUCTIONS.md"),
        "# projectmem AI Instructions\n\n<!-- projectmem-instructions: 2 -->\n",
    );
    write(
        &root.join(".gitignore"),
        "node_modules/\n.projectmem/events.jsonl\n",
    );
    write(&root.join("CLAUDE.md"), OLD_BRIDGE);
    write(&root.join(".git/hooks/pre-commit"), OLD_HOOK);
    write(
        &old_home.join("projects.json"),
        &json!([root.display().to_string()]).to_string(),
    );
    machine
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn states(events: &[MigrateEvent]) -> Vec<&ProjectState> {
    events
        .iter()
        .filter_map(|event| match event {
            MigrateEvent::Project { state, .. } => Some(state),
            _ => None,
        })
        .collect()
}

fn skipped(events: &[MigrateEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            MigrateEvent::HomeSkip { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn dry_run_changes_nothing() {
    let machine = legacy();
    let before = tree(&machine.base);
    let events = machine.run(&machine.options(false, &[])).unwrap();
    assert_eq!(tree(&machine.base), before);
    assert_eq!(events.first(), Some(&MigrateEvent::Banner { apply: false }));
    assert_eq!(
        states(&events),
        [&ProjectState::Steps(vec![
            MigrateStep::Rename,
            MigrateStep::Paths(1),
            MigrateStep::Instructions,
            MigrateStep::Gitignore,
            MigrateStep::Bridges(vec!["CLAUDE.md"]),
            MigrateStep::Hooks(vec!["pre-commit"]),
        ])]
    );
    assert!(!events.contains(&MigrateEvent::Done));
}

#[test]
fn apply_moves_memory_home_and_hooks() {
    let machine = legacy();
    let events = machine.apply();
    assert_eq!(events.last(), Some(&MigrateEvent::Done));
    let new_home = machine.new_home();
    assert!(!machine.old_home.exists());
    assert_eq!(
        serde_json::from_str::<Value>(&read(&new_home.join("projects.json"))).unwrap(),
        json!([machine.root.display().to_string()])
    );
    assert!(new_home.join("global/library_gotchas.jsonl").exists());
    let memory = machine.root.join(".dejafix");
    assert!(!machine.root.join(".projectmem").exists());
    assert_eq!(
        serde_json::from_str::<Value>(&read(&memory.join("events.jsonl"))).unwrap(),
        json!({"location": ".dejafix/summary.md", "summary": "keep me"})
    );
    assert!(read(&memory.join("AI_INSTRUCTIONS.md")).contains("dejafix-instructions"));
    assert!(read(&memory.join("AI_INSTRUCTIONS.md.bak")).contains("projectmem-instructions"));
    assert!(read(&machine.root.join(".gitignore")).contains(".dejafix/events.jsonl"));
    let claude = read(&machine.root.join("CLAUDE.md"));
    assert!(claude.contains("My own rules."));
    assert!(!claude.contains("projectmem"));
    assert_eq!(claude.matches("<!-- >>> dejafix bridge >>> -->").count(), 1);
    let hook = read(&machine.root.join(".git/hooks/pre-commit"));
    assert!(hook.contains("echo user hook"));
    assert!(!hook.contains("projectmem auto-capture") && !hook.contains("pjm auto-capture"));
    assert_eq!(hook.matches("# >>> dejafix auto-capture >>>").count(), 1);
    assert!(hook.contains(DJF));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, MigrateEvent::Hooks(_)))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, MigrateEvent::Bridge("AGENTS.md", _)))
    );
}

#[cfg(unix)]
#[test]
fn rewritten_files_keep_their_mode() {
    use std::os::unix::fs::PermissionsExt;
    let machine = legacy();
    let hook = machine.root.join(".git/hooks/pre-commit");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o750)).unwrap();
    let events = machine.root.join(".projectmem/events.jsonl");
    fs::set_permissions(&events, fs::Permissions::from_mode(0o600)).unwrap();
    machine.apply();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&hook) & 0o770, 0o750);
    assert_eq!(mode(&machine.root.join(".dejafix/events.jsonl")), 0o600);
}

#[test]
fn a_second_run_is_a_no_op() {
    let machine = legacy();
    machine.apply();
    let claude = read(&machine.root.join("CLAUDE.md"));
    let events = machine.apply();
    assert_eq!(states(&events), [&ProjectState::Migrated]);
    assert_eq!(read(&machine.root.join("CLAUDE.md")), claude);
}

#[test]
fn existing_dejafix_folder_is_never_overwritten() {
    let machine = legacy();
    fs::create_dir(machine.root.join(".dejafix")).unwrap();
    let events = machine.apply();
    assert!(machine.root.join(".projectmem/events.jsonl").exists());
    assert_eq!(states(&events), [&ProjectState::BothExist]);
}

#[test]
fn project_with_a_running_old_watcher_is_skipped() {
    let machine = legacy();
    write(
        &machine.root.join(".projectmem/watch.pid"),
        &std::process::id().to_string(),
    );
    let events = machine.apply();
    assert!(machine.root.join(".projectmem").is_dir());
    assert_eq!(
        states(&events),
        [&ProjectState::Watcher(std::process::id())]
    );
}

#[test]
fn a_home_stamped_by_a_first_djf_run_is_replaced() {
    let machine = legacy();
    write(
        &machine.old_home.join("projects.meta.json"),
        &json!({"active_project": "repo"}).to_string(),
    );
    let new_home = machine.new_home();
    fs::create_dir_all(new_home.join("global")).unwrap();
    write(
        &new_home.join("projects.meta.json"),
        &json!({"seen_version": "0.4.0"}).to_string(),
    );
    machine.apply();
    let meta: Value = serde_json::from_str(&read(&new_home.join("projects.meta.json"))).unwrap();
    assert_eq!(meta["active_project"], "repo");
    assert!(meta.get("seen_version").is_none());
    assert!(new_home.join("global/library_gotchas.jsonl").exists());
    assert!(!machine.old_home.exists());
}

#[test]
fn home_entry_with_data_in_both_places_is_kept() {
    let machine = legacy();
    let new_home = machine.new_home();
    write(&new_home.join("projects.json"), "[]");
    write(&new_home.join("global/library_gotchas.jsonl"), "new\n");
    let events = machine.apply();
    assert_eq!(
        read(&new_home.join("global/library_gotchas.jsonl")),
        "new\n"
    );
    assert!(
        machine
            .old_home
            .join("global/library_gotchas.jsonl")
            .exists()
    );
    assert_eq!(skipped(&events), ["global", "projects.json"]);
}

#[test]
fn unregistered_project_given_by_path() {
    let machine = bare();
    let root = machine.base.join("loose");
    fs::create_dir_all(root.join(".projectmem")).unwrap();
    let events = machine
        .run(&machine.options(true, &[&root.display().to_string()]))
        .unwrap();
    assert!(root.join(".dejafix").is_dir());
    assert!(events.contains(&MigrateEvent::HomeMissing(machine.old_home.clone())));
}

#[test]
fn stale_client_configs_are_reported() {
    let machine = bare();
    let config = machine.home.join(".codex/config.toml");
    write(&config, "[mcp_servers.projectmem]\ncommand = \"pjm-mcp\"\n");
    write(
        &machine.home.join(".claude/settings.json"),
        "{\"pjm-edit-hook\": 1}",
    );
    let mut options = machine.options(false, &[]);
    options.legacy_env = vec!["PROJECTMEM_ROOT".to_owned()];
    let events = machine.run(&options).unwrap();
    assert_eq!(
        events.last(),
        Some(&MigrateEvent::Leftovers {
            configs: vec![
                ("Codex", config),
                (
                    "Claude Code hooks",
                    machine.home.join(".claude/settings.json")
                ),
            ],
            env: vec!["PROJECTMEM_ROOT".to_owned()],
        })
    );
}

#[test]
fn dry_run_does_not_repair_the_legacy_registry() {
    let machine = legacy();
    machine.run(&machine.options(false, &[])).unwrap();
    assert!(!machine.old_home.join("projects.meta.json").exists());
}

#[test]
fn a_run_that_stopped_after_the_rename_is_finished() {
    let machine = legacy();
    fs::rename(
        machine.root.join(".projectmem"),
        machine.root.join(".dejafix"),
    )
    .unwrap();
    machine.apply();
    let memory = machine.root.join(".dejafix");
    assert!(read(&memory.join("events.jsonl")).contains("\".dejafix/summary.md\""));
    assert!(read(&memory.join("AI_INSTRUCTIONS.md")).contains("dejafix-instructions"));
    assert!(!read(&machine.root.join("CLAUDE.md")).contains("projectmem bridge"));
    assert!(!read(&machine.root.join(".git/hooks/pre-commit")).contains("projectmem auto-capture"));
    assert!(!read(&machine.root.join(".gitignore")).contains(".projectmem"));
}

#[test]
fn a_bridge_already_added_by_djf_init_is_not_duplicated() {
    let machine = legacy();
    write(
        &machine.root.join("CLAUDE.md"),
        &format!(
            "{OLD_BRIDGE}\n<!-- >>> dejafix bridge >>> -->\nold\n<!-- <<< dejafix bridge <<< -->\n"
        ),
    );
    machine.apply();
    let claude = read(&machine.root.join("CLAUDE.md"));
    assert!(claude.contains("My own rules."));
    assert!(!claude.contains("projectmem bridge"));
    assert_eq!(claude.matches("<!-- >>> dejafix bridge >>> -->").count(), 1);
}

#[test]
fn a_relative_path_gets_the_folder_name() {
    let machine = bare();
    let root = machine.base.join("loose");
    fs::create_dir_all(root.join(".projectmem")).unwrap();
    let mut options = machine.options(false, &["."]);
    options.cwd = root.clone();
    let events = machine.run(&options).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        MigrateEvent::Project { label, .. } if *label == format!("Project loose ({})", root.display())
    )));
}

#[cfg(unix)]
#[test]
fn a_symlinked_memory_folder_is_left_alone() {
    let machine = bare();
    let elsewhere = machine.base.join("elsewhere");
    write(
        &elsewhere.join("summary.md"),
        "see .projectmem/summary.md\n",
    );
    fs::create_dir_all(&machine.root).unwrap();
    std::os::unix::fs::symlink(&elsewhere, machine.root.join(".projectmem")).unwrap();
    let root = machine.root.display().to_string();
    let events = machine.run(&machine.options(true, &[&root])).unwrap();
    assert!(
        fs::symlink_metadata(machine.root.join(".projectmem"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        read(&elsewhere.join("summary.md")),
        "see .projectmem/summary.md\n"
    );
    assert_eq!(states(&events), [&ProjectState::Symlink]);
}

#[test]
fn unregistered_home_data_is_not_deleted() {
    let machine = legacy();
    let new_global = machine.new_home().join("global");
    write(&new_global.join("library_gotchas.jsonl"), "new\n");
    let events = machine.apply();
    assert_eq!(read(&new_global.join("library_gotchas.jsonl")), "new\n");
    assert_eq!(skipped(&events), ["global"]);
}

#[test]
fn a_folder_named_projectmem_is_not_a_stale_config() {
    let machine = bare();
    write(
        &machine.home.join(".codex/config.toml"),
        "[projects.\"/src/projectmem\"]\ntrust_level = \"trusted\"\n",
    );
    let events = machine.run(&machine.options(false, &[])).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, MigrateEvent::Leftovers { .. }))
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_new_home_is_never_followed_or_removed() {
    let machine = legacy();
    let target = machine.base.join("elsewhere");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(machine.new_home()).unwrap();
    std::os::unix::fs::symlink(&target, machine.new_home().join("global")).unwrap();
    let events = machine.apply();
    assert_eq!(skipped(&events), ["global"]);
    assert!(target.is_dir());
    assert!(
        machine
            .old_home
            .join("global/library_gotchas.jsonl")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn memory_files_behind_symlinks_are_not_rewritten() {
    let machine = legacy();
    let outside = machine.base.join("outside.md");
    write(&outside, ".projectmem/x\n");
    std::os::unix::fs::symlink(&outside, machine.root.join(".projectmem/linked.md")).unwrap();
    machine.apply();
    assert_eq!(read(&outside), ".projectmem/x\n");
}

#[test]
fn the_same_path_given_twice_is_listed_twice_as_python_does() {
    let machine = bare();
    let root = machine.base.join("loose");
    fs::create_dir_all(root.join(".dejafix")).unwrap();
    let raw = root.display().to_string();
    let events = machine.run(&machine.options(false, &[&raw, &raw])).unwrap();
    assert_eq!(
        states(&events),
        [&ProjectState::Migrated, &ProjectState::Migrated]
    );
}

#[test]
fn a_missing_project_folder_is_skipped() {
    let machine = bare();
    let raw = machine.base.join("gone").display().to_string();
    let events = machine.run(&machine.options(true, &[&raw])).unwrap();
    assert_eq!(states(&events), [&ProjectState::NoLegacy]);
}

fn meta(machine: &Machine) -> Value {
    serde_json::from_str(&read(&machine.new_home().join("projects.meta.json"))).unwrap()
}

#[test]
fn the_update_check_is_off_until_asked() {
    let machine = bare();
    let status = update_status(&machine.settings(), false, "1.0.0", || {
        panic!("no network call")
    });
    assert_eq!(status, UpdateStatus::Offline);
}

#[test]
fn online_reports_a_newer_release() {
    let machine = bare();
    let settings = machine.settings();
    assert_eq!(
        update_status(&settings, true, "1.0.0-rc.1", || Some("99.0.0".to_owned())),
        UpdateStatus::Newer("99.0.0".to_owned())
    );
    let meta = meta(&machine);
    assert_eq!(meta["update_latest"], "99.0.0");
    assert!(meta["update_checked_at"].as_f64().unwrap() > 1.0e9);
    assert_eq!(
        update_status(&settings, true, "1.0.0", || Some("1.0.0rc1".to_owned())),
        UpdateStatus::Latest
    );
}

#[test]
fn auto_check_is_remembered_and_can_be_turned_off() {
    let machine = bare();
    let settings = machine.settings();
    write(
        &machine.new_home().join("projects.meta.json"),
        &json!({"active_project": "repo"}).to_string(),
    );
    set_auto_check(&settings, true).unwrap();
    assert_eq!(meta(&machine)["update_check"], true);
    assert_eq!(meta(&machine)["active_project"], "repo");
    assert_eq!(
        update_status(&settings, false, "0.0.1", || Some("0.0.1".to_owned())),
        UpdateStatus::Latest
    );
    set_auto_check(&settings, false).unwrap();
    assert_eq!(meta(&machine)["update_check"], false);
    assert_eq!(
        update_status(&settings, false, "0.0.1", || panic!("no network call")),
        UpdateStatus::Offline
    );
}

#[test]
fn the_daily_check_reuses_a_recent_answer() {
    let machine = bare();
    let settings = machine.settings();
    let now = settings.clock.now.as_second();
    write(
        &machine.new_home().join("projects.meta.json"),
        &json!({"update_check": true, "update_checked_at": now - 60, "update_latest": "2.0.0"})
            .to_string(),
    );
    assert_eq!(
        update_status(&settings, false, "1.0.0", || panic!("no network call")),
        UpdateStatus::Newer("2.0.0".to_owned())
    );
    write(
        &machine.new_home().join("projects.meta.json"),
        &json!({"update_check": true, "update_checked_at": now - 90_000, "update_latest": "2.0.0"})
            .to_string(),
    );
    assert_eq!(
        update_status(&settings, false, "3.0.0", || None),
        UpdateStatus::Latest
    );
    assert_eq!(
        update_status(&settings, false, "1.0.0", || Some("3.0.0".to_owned())),
        UpdateStatus::Newer("3.0.0".to_owned())
    );
}

#[test]
fn an_unreachable_index_without_a_remembered_answer_says_so() {
    let machine = bare();
    assert_eq!(
        update_status(&machine.settings(), true, "1.0.0", || None),
        UpdateStatus::Unreachable
    );
    assert!(!machine.new_home().join("projects.meta.json").exists());
}

#[cfg(unix)]
#[test]
fn a_missing_registered_root_and_its_path_through_a_symlink_are_one_project() {
    let machine = bare();
    let real = machine.base.join("real");
    fs::create_dir_all(&real).unwrap();
    let link = machine.base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let registered = link.join("gone");
    write(
        &machine.old_home.join("projects.json"),
        &json!([registered.display().to_string()]).to_string(),
    );
    let raw = real.join("gone").display().to_string();
    let events = machine.run(&machine.options(false, &[&raw])).unwrap();
    assert_eq!(states(&events), [&ProjectState::NoLegacy]);
}

#[test]
fn a_stopped_sink_ends_the_run_before_the_next_change() {
    let machine = legacy();
    let mut seen = 0;
    migrate(
        &machine.settings(),
        &machine.options(true, &[]),
        &mut |_| {
            seen += 1;
            if seen == 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .unwrap();
    assert!(machine.old_home.join("global").is_dir());
    assert!(machine.root.join(".projectmem").is_dir());
}

#[cfg(unix)]
#[test]
fn rule_files_linked_from_outside_the_project_are_left_alone() {
    let machine = legacy();
    let outside = machine.base.join("private/CLAUDE.md");
    write(&outside, OLD_BRIDGE);
    fs::remove_file(machine.root.join("CLAUDE.md")).unwrap();
    std::os::unix::fs::symlink(&outside, machine.root.join("CLAUDE.md")).unwrap();
    let ignore = machine.base.join("private/gitignore");
    write(&ignore, ".projectmem/secret\n");
    fs::remove_file(machine.root.join(".gitignore")).unwrap();
    std::os::unix::fs::symlink(&ignore, machine.root.join(".gitignore")).unwrap();
    let events = machine.run(&machine.options(false, &[])).unwrap();
    assert_eq!(
        states(&events),
        [&ProjectState::Steps(vec![
            MigrateStep::Rename,
            MigrateStep::Paths(1),
            MigrateStep::Instructions,
            MigrateStep::Hooks(vec!["pre-commit"]),
        ])]
    );
    machine.apply();
    assert!(
        !fs::read_to_string(machine.root.join(".gitignore"))
            .unwrap()
            .contains("secret")
    );
    assert!(
        fs::symlink_metadata(machine.root.join("CLAUDE.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(read(&ignore), ".projectmem/secret\n");
    assert_eq!(read(&outside), OLD_BRIDGE);
}
