use std::fs;
use std::path::Path;

use super::{
    Project, SearchOutput, Settings, WiringError, record, regenerate, search_project, show,
};
use crate::core::event::Outcome;
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
