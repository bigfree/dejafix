use std::cell::Cell;

use super::{
    Severity, SnoozeError, Warning, WarningKind, age, analyze, parse_snooze, read_expiry,
    remaining, render_warnings, snooze_note, snoozed_until, targets,
};
use crate::core::log::EventLog;
use crate::core::path::{PathContext, PathResolver};
use crate::core::stale::{CommitsSince, Stale};
use crate::core::time::normalize_timestamp;
use crate::testing::{NoResolve, fixed_clock};

const ROOT: &str = "/work/r";

fn context() -> PathContext {
    PathContext {
        root: ROOT.into(),
        cwd: ROOT.into(),
        fold_case: false,
    }
}

fn now() -> jiff::Timestamp {
    fixed_clock().now
}

fn check(lines: &[&str], files: &[&str]) -> Vec<Warning> {
    check_with(lines, files, &context())
}

fn check_with(lines: &[&str], files: &[&str], context: &PathContext) -> Vec<Warning> {
    let text = lines.join("\n");
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), context, &NoResolve);
    let files: Vec<String> = files.iter().map(|file| (*file).to_owned()).collect();
    let found = targets(&files, &log, context, &NoResolve);
    let churn = vec![None; found.files.len()];
    analyze(&log, &found, &churn, &[], now())
}

fn kinds(warnings: &[Warning]) -> Vec<WarningKind> {
    warnings.iter().map(|warning| warning.kind).collect()
}

fn lessons(warnings: &[Warning]) -> Vec<String> {
    warnings
        .iter()
        .filter(|warning| warning.kind == WarningKind::FailedAttempts)
        .flat_map(|warning| warning.details.clone())
        .collect()
}

const ISSUE: &str = r#"{"type": "issue", "issue_id": "0001", "summary": "token refresh race", "location": "src/auth.py", "timestamp": "2027-01-15T07:00:00Z"}"#;

#[test]
fn attempt_without_location_inherits_its_issue_file() {
    let warnings = check(
        &[
            ISSUE,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "failed", "summary": "wrapped refresh in a lock"}"#,
        ],
        &["src/auth.py"],
    );
    assert!(
        lessons(&warnings)
            .iter()
            .any(|line| line.contains("wrapped refresh in a lock"))
    );
}

#[test]
fn partial_and_old_failures_are_lessons_and_worked_is_not() {
    let warnings = check(
        &[
            ISSUE,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "partial", "summary": "retry halves the failures", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "failed", "summary": "pinned the clock skew", "location": "src/auth.py", "timestamp": "2026-11-01T00:00:00Z"}"#,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "worked", "summary": "serialized refresh", "location": "src/auth.py"}"#,
        ],
        &["src/auth.py"],
    );
    let lines = lessons(&warnings);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("~ retry halves the failures"))
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "x pinned the clock skew (2mo ago)")
    );
    assert!(!lines.iter().any(|line| line.contains("serialized")));
    let title = &warnings.first().unwrap().title;
    assert_eq!(
        title,
        "What already failed here (2 attempts, 1 only partly worked):"
    );
}

#[test]
fn overflow_names_the_issues_of_hidden_lessons() {
    let warnings = check(
        &[
            r#"{"type": "issue", "issue_id": "0001", "summary": "old race", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "partial", "summary": "old a", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "failed", "summary": "old b", "location": "src/auth.py"}"#,
            r#"{"type": "issue", "issue_id": "0002", "summary": "new race", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0002", "outcome": "failed", "summary": "new 0", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0002", "outcome": "failed", "summary": "new 1", "location": "src/auth.py"}"#,
            r#"{"type": "attempt", "issue_id": "0002", "outcome": "failed", "summary": "new 2", "location": "src/auth.py"}"#,
        ],
        &["src/auth.py"],
    );
    let first = warnings.first().unwrap();
    assert_eq!(first.severity, Severity::Block);
    assert_eq!(
        first.details.last().unwrap(),
        "  ... and 2 more, from #0001 (get_issue(issue_id))"
    );
    let open = warnings
        .iter()
        .find(|w| w.kind == WarningKind::OpenIssues)
        .unwrap();
    assert_eq!(open.title, "2 unresolved issues on this file");
    assert_eq!(open.details, ["#0001: old race", "#0002: new race"]);
}

