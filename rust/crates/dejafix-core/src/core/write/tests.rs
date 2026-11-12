use serde_json::json;

use super::{
    Command, Ids, MarkerChange, MarkerState, ValidationError, WriteContext, WriteOutcome,
    checks_marker, marker_state, next_issue_id, plan_command, sole_open_issue,
};
use crate::core::event::{EventId, IssueId, Outcome};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::core::time::Clock;
use crate::testing::{NoResolve, SeqIds, fixed_clock};

const T: &str = "2027-01-15T07:00:00Z";

fn issue(id: &str, summary: &str, at: &str) -> String {
    json!({"id": format!("evt_i{id}"), "issue_id": id, "summary": summary, "timestamp": at, "type": "issue"}).to_string()
}

fn fix_line(id: &str) -> String {
    json!({"id": format!("evt_f{id}"), "issue_id": id, "summary": "fixed", "timestamp": T, "type": "fix"}).to_string()
}

fn decision_line(id: &str, summary: &str) -> String {
    json!({"id": id, "summary": summary, "timestamp": T, "type": "decision"}).to_string()
}

fn lines(lines: &[String]) -> String {
    lines
        .iter()
        .flat_map(|line| [line.as_str(), "\n"])
        .collect()
}

fn context_paths() -> PathContext {
    PathContext {
        root: "/work/proj".into(),
        cwd: "/work/proj".into(),
        fold_case: false,
    }
}

fn parse(text: &str) -> EventLog<'_> {
    EventLog::parse(
        text.as_bytes(),
        &fixed_clock(),
        &context_paths(),
        &NoResolve,
    )
}

fn context<'c>(clock: &'c Clock, marker: Option<&'c str>) -> WriteContext<'c> {
    WriteContext {
        clock,
        git_commit: None,
        marker,
        redact: true,
    }
}

fn attempt(text: &str, outcomes: &[Outcome], issue: Option<&str>, auto_issue: bool) -> Command {
    Command::Attempt {
        text: text.into(),
        outcomes: outcomes.to_vec(),
        location: None,
        issue: issue.map(Into::into),
        auto_issue,
    }
}

fn fix(text: &str, issue: Option<&str>) -> Command {
    Command::Fix {
        text: text.into(),
        location: None,
        issue: issue.map(Into::into),
    }
}

fn plan(
    text: &str,
    command: Command,
    marker: Option<&str>,
) -> Result<super::WritePlan, ValidationError> {
    let clock = fixed_clock();
    let log = parse(text);
    plan_command(
        &log,
        command,
        &context(&clock, marker),
        &mut SeqIds::default(),
    )
}

#[test]
fn log_opens_the_next_issue_and_sets_the_marker() {
    let text = lines(&[issue("0009", "a", T), issue("abc", "b", T)]);
    let planned = plan(
        &text,
        Command::Log {
            text: "new".into(),
            location: Some("a.py:1".into()),
        },
        None,
    )
    .unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Logged("0010".into()));
    assert!(matches!(planned.marker, MarkerChange::Set(ref id) if id.as_str() == "0010"));
    assert_eq!(planned.events.len(), 1);
}

#[test]
fn next_issue_id_counts_digit_ids_numerically() {
    let text = lines(&[
        issue("0009", "a", T),
        issue("abc", "b", T),
        issue("12", "c", T),
        issue("²", "d", T),
    ]);
    assert_eq!(next_issue_id(&parse(&text)).as_str(), "0013");
    assert_eq!(next_issue_id(&parse("")).as_str(), "0001");
    let wide = lines(&[issue("9999", "a", T)]);
    assert_eq!(next_issue_id(&parse(&wide)).as_str(), "10000");
}

#[test]
fn attempt_after_fix_does_not_misattribute() {
    let text = lines(&[
        issue("0001", "old bug", "2027-01-15T07:58:00Z"),
        fix_line("0001"),
    ]);
    let error = plan(&text, attempt("try again", &[], None, false), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        "No active issue. Run `djf log \"<summary>\"` first, pass `--issue <id>` to attach explicitly, or rerun with `--auto-issue` to auto-create a parent issue from this attempt's text."
    );
}

#[test]
fn markerless_attempt_attaches_only_to_an_issue_opened_in_the_last_five_minutes() {
    let recent = lines(&[issue("0003", "fresh", "2027-01-15T07:55:00Z")]);
    let planned = plan(&recent, attempt("x", &[], None, false), None).unwrap();
    assert_eq!(
        planned.outcome,
        WriteOutcome::Attempted {
            outcome: Outcome::Partial,
            issue: "0003".into()
        }
    );
    let old = lines(&[issue("0003", "old", "2027-01-15T07:54:59Z")]);
    assert_eq!(
        plan(&old, attempt("x", &[], None, false), None).unwrap_err(),
        ValidationError::NoActiveIssue
    );
}

