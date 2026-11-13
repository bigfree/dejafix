use serde_json::json;

use super::{
    Score, ScoreError, calculate_score, raw_events, score_event, score_event_of, score_reply,
};
use crate::core::event::EventKind;
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::testing::{NoResolve, fixed_clock};

fn score_of(text: &str) -> Score {
    let raw = raw_events(text).unwrap();
    let events: Vec<_> = raw.iter().map(score_event).collect();
    calculate_score(&events)
}

fn reply_of(text: &str) -> String {
    score_reply(&score_of(text))
}

fn counted(
    failed: usize,
    fixes: usize,
    decisions: usize,
    notes: usize,
    worked: usize,
    tagged: usize,
) -> String {
    let mut events = Vec::new();
    events.extend((0..failed).map(|_| json!({"type": "attempt", "outcome": "failed"})));
    events.extend((0..fixes).map(|_| json!({"type": "fix"})));
    events.extend((0..decisions).map(|_| json!({"type": "decision"})));
    events.extend((0..notes).map(|_| json!({"type": "note"})));
    events.extend((0..worked).map(|_| json!({"type": "attempt", "outcome": "worked"})));
    for (index, event) in events.iter_mut().take(tagged).enumerate() {
        event["files"] = json!([format!("f{index}.py")]);
    }
    events
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn notes_with_files(files: usize, decisions: usize) -> String {
    let mut events: Vec<serde_json::Value> = (0..decisions)
        .map(|_| json!({"type": "decision"}))
        .collect();
    for index in 0..files {
        events.extend((0..4).map(|_| json!({"type": "note", "files": [format!("f{index}.py")]})));
    }
    events
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn located(location: &str) -> Score {
    let text = [
        json!({"type": "issue", "location": location}),
        json!({"type": "attempt", "outcome": "failed", "location": location}),
    ]
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    score_of(&text)
}

#[test]
fn an_empty_log_scores_zero() {
    assert_eq!(
        reply_of(""),
        "dejafix Prevention Score: F (0/100)\n  Failed approaches on record: 0\n  Decisions documented: 0\n  Fixes with context: 0\n  Debugging hours saved: ~0.0h\n  Tokens saved: 0\n  Estimated USD saved: $0.00"
    );
    let score = score_of("");
    assert_eq!((score.total, score.manual, score.auto), (0, 0, 0));
}

const MIXED: &str = r#"{"type": "issue", "summary": "a", "location": "src/a.py:10"}
{"type": "attempt", "outcome": "failed", "files": ["src/a.py", "src/b.py"], "location": "Foo.bar"}
{"type": "attempt", "outcome": "failed", "location": "C:\\x\\y.py:3"}
{"type": "attempt", "outcome": "worked", "files": ["src/a.py"], "auto_captured": true}
{"type": "attempt", "outcome": "partial"}

{"type": "fix", "files": ["src/a.py"], "auto_captured": 1}
{"type": "fix", "location": "src/a.py"}
{"type": "decision", "auto_captured": false}
{"type": "decision", "auto_captured": 0}
{"type": "decision", "auto_captured": ""}
{"type": "note", "auto_captured": []}
{"type": "note", "auto_captured": {}}
{"type": "note", "auto_captured": null}
{"type": "note", "auto_captured": "yes", "files": ["src/b.py", "src/b.py"]}
{"type": "note", "auto_captured": 0.0}
{"type": "note", "auto_captured": [0]}
{"type": "plan"}
{"type": "note", "files": ["./src/a.py", "src/a.py", "src/a.py"]}
"#;

#[test]
fn a_mixed_log_hits_every_counter() {
    assert_eq!(
        reply_of(MIXED),
        "dejafix Prevention Score: D (20/100)\n  Failed approaches on record: 2\n  Decisions documented: 3\n  Fixes with context: 2\n  Debugging hours saved: ~2.7h\n  Tokens saved: 19,500\n  Estimated USD saved: $0.20"
    );
    let score = score_of(MIXED);
    assert_eq!(
        (
            score.worked,
            score.notes,
            score.files_with_gotchas,
            score.high_churn_files,
            score.auto,
            score.manual,
            score.total,
        ),
        (1, 7, 4, 1, 4, 14, 18)
    );
}

#[test]
fn grades_change_exactly_on_the_boundaries() {
    let table = [
        ((0, 0, 0, 0, 0, 0), 0, "F"),
        ((0, 0, 0, 38, 0, 0), 19, "F"),
        ((0, 0, 0, 40, 0, 0), 20, "D"),
        ((0, 0, 13, 40, 0, 0), 39, "D"),
        ((10, 10, 0, 0, 0, 0), 40, "C"),
        ((0, 10, 13, 40, 0, 0), 59, "C"),
        ((10, 10, 14, 0, 0, 0), 60, "B"),
        ((10, 10, 13, 40, 0, 0), 79, "B"),
        ((10, 10, 14, 40, 0, 0), 80, "A"),
        ((10, 10, 14, 40, 0, 18), 89, "A"),
        ((10, 10, 14, 40, 0, 19), 89, "A"),
        ((10, 10, 14, 40, 0, 20), 90, "A+"),
        ((10, 10, 14, 40, 0, 40), 100, "A+"),
        ((0, 0, 0, 0, 19, 0), 19, "F"),
        ((0, 0, 0, 0, 20, 0), 20, "D"),
        ((100, 100, 100, 100, 100, 0), 80, "A"),
    ];
    for ((failed, fixes, decisions, notes, worked, files), score, grade) in table {
        let found = score_of(&counted(failed, fixes, decisions, notes, worked, files));
        assert_eq!(
            (found.score, found.grade),
            (score, grade),
            "{failed} {fixes} {decisions} {notes} {worked} {files}"
        );
    }
}

#[test]
fn the_top_boundary_replies_match_python() {
    assert_eq!(
        reply_of(&counted(10, 10, 14, 40, 0, 20)),
        "dejafix Prevention Score: A+ (90/100)\n  Failed approaches on record: 10\n  Decisions documented: 14\n  Fixes with context: 10\n  Debugging hours saved: ~9.6h\n  Tokens saved: 57,000\n  Estimated USD saved: $0.57"
    );
}

#[test]
fn rounding_follows_python_on_the_binary_value() {
    let rows = [
        (counted(0, 0, 3, 0, 0, 0), "~0.4h", "1,500", "$0.01"),
        (counted(0, 7, 0, 0, 0, 0), "~1.8h", "14,000", "$0.14"),
        (counted(0, 0, 7, 0, 0, 0), "~1.1h", "3,500", "$0.04"),
        (counted(1, 1, 1, 0, 0, 0), "~0.9h", "5,500", "$0.05"),
        (counted(2, 1, 1, 0, 0, 0), "~1.4h", "8,500", "$0.09"),
        (counted(0, 0, 200, 0, 0, 0), "~30.0h", "100,000", "$1.00"),
        (
            counted(1000, 1000, 1000, 0, 0, 0),
            "~900.0h",
            "5,500,000",
            "$55.00",
        ),
    ];
    for (text, hours, tokens, usd) in rows {
        let reply = reply_of(&text);
        let lines: Vec<&str> = reply.lines().collect();
        assert_eq!(
            lines.get(4).copied(),
            Some(format!("  Debugging hours saved: {hours}").as_str()),
            "{text}"
        );
        assert_eq!(
            lines.get(5).copied(),
            Some(format!("  Tokens saved: {tokens}").as_str())
        );
        assert_eq!(
            lines.get(6).copied(),
            Some(format!("  Estimated USD saved: {usd}").as_str())
        );
    }
}

#[test]
fn hours_print_with_pythons_long_float_repr() {
    let rows = [
        (notes_with_files(1, 3), "~1.2000000000000002h"),
        (notes_with_files(3, 1), "~2.3000000000000003h"),
        (notes_with_files(5, 4), "~4.3999999999999995h"),
        (notes_with_files(1, 0), "~0.8h"),
    ];
    for (text, hours) in rows {
        let reply = reply_of(&text);
        assert_eq!(
            reply.lines().nth(4),
            Some(format!("  Debugging hours saved: {hours}").as_str())
        );
    }
}

#[test]
fn a_location_counts_as_a_file_only_when_it_looks_like_one() {
    let table = [
        ("src/foo.py", 1),
        ("src/foo.py:42", 1),
        ("src/foo.py:42:7", 1),
        ("foo.py", 1),
        (r"C:\src\foo.py", 1),
        (r"C:\src\foo.py:42", 1),
        ("D:/repo/app.tsx:9", 1),
        (r"src\windows\rel.py:3", 1),
        ("AuthService.validate", 0),
        ("SomeClass", 0),
        ("", 0),
        ("   ", 0),
    ];
    for (location, files) in table {
        let found = located(location);
        assert_eq!(found.files_with_gotchas, files, "{location:?}");
        assert_eq!(found.high_churn_files, 0, "{location:?}");
    }
}

#[test]
fn a_bare_path_and_a_path_with_a_line_are_one_file() {
    let bare = located("src/foo.py");
    let with_line = located("src/foo.py:42");
    assert_eq!(bare.files_with_gotchas, with_line.files_with_gotchas);
    assert_eq!(bare.files_with_gotchas, 1);
}

#[test]
fn two_spellings_of_one_file_are_two_files() {
    let text = [
        json!({"type": "note", "files": ["foo.py"]}),
        json!({"type": "note", "files": ["./foo.py"]}),
    ]
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    assert_eq!(score_of(&text).files_with_gotchas, 2);
}

#[test]
fn a_file_repeated_inside_one_event_counts_every_mention() {
    let text = json!({"type": "note", "files": ["a.py", "a.py", "a.py", "a.py"]}).to_string();
    let score = score_of(&text);
    assert_eq!((score.files_with_gotchas, score.high_churn_files), (1, 1));
}

#[test]
fn raw_events_skip_blank_lines_and_split_like_python() {
    let text =
        "{\"type\": \"note\"}\n   \n\u{a0}\n{\"type\": \"fix\"}\u{2028}{\"type\": \"fix\"}\r\n";
    assert_eq!(raw_events(text).unwrap().len(), 3);
    assert_eq!(raw_events("\n\n").unwrap().len(), 0);
}

#[test]
fn raw_events_report_the_line_that_is_not_an_object() {
    let error = raw_events("{}\n\nnot json\n").unwrap_err();
    assert!(matches!(error, ScoreError::Json { line: 3, .. }));
    let error = raw_events("{}\n[1]\n").unwrap_err();
    assert_eq!(
        error,
        ScoreError::Json {
            line: 2,
            message: "expected a JSON object".to_owned(),
        }
    );
    let error = raw_events("{").unwrap_err();
    assert_eq!(
        error.to_string(),
        "line 1: EOF while parsing an object at line 1 column 1"
    );
}

#[test]
fn score_event_reads_loose_json() {
    let raw = raw_events(
        r#"{"type": 5, "outcome": 7, "auto_captured": 2, "files": ["a.py", 3, null, "b.py"], "location": 9}"#,
    )
    .unwrap();
    let event = score_event(raw.first().unwrap());
    assert_eq!(event.kind, "");
    assert_eq!(event.outcome, None);
    assert!(event.auto);
    assert_eq!(event.files, ["a.py", "b.py"]);
    assert_eq!(event.at, None);

    let raw = raw_events(r#"{"type": "fix", "files": "abc", "location": "x/y.rs:4"}"#).unwrap();
    let event = score_event(raw.first().unwrap());
    assert_eq!((event.kind, event.files), ("fix", vec!["x/y.rs".into()]));
    assert!(!event.auto);
}

#[test]
fn score_event_of_reads_a_parsed_event() {
    let text = r#"{"timestamp": "2026-03-04T05:06:07Z", "type": "attempt", "summary": "s", "outcome": "failed", "files": ["a.py"], "location": "b.py:3", "auto_captured": true}"#;
    let context = PathContext {
        root: "/work/r".into(),
        cwd: "/work/r".into(),
        fold_case: false,
    };
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context, &NoResolve);
    let event = log.events().first().unwrap();
    assert_eq!(event.kind, EventKind::Attempt);
    let found = score_event_of(event);
    assert_eq!(found.kind, "attempt");
    assert_eq!(found.outcome, Some("failed"));
    assert!(found.auto);
    assert_eq!(found.files, ["a.py", "b.py"]);
    assert_eq!(
        found.at,
        "2026-03-04T05:06:07Z".parse::<jiff::Timestamp>().ok()
    );
    let score = calculate_score(&[found]);
    assert_eq!(
        (score.failed, score.files_with_gotchas, score.auto),
        (1, 2, 1)
    );
}

#[test]
fn the_same_file_by_two_spellings_counts_once() {
    let text = [
        json!({"type": "issue", "summary": "a", "location": "src/foo.py"}),
        json!({"type": "attempt", "outcome": "failed", "summary": "b", "location": "src/foo.py:42"}),
    ]
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    assert_eq!(score_of(&text).files_with_gotchas, 1);
}
