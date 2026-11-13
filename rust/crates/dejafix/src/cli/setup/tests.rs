use std::fs;
use std::path::{Path, PathBuf};

use dejafix_core::{
    BridgeOutcome, ClientPaths, Clock, HookKind, HooksReport, MigrateOptions, Settings,
};

use super::migrate::write_event;
use super::{DoctorRun, run_doctor, run_migrate};
use crate::cli::types::Outcome;
use crate::config::Config;
use dejafix_core::{MigrateEvent, MigrateStep, ProjectState};

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Machine {
    _dir: tempfile::TempDir,
    base: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(dir.path()).unwrap();
        fs::create_dir_all(base.join("home")).unwrap();
        Self { _dir: dir, base }
    }

    fn home(&self) -> PathBuf {
        self.base.join("home")
    }

    fn config(&self) -> Config {
        Config {
            cwd: self.base.clone(),
            home: Some(self.home()),
            dejafix_home: Some(self.home().join(".dejafix")),
            xdg_config_home: None,
            appdata: None,
            local_appdata: None,
            redact: true,
            color: false,
            zone: jiff::tz::TimeZone::UTC,
            path_var: None,
            exe_dir: None,
            legacy_env: Vec::new(),
        }
    }

    fn settings(&self) -> Settings {
        Settings {
            dejafix_home: self.home().join(".dejafix"),
            redact: true,
            clock: Clock {
                now: jiff::Timestamp::now(),
                zone: jiff::tz::TimeZone::UTC,
            },
        }
    }

    fn paths(&self) -> ClientPaths {
        ClientPaths {
            home: self.home(),
            xdg_config_home: None,
            appdata: None,
            local_appdata: None,
        }
    }

    fn doctor(&self, online: bool, auto: Option<bool>, latest: Option<&str>) -> String {
        let mut out = Vec::new();
        let run = DoctorRun {
            fix: false,
            depth: 1,
            roots: vec![self.base.join("empty").display().to_string()],
            online,
            auto,
        };
        let outcome = run_doctor(
            run,
            &self.paths(),
            &self.config(),
            &self.settings(),
            &mut out,
            || latest.map(str::to_owned),
        )
        .unwrap();
        assert!(matches!(outcome, Outcome::Done));
        String::from_utf8(out).unwrap()
    }

    fn meta(&self) -> serde_json::Value {
        let text = fs::read_to_string(self.home().join(".dejafix/projects.meta.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn online_flag_reports_a_newer_release() {
    let machine = Machine::new();
    let out = machine.doctor(true, None, Some("99.0.0"));
    assert!(out.contains(&format!(
        "\n  ⚠ 99.0.0 is available (you have {VERSION})\n      pip install -U dejafix\n"
    )));
}

#[test]
fn online_without_an_answer_says_pypi_was_unreachable() {
    let machine = Machine::new();
    let out = machine.doctor(true, None, None);
    assert!(out.contains(&format!(
        "\n  ✓ Running {VERSION}\n      Could not reach PyPI — nothing else was affected.\n"
    )));
}

#[test]
fn online_on_the_newest_release_says_so() {
    let machine = Machine::new();
    let out = machine.doctor(true, None, Some(VERSION));
    assert!(out.contains(&format!("\n  ✓ Running {VERSION} — the latest\n")));
}

#[test]
fn auto_check_is_remembered_and_can_be_turned_off() {
    let machine = Machine::new();
    let on = machine.doctor(false, Some(true), Some("0.0.1"));
    assert!(on.starts_with("✓ Automatic update checks on (once a day)\n\n"));
    assert_eq!(machine.meta()["update_check"], true);
    let off = machine.doctor(false, Some(false), Some("99.0.0"));
    assert!(off.starts_with("✓ Automatic update checks off\n\n"));
    assert_eq!(machine.meta()["update_check"], false);
    assert!(off.contains("djf doctor --online"));
}

fn rendered(events: &[MigrateEvent]) -> String {
    let mut out = Vec::new();
    let mut err = Vec::new();
    for event in events {
        write_event(event, &mut out, &mut err).unwrap();
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn every_migrate_line_reads_as_python_prints_it() {
    let label = "Project repo (/work/repo)".to_owned();
    let project = |state| MigrateEvent::Project {
        label: label.clone(),
        state,
    };
    let text = rendered(&[
        MigrateEvent::Banner { apply: false },
        MigrateEvent::Home {
            from: PathBuf::from("/h/.projectmem"),
            to: PathBuf::from("/h/.dejafix"),
        },
        MigrateEvent::HomeSkip {
            name: "global".to_owned(),
            dest: PathBuf::from("/h/.dejafix/global"),
        },
        MigrateEvent::HomeMove("projects.json".to_owned()),
        project(ProjectState::Symlink),
        project(ProjectState::BothExist),
        project(ProjectState::NoLegacy),
        project(ProjectState::Watcher(42)),
        project(ProjectState::Migrated),
        project(ProjectState::Steps(vec![
            MigrateStep::Rename,
            MigrateStep::Hooks(vec!["pre-commit"]),
        ])),
        MigrateEvent::Bridge("AGENTS.md", BridgeOutcome::Created),
        MigrateEvent::Hooks(HooksReport {
            refreshed: vec![HookKind::PreCommit],
            ..HooksReport::default()
        }),
        MigrateEvent::Leftovers {
            configs: vec![("Codex", PathBuf::from("/h/.codex/config.toml"))],
            env: vec!["PROJECTMEM_ROOT".to_owned()],
        },
        MigrateEvent::Banner { apply: true },
        MigrateEvent::HomeMissing(PathBuf::from("/h/.projectmem")),
        MigrateEvent::Done,
    ]);
    insta::assert_snapshot!(text);
}

#[test]
fn a_failed_step_keeps_the_lines_printed_before_it() {
    let machine = Machine::new();
    let root = machine.base.join("repo");
    fs::create_dir_all(root.join(".projectmem")).unwrap();
    fs::write(root.join(".projectmem/notes.md"), "see .projectmem/x\n").unwrap();
    let blocker = machine.base.join("blocker");
    fs::write(&blocker, "a file where the new home should be").unwrap();
    let options = MigrateOptions {
        apply: true,
        old_home: machine.home().join(".projectmem"),
        cwd: machine.base.clone(),
        extra_roots: vec![root.display().to_string()],
        clients: machine.paths(),
        legacy_env: Vec::new(),
        djf: "djf".to_owned(),
    };
    let mut settings = machine.settings();
    settings.dejafix_home = blocker.join("home");
    fs::create_dir_all(options.old_home.join("global")).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = run_migrate(&options, &settings, &mut out, &mut err).unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)));
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("Close running agent sessions"));
    assert!(text.contains(&format!("Home: {} -> ", options.old_home.display())));
    assert!(root.join(".projectmem").is_dir());
    assert!(Path::new(&options.old_home).join("global").is_dir());
}