#[test]
fn attempt_uses_the_marker_and_rejects_two_outcomes() {
    let text = lines(&[issue("0001", "a", T), issue("0002", "b", T)]);
    let planned = plan(
        &text,
        attempt("x", &[Outcome::Failed], None, false),
        Some("0001"),
    )
    .unwrap();
    assert_eq!(
        planned.outcome,
        WriteOutcome::Attempted {
            outcome: Outcome::Failed,
            issue: "0001".into()
        }
    );
    assert_eq!(planned.marker, MarkerChange::Keep);
    assert_eq!(
        plan(
            &text,
            attempt("x", &[Outcome::Failed, Outcome::Worked], None, false),
            None
        )
        .unwrap_err()
        .to_string(),
        "Use only one of --worked, --failed, or --partial."
    );
}

#[test]
fn attempt_issue_id_is_normalised_and_unknown_ids_are_refused() {
    let text = lines(&[issue("0001", "a", T)]);
    let planned = plan(&text, attempt("x", &[], Some("1"), false), None).unwrap();
    assert_eq!(
        planned.outcome,
        WriteOutcome::Attempted {
            outcome: Outcome::Partial,
            issue: "0001".into()
        }
    );
    assert_eq!(
        plan(&text, attempt("x", &[], Some("42"), false), None)
            .unwrap_err()
            .to_string(),
        "Issue #0042 was not found. Run `djf search <query>` or `djf brief` to find the issue ID."
    );
    assert_eq!(
        plan(&text, attempt("x", &[], Some(" # "), false), None).unwrap_err(),
        ValidationError::UnknownIssue("?".into())
    );
    assert_eq!(
        plan(&text, attempt("x", &[], Some("#"), false), None).unwrap_err(),
        ValidationError::UnknownIssue("?".into())
    );
}

#[test]
fn auto_issue_creates_the_parent_issue_first() {
    let planned = plan(
        "",
        attempt("flaky test", &[Outcome::Worked], None, true),
        None,
    )
    .unwrap();
    assert_eq!(planned.events.len(), 2);
    assert_eq!(planned.events[0].issue.as_ref().unwrap().as_str(), "0001");
    assert_eq!(planned.events[1].summary.as_str(), "flaky test");
    assert!(matches!(planned.marker, MarkerChange::Set(ref id) if id.as_str() == "0001"));
}

#[test]
fn fix_closes_the_active_issue_and_clears_the_marker() {
    let text = lines(&[issue("0001", "a", T), issue("0002", "b", T)]);
    let planned = plan(&text, fix("done", None), Some("0001")).unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Fixed("0001".into()));
    assert_eq!(planned.marker, MarkerChange::Clear);
    let planned = plan(&text, fix("done", None), None).unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Fixed("0002".into()));
}

#[test]
fn fix_issue_closes_target_without_clearing_newer_active_issue() {
    let text = lines(&[issue("0001", "a", T), issue("0002", "b", T)]);
    let planned = plan(&text, fix("done", Some("1")), Some("0002")).unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Fixed("0001".into()));
    assert_eq!(planned.marker, MarkerChange::Keep);
    let planned = plan(&text, fix("done", Some("2")), Some("0002")).unwrap();
    assert_eq!(planned.marker, MarkerChange::Clear);
    assert_eq!(
        plan(&text, fix("done", Some("9")), None)
            .unwrap_err()
            .to_string(),
        "Issue #0009 was not found. Run `djf search <query>` or `djf brief` to find the issue ID."
    );
    assert_eq!(
        plan("", fix("done", None), None).unwrap_err().to_string(),
        "No open issue found. Run `djf log <text>` first."
    );
}

#[test]
fn a_stale_marker_is_ignored() {
    let text = lines(&[issue("0001", "real", T)]);
    let log = parse(&text);
    assert_eq!(marker_state(&log, Some("0099")), MarkerState::Stale);
    assert_eq!(
        marker_state(&log, Some("0001")),
        MarkerState::Valid("0001".into())
    );
    assert_eq!(marker_state(&log, None), MarkerState::Absent);
    let planned = plan(&text, fix("done", None), Some("0099")).unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Fixed("0001".into()));
    assert!(checks_marker(&fix("x", None)));
    assert!(checks_marker(&attempt("x", &[], None, false)));
    assert!(!checks_marker(&attempt("x", &[], Some("1"), false)));
    assert!(checks_marker(&attempt("x", &[], Some(""), false)));
    assert!(!checks_marker(&Command::Note {
        text: "x".into(),
        location: None
    }));
}

