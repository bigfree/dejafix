use std::borrow::Cow;

use proptest::prelude::*;
use serde_json::{Map, Value};

use super::{Event, IssueId, LineContext, LineError, decode_line};
use crate::core::json::write_canonical;
use crate::testing::fixed_clock;

fn decode(line: &str) -> Result<Event<'_>, LineError> {
    let clock = fixed_clock();
    decode_line(line, &LineContext::new(&clock))
}

fn round_trip(line: &str) -> String {
    let event = decode(line).unwrap();
    let mut out = Vec::new();
    event.write_line(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn error(line: &str) -> LineError {
    decode(line).unwrap_err()
}

#[test]
fn a_python_line_round_trips_byte_for_byte() {
    let line = r#"{"files": ["src/a.py"], "id": "evt_0123456789abcdef0123", "issue_id": "0001", "outcome": "failed", "summary": "Try: na\u00efve fa\u00e7ade \ud83d\ude80", "timestamp": "2026-05-12T21:07:46Z", "type": "attempt", "x_fixture_extra": {"kept": true, "n": 3}}"#;
    assert_eq!(round_trip(line), format!("{line}\n"));
}

#[test]
fn every_known_field_round_trips() {
    let line = r#"{"auto_captured": true, "capture_confidence": "high", "capture_source": "git_post_revert", "command": "pytest", "files": ["a", "b"], "git_commit": "abc", "git_message": "chore: x", "id": "evt_1", "issue_id": "0002", "location": "src/a.py:3", "notes": "n", "outcome": "partial", "summary": "s", "supersedes": "evt_0", "timestamp": "2026-05-12T21:07:46Z", "type": "decision"}"#;
    assert_eq!(round_trip(line), format!("{line}\n"));
}

#[test]
fn python_normalisations_are_applied() {
    let line = r#"{"type":"note","summary":"  padded  ","timestamp":"2026-05-12 21:07:46 -0600","files":null,"auto_captured":0,"issue_id":false,"location":"","command":0.0,"unknown":null}"#;
    assert_eq!(
        round_trip(line),
        "{\"id\": \"evt_19f13b42bf8ba71a2d41\", \"location\": \"\", \"summary\": \"padded\", \"timestamp\": \"2026-05-13T03:07:46Z\", \"type\": \"note\", \"unknown\": null}\n"
    );
}

#[test]
fn an_event_without_an_id_gets_the_same_id_on_every_read() {
    let line =
        r#"{"type": "note", "summary": "hand-written note", "timestamp": "2026-10-01T08:00:00Z"}"#;
    let first = decode(line).unwrap();
    let second = decode(line).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.id.as_str(), "evt_75f00711fdf76fa450d5");
}

#[test]
fn events_without_ids_but_different_content_get_different_ids() {
    let one = decode(r#"{"type": "note", "summary": "one", "timestamp": "2026-10-01T08:00:00Z"}"#)
        .unwrap();
    let two = decode(r#"{"type": "note", "summary": "two", "timestamp": "2026-10-01T08:00:00Z"}"#)
        .unwrap();
    assert_eq!(one.id.as_str(), "evt_60d5562bf983165733ca");
    assert_eq!(two.id.as_str(), "evt_6a7ba8ade5ec2441c9eb");
}

#[test]
fn a_stable_id_hashes_unknown_fields_in_python_form() {
    let line = r#"{"id": "", "type": "note", "summary": "x", "timestamp": "2026-10-01T08:00:00Z", "x": [1.5, 1e5, {"b": "\u00e9", "a": 1}]}"#;
    assert_eq!(
        round_trip(line),
        "{\"id\": \"evt_8b04077db81443ec8aa3\", \"summary\": \"x\", \"timestamp\": \"2026-10-01T08:00:00Z\", \"type\": \"note\", \"x\": [1.5, 100000.0, {\"a\": 1, \"b\": \"\\u00e9\"}]}\n"
    );
}

#[test]
fn a_stored_id_is_kept() {
    let event = decode(r#"{"id": "evt_kept", "type": "note", "summary": "x"}"#).unwrap();
    assert_eq!(event.id.as_str(), "evt_kept");
}

#[test]
fn a_missing_timestamp_is_now() {
    let event =
        decode(r#"{"id": "evt_kept", "type": "note", "summary": "x", "timestamp": ""}"#).unwrap();
    assert_eq!(event.at.as_str(), "2027-01-15T08:00:00Z");
}

#[test]
fn duplicate_keys_keep_the_last_value() {
    let line = r#"{"id": "a", "type": "note", "summary": "x", "id": "b", "timestamp": "2026-10-01T08:00:00Z"}"#;
    assert_eq!(
        round_trip(line),
        "{\"id\": \"b\", \"summary\": \"x\", \"timestamp\": \"2026-10-01T08:00:00Z\", \"type\": \"note\"}\n"
    );
}

#[test]
fn an_odd_typed_pass_through_field_is_kept_verbatim() {
    let line = r#"{"id": "a", "type": "note", "summary": "x", "timestamp": "2026-10-01T08:00:00Z", "issue_id": 7, "location": {"a": 1}}"#;
    let event = decode(line).unwrap();
    assert!(event.issue.is_none() && event.location.is_none());
    assert_eq!(
        round_trip(line),
        "{\"id\": \"a\", \"issue_id\": 7, \"location\": {\"a\": 1}, \"summary\": \"x\", \"timestamp\": \"2026-10-01T08:00:00Z\", \"type\": \"note\"}\n"
    );
}

#[test]
fn lines_python_rejects_are_errors() {
    let note = |extra: &str| format!(r#"{{"type": "note", "summary": "x"{extra}}}"#);
    assert_eq!(error("{broken"), LineError::Json);
    assert_eq!(error("[1, 2]"), LineError::NotAnObject);
    assert_eq!(error("\"text\""), LineError::NotAnObject);
    assert_eq!(error(r#"{"summary": "x"}"#), LineError::Kind);
    assert_eq!(error(r#"{"type": "bug", "summary": "x"}"#), LineError::Kind);
    assert_eq!(error(r#"{"type": null, "summary": "x"}"#), LineError::Kind);
    assert_eq!(error(r#"{"type": "note"}"#), LineError::Summary);
    assert_eq!(
        error(r#"{"type": "note", "summary": 5}"#),
        LineError::Summary
    );
    assert_eq!(
        error(r#"{"type": "note", "summary": " \u001f "}"#),
        LineError::Summary
    );
    assert_eq!(error(&note(r#", "outcome": "won""#)), LineError::Outcome);
    assert_eq!(error(&note(r#", "outcome": """#)), LineError::Outcome);
    assert_eq!(
        error(&note(r#", "capture_source": "x""#)),
        LineError::CaptureSource
    );
    assert_eq!(
        error(&note(r#", "capture_confidence": 1"#)),
        LineError::Confidence
    );
    assert_eq!(error(&note(r#", "timestamp": 5"#)), LineError::Timestamp);
    assert_eq!(
        error(&note(r#", "timestamp": "0001-01-01T00:30:00+01:00""#)),
        LineError::Timestamp
    );
}

#[test]
fn documented_divergences_are_skipped() {
    let note = |extra: &str| format!(r#"{{"type": "note", "summary": "x"{extra}}}"#);
    assert_eq!(
        error(&note(r#", "files": "ab""#)),
        LineError::Unsupported("files")
    );
    assert_eq!(
        error(&note(r#", "files": {"a": 1}"#)),
        LineError::Unsupported("files")
    );
    assert_eq!(
        error(&note(r#", "files": [1]"#)),
        LineError::Unsupported("files")
    );
    assert_eq!(error(&note(r#", "id": 5"#)), LineError::Unsupported("id"));
    assert_eq!(error(&note(r#", "x": NaN"#)), LineError::Json);
    assert_eq!(error(&note(r#", "summary": "\ud800""#)), LineError::Summary);
    assert_eq!(
        error(&note(r#", "id": "evt_1", "x": "\ud800""#)),
        LineError::Json
    );
    assert_eq!(
        error(&note(r#", "id": "evt_1", "location": ["\udc00"]"#)),
        LineError::Json
    );
}

#[test]
fn falsy_files_are_empty() {
    for files in ["null", "false", "0", "\"\"", "[]", "{}"] {
        let line = format!(r#"{{"type": "note", "summary": "x", "files": {files}}}"#);
        let event = decode(&line).unwrap();
        assert!(event.files.is_empty(), "{files}");
    }
}

#[test]
fn issue_ids_normalise_like_python() {
    for (input, expected) in [
        ("1", Some("0001")),
        ("#1", Some("0001")),
        ("0001", Some("0001")),
        ("##12", Some("0012")),
        (" 7 ", Some("0007")),
        ("12345", Some("12345")),
        ("abc", Some("abc")),
        ("# 1", Some(" 1")),
        ("١", Some("000١")),
        ("#", None),
        ("  ", None),
    ] {
        assert_eq!(
            IssueId::normalize(input).as_ref().map(IssueId::as_str),
            expected,
            "{input}"
        );
    }
}

#[test]
fn unescaped_strings_are_borrowed() {
    let line = r#"{"files": ["src/a.py"], "id": "evt_1", "location": "src/a.py:3", "summary": "plain", "timestamp": "2026-05-12T21:07:46Z", "type": "issue", "issue_id": "0001"}"#;
    let event = decode(line).unwrap();
    assert!(matches!(event.summary.0, Cow::Borrowed(_)));
    assert!(matches!(event.files.first(), Some(Cow::Borrowed(_))));
    assert!(matches!(event.location, Some(Cow::Borrowed(_))));
    assert!(matches!(event.id.0, Cow::Borrowed(_)));
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        (-1e300_f64..1e300).prop_map(Value::from),
        "[ -~\u{e9}\u{1f680}]{0,8}".prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            proptest::collection::btree_map("[a-z]{1,4}", inner, 0..4)
                .prop_map(|map| Value::Object(map.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

fn optional_text() -> impl Strategy<Value = Option<String>> {
    proptest::option::of("[ -~\u{e9}\u{1f680}]{0,12}")
}

prop_compose! {
    fn arb_event_line()(
        kind in prop::sample::select(vec!["issue", "hypothesis", "attempt", "fix", "decision", "note"]),
        summary in "[!-~\u{e9}\u{1f680}][ -~\u{e9}\u{1f680}]{0,20}[!-~]",
        id in "evt_[0-9a-f]{20}",
        issue in optional_text(),
        outcome in proptest::option::of(prop::sample::select(vec!["worked", "failed", "partial"])),
        files in proptest::collection::vec("[ -~]{0,10}", 0..3),
        location in optional_text(),
        command in optional_text(),
        supersedes in optional_text(),
        auto in any::<bool>(),
        source in proptest::option::of(prop::sample::select(vec!["git_post_commit", "ci_parser"])),
        extras in proptest::collection::btree_map("x_[a-z]{1,4}", arb_json(), 0..3),
    ) -> String {
        let mut object = Map::new();
        object.insert("type".into(), kind.into());
        object.insert("summary".into(), summary.into());
        object.insert("id".into(), id.into());
        object.insert("timestamp".into(), "2026-05-12T21:07:46Z".into());
        for (key, value) in [("issue_id", issue), ("location", location), ("command", command), ("supersedes", supersedes)] {
            if let Some(value) = value {
                object.insert(key.into(), value.into());
            }
        }
        if let Some(outcome) = outcome {
            object.insert("outcome".into(), outcome.into());
        }
        if !files.is_empty() {
            object.insert("files".into(), files.into());
        }
        if auto {
            object.insert("auto_captured".into(), true.into());
        }
        if let Some(source) = source {
            object.insert("capture_source".into(), source.into());
        }
        object.extend(extras);
        let mut out = Vec::new();
        write_canonical(&mut out, &Value::Object(object)).unwrap();
        String::from_utf8(out).unwrap()
    }
}

proptest! {
    #[test]
    fn decoding_any_line_never_panics(line in "\\PC{0,200}") {
        let _ = decode(&line);
    }

    #[test]
    fn decoding_json_shaped_noise_never_panics(line in r#"\{("[a-z_]{1,12}": ("[^"\\]{0,8}"|-?[0-9]{1,3}|true|false|null|\[\]|\{\}), ){0,6}"type": "(note|issue|fix|bug)", "summary": "[ -~]{0,10}"\}"#) {
        let _ = decode(&line);
    }

    #[test]
    fn writing_then_reading_preserves_every_field(line in arb_event_line()) {
        let first = round_trip(&line);
        prop_assert_eq!(&first, &format!("{line}\n"));
        prop_assert_eq!(round_trip(first.trim_end_matches('\n')), first);
    }

    #[test]
    fn issue_id_normalisation_is_idempotent(input in "\\s{0,2}#{0,2}[0-9a-z\u{661}]{0,6}\\s{0,2}") {
        let once = IssueId::normalize(&input);
        let twice = once.as_ref().and_then(|id| IssueId::normalize(id.as_str()));
        prop_assert_eq!(once, twice);
    }
}