#[test]
fn retired_events_and_memory_files_are_not_warned_about() {
    let warnings = check(
        &[
            r#"{"id": "evt_old", "type": "decision", "summary": "old routes", "location": "src/api/routes.py", "timestamp": "2027-01-10T00:00:00Z"}"#,
            r#"{"id": "evt_new", "type": "decision", "summary": "new routes", "location": "src/api/routes.py", "supersedes": "evt_old", "timestamp": "2027-01-11T00:00:00Z"}"#,
        ],
        &["src/api/routes.py"],
    );
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings.first().unwrap().details.first().unwrap(),
        "new routes"
    );
    let summary = [
        r#"{"type": "fix", "summary": "regen 0", "git_commit": "c0", "files": [".dejafix/summary.md"]}"#,
        r#"{"type": "fix", "summary": "regen 1", "git_commit": "c1", "files": [".dejafix/summary.md"]}"#,
        r#"{"type": "fix", "summary": "regen 2", "git_commit": "c2", "files": [".dejafix/summary.md"]}"#,
        r#"{"type": "fix", "summary": "regen 3", "git_commit": "c3", "files": [".dejafix/summary.md"]}"#,
    ];
    for spelled in [
        ".dejafix/summary.md",
        "./.dejafix/summary.md",
        "/work/r/.dejafix/summary.md",
        ".dejafix\\summary.md",
    ] {
        assert!(check(&summary, &[spelled]).is_empty(), "{spelled}");
    }
}

#[test]
fn spellings_root_and_blank_paths() {
    let attempts = [
        r#"{"type": "attempt", "summary": "tried v1.0 of the client.", "outcome": "failed", "location": "src/f.py"}"#,
        r#"{"type": "attempt", "summary": "tried v1.1 of the client.", "outcome": "failed", "location": "src/f.py"}"#,
        r#"{"type": "attempt", "summary": "tried v1.2 of the client.", "outcome": "failed", "location": "src/f.py"}"#,
    ];
    assert_eq!(check(&attempts, &["src/f.py"]).len(), 1);
    assert_eq!(
        check(&attempts, &["src/f.py", "./src/f.py", "/work/r/src/f.py"]).len(),
        1
    );
    assert_eq!(check(&attempts, &["/work/r"]), []);
    for blank in ["", "   ", "\n"] {
        assert_eq!(check(&attempts, &[blank]), []);
    }
    assert_eq!(check(&attempts, &[" src/f.py\n"]).len(), 1);
    let nested = r#"{"type": "attempt", "summary": "tried hand-editing the fixture", "outcome": "failed", "location": "tests/fixtures/.dejafix/events.jsonl"}"#;
    assert_eq!(
        kinds(&check(&[nested], &["tests/fixtures/.dejafix/events.jsonl"])),
        [WarningKind::FailedAttempts]
    );
}

#[test]
fn a_windows_drive_path_matches_on_a_windows_root() {
    let windows = PathContext {
        root: "C:/proj".into(),
        cwd: "C:/proj".into(),
        fold_case: true,
    };
    let attempts = [
        r#"{"type": "attempt", "summary": "lock variant 0", "outcome": "failed", "location": "src/f.py"}"#,
        r#"{"type": "attempt", "summary": "lock variant 1", "outcome": "failed", "location": "src/f.py"}"#,
    ];
    assert_eq!(
        kinds(&check_with(&attempts, &["C:\\proj\\src\\f.py"], &windows)),
        [WarningKind::FailedAttempts]
    );
}

struct Counting(Cell<usize>);

impl PathResolver for Counting {
    fn resolve(&self, _: &str) -> Option<String> {
        self.0.set(self.0.get() + 1);
        None
    }
}

#[test]
fn each_logged_path_is_normalised_once() {
    let lines: Vec<String> = (0..300)
        .map(|index| {
            format!(
                r#"{{"type": "note", "summary": "n{index}", "files": ["/elsewhere/src/f.py", "src/g.py"], "location": "/elsewhere/src/f.py:{index}"}}"#
            )
        })
        .collect();
    let text = lines.join("\n");
    let resolver = Counting(Cell::new(0));
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &resolver);
    let files: Vec<String> = (0..5).map(|index| format!("src/m{index}.py")).collect();
    let found = targets(&files, &log, &context(), &resolver);
    assert!(resolver.0.get() <= 3, "{}", resolver.0.get());
    assert_eq!(analyze(&log, &found, &[None; 5], &[], now()), []);
}