#[test]
fn decision_supersedes_by_unique_prefix_and_rejects_bad_references() {
    let text = lines(&[
        decision_line(
            "evt_abc123",
            "Use Postgres for everything, including the queue and search",
        ),
        decision_line("evt_abd999", "x"),
    ]);
    let planned = plan(
        &text,
        Command::Decision {
            text: "Use SQLite".into(),
            location: None,
            supersedes: Some("abc".into()),
        },
        None,
    )
    .unwrap();
    assert_eq!(
        planned.outcome,
        WriteOutcome::Decided {
            superseded: Some((
                EventId::from("evt_abc123").clone(),
                "Use Postgres for everything, including the queue and search"
                    .chars()
                    .take(60)
                    .collect()
            ))
        }
    );
    assert_eq!(
        planned.events[0].supersedes.as_ref().unwrap().as_str(),
        "evt_abc123"
    );
    assert!(
        plan(
            &text,
            Command::Decision {
                text: "x".into(),
                location: None,
                supersedes: Some("ab".into())
            },
            None
        )
        .is_err()
    );
    assert_eq!(
        plan(
            &text,
            Command::Decision {
                text: "x".into(),
                location: None,
                supersedes: Some("zz".into())
            },
            None
        )
        .unwrap_err()
        .to_string(),
        "No event matches 'zz'. Use `djf search <text>` to find the event id (shown as evt_...)."
    );
    let planned = plan(
        &text,
        Command::Decision {
            text: "x".into(),
            location: None,
            supersedes: Some(String::new()),
        },
        None,
    )
    .unwrap();
    assert_eq!(planned.outcome, WriteOutcome::Decided { superseded: None });
}

#[test]
fn several_open_issues_are_listed_newest_first_and_capped() {
    let text = lines(
        &(1..=12)
            .map(|n| issue(&format!("{n:04}"), "x", T))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        sole_open_issue(&parse(&text)).unwrap_err().to_string(),
        "12 issues are open, so pass issue_id (the id log_issue returned). Open issues, newest first: #0012, #0011, #0010, #0009, #0008, #0007, #0006, #0005, #0004, #0003 and 2 more."
    );
    let two = lines(&[issue("0001", "x", T), issue("0002", "y", T)]);
    assert_eq!(
        sole_open_issue(&parse(&two)).unwrap_err().to_string(),
        "2 issues are open, so pass issue_id (the id log_issue returned). Open issues, newest first: #0002, #0001."
    );
    let one = lines(&[issue("0001", "x", T)]);
    assert_eq!(
        sole_open_issue(&parse(&one)).unwrap().map(IssueId::as_str),
        Some("0001")
    );
    assert_eq!(sole_open_issue(&parse("")).unwrap(), None);
}

#[test]
fn new_events_are_written_like_python_to_dict() {
    let clock = fixed_clock();
    let log = parse("");
    let context = WriteContext {
        clock: &clock,
        git_commit: Some("abc1234"),
        marker: None,
        redact: true,
    };
    let planned = plan_command(
        &log,
        Command::Note {
            text: "  keep it  ".into(),
            location: Some(String::new()),
        },
        &context,
        &mut SeqIds::default(),
    )
    .unwrap();
    let mut line = Vec::new();
    planned.events[0].write_line(&mut line).unwrap();
    assert_eq!(
        String::from_utf8(line).unwrap(),
        "{\"git_commit\": \"abc1234\", \"id\": \"evt_00000000000000000001\", \"location\": \"\", \"summary\": \"keep it\", \"timestamp\": \"2027-01-15T08:00:00Z\", \"type\": \"note\"}\n"
    );
}

#[test]
fn an_empty_summary_is_refused() {
    assert_eq!(
        plan(
            "",
            Command::Note {
                text: " \n ".into(),
                location: None
            },
            None
        )
        .unwrap_err()
        .to_string(),
        "Event summary cannot be empty"
    );
}

#[test]
fn secrets_in_summary_and_location_are_redacted_unless_disabled() {
    let secret = format!("sk-{}", "a".repeat(44));
    let command = Command::Note {
        text: format!("key {secret}"),
        location: Some(format!("{secret}:1")),
    };
    let planned = plan("", command.clone(), None).unwrap();
    assert_eq!(
        planned.events[0].summary.as_str(),
        "key [REDACTED:openai_key]"
    );
    assert_eq!(
        planned.events[0].location.as_deref(),
        Some("[REDACTED:openai_key]:1")
    );
    assert_eq!(
        planned.notices,
        ["dejafix: redacted 2 secret(s) before write (openai_key)"]
    );
    let clock = fixed_clock();
    let log = parse("");
    let context = WriteContext {
        clock: &clock,
        git_commit: None,
        marker: None,
        redact: false,
    };
    let planned = plan_command(&log, command, &context, &mut SeqIds::default()).unwrap();
    assert!(planned.events[0].summary.as_str().contains(&secret));
    assert_eq!(planned.notices.len(), 0);
}

#[test]
fn os_ids_have_python_uuid4_shape() {
    let mut ids = super::OsIds;
    let id = ids.event_id();
    let hex = id.as_str().strip_prefix("evt_").unwrap();
    assert_eq!(hex.len(), 20);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(&hex[12..13], "4");
    assert!("89ab".contains(&hex[16..17]));
    assert_ne!(ids.event_id(), id);
    let entry = ids.entry_id("got");
    assert!(entry.starts_with("got_") && entry.len() == 16);
}
