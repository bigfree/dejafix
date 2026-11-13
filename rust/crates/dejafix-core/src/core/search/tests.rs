use serde_json::json;

use super::{
    SearchQuery, excerpt, match_line, mcp_match_line, no_match_message, search, search_reply,
};
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

fn long_text() -> String {
    let words: Vec<String> = (0..120).map(|index| format!("word{index}")).collect();
    format!("gotcha: {}", words.join(" "))
}

fn words(prefix: &str, range: std::ops::Range<usize>) -> String {
    range
        .map(|index| format!("{prefix}{index}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn excerpt_clips_from_the_start_when_the_match_fits() {
    assert_eq!(
        excerpt(&long_text(), "gotcha"),
        format!("gotcha: {}...", words("word", 0..42))
    );
    assert_eq!(excerpt("x", "y"), "x");
}

#[test]
fn excerpt_shows_the_part_that_matched_past_the_clip() {
    assert_eq!(
        excerpt(&long_text(), "word100"),
        format!("...{}", words("word", 86..120))
    );
    let text = format!(
        "{} the argon2 migration broke logins",
        words("filler", 0..80)
    );
    assert_eq!(
        excerpt(&text, "argon2"),
        format!(
            "...{} the argon2 migration broke logins",
            words("filler", 70..80)
        )
    );
}

#[test]
fn excerpt_offsets_follow_python_when_casefold_changes_length() {
    let text = format!("{} MATCH here", "Straße ".repeat(60));
    assert_eq!(
        excerpt(&text, "match"),
        "...Straße Straße Straße Straße Straße MATCH here"
    );
}

fn one(line: &serde_json::Value) -> EventLog<'static> {
    let text = format!("{line}\n");
    let context = PathContext {
        root: "/p".into(),
        cwd: "/p".into(),
        fold_case: false,
    };
    EventLog::parse(text.as_bytes(), &fixed_clock(), &context, &NoResolve).into_owned()
}

#[test]
fn mcp_match_lines_are_python_s() {
    let notes = one(
        &json!({"id": "evt_1", "type": "note", "summary": "queue tuning", "notes": "retry storm after the argon2 upgrade", "timestamp": "2027-01-02T03:04:05Z"}),
    );
    let event = notes.events().first();
    assert_eq!(
        event
            .map(|event| mcp_match_line(event, "argon2", false, false))
            .as_deref(),
        Some(
            "  [note] evt_1 2027-01-02: queue tuning | notes: retry storm after the argon2 upgrade"
        )
    );
    let multi = one(
        &json!({"id": "evt_2", "type": "attempt", "issue_id": "0003", "outcome": "failed", "summary": "multi\nline\nterm note", "location": "src/a.py\n:3", "timestamp": "2027-01-02T03:04:05Z"}),
    );
    let event = multi.events().first();
    assert_eq!(
        event
            .map(|event| mcp_match_line(event, "term", true, true))
            .as_deref(),
        Some(
            "  [attempt #0003 (failed)] evt_2 2027-01-02: multi line term note @ src/a.py :3 (superseded)"
        )
    );
    let clipped = one(
        &json!({"id": "evt_3", "type": "note", "summary": long_text(), "notes": "n", "timestamp": "2027-01-02T03:04:05Z"}),
    );
    let event = clipped.events().first();
    assert_eq!(
        event.map(|event| mcp_match_line(event, "n", false, false)),
        Some(format!(
            "  [note] evt_3 2027-01-02: gotcha: {}... | notes: n",
            words("word", 0..42)
        ))
    );
}

#[test]
fn search_reply_lists_the_last_matches_oldest_first() {
    let lines = log_text();
    let context = PathContext {
        root: "/p".into(),
        cwd: "/p".into(),
        fold_case: false,
    };
    let log = EventLog::parse(lines.as_bytes(), &fixed_clock(), &context, &NoResolve);
    assert_eq!(
        search_reply(&log, "use", 1, false),
        "Found 1 match(es) for 'use':\n  [decision] evt_6 2027-01-01: use argon2"
    );
    assert_eq!(
        search_reply(&log, "use", 10, false),
        "Found 3 match(es) for 'use':\n  [attempt #0001 (worked)] evt_4 2027-01-01: rewrote the carousel\n  [decision] evt_5 2027-01-01: use bcrypt (superseded)\n  [decision] evt_6 2027-01-01: use argon2"
    );
    assert_eq!(
        search_reply(&log, "zebra", 10, false),
        "No events match 'zebra'."
    );
}