#[test]
fn every_check_and_both_renderings() {
    let lines = [
        r#"{"type": "issue", "issue_id": "0007", "summary": "payment form jumps", "location": "payment.py", "timestamp": "2027-01-15T06:00:00Z"}"#,
        r#"{"type": "attempt", "issue_id": "0007", "outcome": "failed", "summary": "tried CSS contain:layout", "location": "payment.py", "timestamp": "2027-01-14T07:00:00Z"}"#,
        r#"{"type": "attempt", "issue_id": "0007", "outcome": "failed", "summary": "debounced the handler", "location": "payment.py", "timestamp": "2027-01-10T07:00:00Z"}"#,
        r#"{"type": "attempt", "outcome": "failed", "summary": "Reverted: drop the cache", "location": "payment.py", "capture_source": "git_post_revert", "git_message": "Revert \"drop the cache\"", "auto_captured": true, "timestamp": "2026-12-25T07:00:00Z"}"#,
        r#"{"id": "evt_d", "type": "decision", "summary": "payments go through one gateway", "location": "payment.py", "timestamp": "2026-12-20T07:00:00Z"}"#,
    ];
    let text = lines.join("\n");
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    let found = targets(&["payment.py".to_owned()], &log, &context(), &NoResolve);
    let decision = log.events().get(4).unwrap();
    let stale = [Stale {
        event: decision,
        file: "payment.py".to_owned(),
        commits_since: CommitsSince::Count(5),
    }];
    let warnings = analyze(&log, &found, &[Some(6)], &stale, now());
    assert_eq!(
        kinds(&warnings),
        [
            WarningKind::FailedAttempts,
            WarningKind::OpenIssues,
            WarningKind::HighChurn,
            WarningKind::RecentRevert,
            WarningKind::RelevantDecision,
            WarningKind::PossiblyStale,
        ]
    );
    insta::assert_snapshot!(
        "plain_warn",
        render_warnings(&warnings, "warn", false).unwrap()
    );
    insta::assert_snapshot!(
        "plain_info",
        render_warnings(&warnings, "info", false).unwrap()
    );
    insta::assert_snapshot!(
        "colour_block",
        render_warnings(&warnings, "block", true).unwrap()
    );
}

#[test]
fn ages_follow_python_timedelta_floors() {
    let at =
        |text: &'static str| normalize_timestamp(text.into(), &jiff::tz::TimeZone::UTC).unwrap();
    assert_eq!(age(&at("2027-01-15T07:30:00Z"), now()), "just now");
    assert_eq!(age(&at("2027-01-15T05:00:00Z"), now()), "3h ago");
    assert_eq!(age(&at("2027-01-14T07:00:00Z"), now()), "yesterday");
    assert_eq!(age(&at("2027-01-10T07:00:00Z"), now()), "5d ago");
    assert_eq!(age(&at("2026-12-25T07:00:00Z"), now()), "3w ago");
    assert_eq!(age(&at("2026-10-01T07:00:00Z"), now()), "3mo ago");
    assert_eq!(age(&at("2027-01-16T08:00:00Z"), now()), "-1d ago");
    assert_eq!(age(&at("not a time"), now()), "unknown");
}

#[test]
fn snooze_durations_and_remaining_time() {
    assert_eq!(parse_snooze("30m").unwrap().as_secs(), 1_800);
    assert_eq!(parse_snooze(" 2H ").unwrap().as_secs(), 7_200);
    assert_eq!(parse_snooze("1 d").unwrap().as_secs(), 86_400);
    assert_eq!(
        parse_snooze("soon"),
        Err(SnoozeError::Invalid("soon".to_owned()))
    );
    assert_eq!(parse_snooze("0h"), Err(SnoozeError::NotPositive));
    assert_eq!(
        parse_snooze("soon").unwrap_err().to_string(),
        "Invalid duration 'soon' — use forms like 30m, 2h, or 1d."
    );
    let expiry = snoozed_until(now(), parse_snooze("2h").unwrap(), "2h").unwrap();
    assert_eq!(
        snooze_note("2h", expiry),
        "precheck warnings snoozed for 2h (until 2027-01-15T10:00:00+00:00)"
    );
    assert_eq!(read_expiry("2027-01-15T10:00:00+00:00\n"), Some(expiry));
    assert_eq!(read_expiry("garbage"), None);
    let later = |seconds: i64| jiff::Timestamp::from_second(1_800_000_000 + seconds).unwrap();
    assert_eq!(remaining(expiry, later(1)), "1h59m");
    assert_eq!(remaining(expiry, now()), "2h");
    assert_eq!(remaining(expiry, later(7_190)), "1m");
    assert_eq!(remaining(expiry, later(3_600 - 600)), "1h10m");
}
