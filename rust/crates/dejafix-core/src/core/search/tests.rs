use serde_json::json;

use super::{SearchQuery, match_line, no_match_message, search};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::testing::{NoResolve, fixed_clock};

fn log_text() -> String {
    [
        json!({"id": "evt_1", "issue_id": "0001", "summary": "token expiry bug", "timestamp": "2027-01-01T00:00:00Z", "type": "issue"}),
        json!({"id": "evt_2", "summary": "startup is slow", "timestamp": "2027-01-01T00:00:01Z", "type": "note", "notes": "STRASSE closed"}),
        json!({"id": "evt_3", "issue_id": "0001", "outcome": "failed", "summary": "tried contain layout", "location": "payment.py", "timestamp": "2027-01-01T00:00:02Z", "type": "attempt"}),
        json!({"id": "evt_4", "issue_id": "0001", "outcome": "worked", "summary": "rewrote the carousel", "files": ["src/Favicon.ico"], "timestamp": "2027-01-01T00:00:03Z", "type": "attempt"}),
        json!({"id": "evt_5", "summary": "use bcrypt", "timestamp": "2027-01-01T00:00:04Z", "type": "decision"}),
        json!({"id": "evt_6", "summary": "use argon2", "supersedes": "evt_5", "timestamp": "2027-01-01T00:00:05Z", "type": "decision"}),
    ]
    .iter()
    .flat_map(|event| [event.to_string(), "\n".to_owned()])
    .collect()
}

fn found(text: &str, regex: bool, failed_only: bool) -> Vec<String> {
    let lines = log_text();
    let context = PathContext {
        root: "/p".into(),
        cwd: "/p".into(),
        fold_case: false,
    };
    let log = EventLog::parse(lines.as_bytes(), &fixed_clock(), &context, &NoResolve);
    search(
        &log,
        &SearchQuery {
            text,
            regex,
            failed_only,
        },
    )
    .into_iter()
    .map(|event| event.id.as_str().to_owned())
    .collect()
}

#[test]
fn substring_search_is_casefolded_over_summary_notes_location_and_files() {
    assert_eq!(found("token", false, false), ["evt_1"]);
    assert_eq!(found("straße", false, false), ["evt_2"]);
    assert_eq!(found("PAYMENT.PY", false, false), ["evt_3"]);
    assert_eq!(found("favicon", false, false), ["evt_4"]);
    assert_eq!(
        found("carousel|favicon", false, false),
        Vec::<String>::new()
    );
}

#[test]
fn regex_search_supports_alternation_and_falls_back_on_bad_patterns() {
    assert_eq!(found("carousel|favicon", true, false), ["evt_4"]);
    assert_eq!(
        found("^USE (bcrypt|argon2)$", true, false),
        ["evt_5", "evt_6"]
    );
    assert_eq!(found("(", true, false), Vec::<String>::new());
    assert_eq!(found("(con", true, false), Vec::<String>::new());
    assert_eq!(found(r"(\w)\1", true, false), Vec::<String>::new());
}

#[test]
fn failed_only_keeps_failed_attempts() {
    assert_eq!(found("carousel", false, true), Vec::<String>::new());
    assert_eq!(found("contain", false, true), ["evt_3"]);
    assert_eq!(found("payment.py", false, true), ["evt_3"]);
}

#[test]
fn lines_and_messages_match_python() {
    let lines = log_text();
    let context = PathContext {
        root: "/p".into(),
        cwd: "/p".into(),
        fold_case: false,
    };
    let log = EventLog::parse(lines.as_bytes(), &fixed_clock(), &context, &NoResolve);
    let events = log.events();
    assert_eq!(
        match_line(&events[2], false),
        "2027-01-01T00:00:02Z [evt_3] attempt #0001 (failed): tried contain layout"
    );
    assert_eq!(
        match_line(&events[4], true),
        "2027-01-01T00:00:04Z [evt_5] decision: use bcrypt (superseded)"
    );
    let plain = SearchQuery {
        text: "a|b",
        regex: false,
        failed_only: false,
    };
    assert!(no_match_message(&plain).starts_with("No matches. (Tip:"));
    assert_eq!(
        no_match_message(&SearchQuery {
            regex: true,
            ..plain
        }),
        "No matches."
    );
    assert_eq!(
        no_match_message(&SearchQuery {
            text: "ab",
            ..plain
        }),
        "No matches."
    );
}
