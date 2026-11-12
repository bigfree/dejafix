use std::cell::Cell;

use proptest::prelude::*;

use super::{EventLog, EventRefError};
use crate::core::event::{EventId, IssueId, LineError};
use crate::core::path::{PathContext, PathResolver};
use crate::core::time::Clock;
use crate::testing::{NoResolve, fixed_clock};

fn clock() -> Clock {
    fixed_clock()
}

fn context() -> PathContext {
    PathContext {
        root: "/work/proj".into(),
        cwd: "/work/proj".into(),
        fold_case: false,
    }
}

fn parse(text: &str) -> EventLog<'_> {
    EventLog::parse(text.as_bytes(), &clock(), &context(), &NoResolve)
}

fn ids<'l>(log: &'l EventLog<'_>, path: &str) -> Vec<&'l str> {
    log.live_events_for_file(path)
        .map(|event| event.id.as_str())
        .collect()
}

const LOG: &str = concat!(
    "\u{feff}",
    r#"{"id": "evt_a1", "issue_id": "0001", "location": "src/a.py:3", "summary": "a", "timestamp": "2026-01-01T00:00:00Z", "type": "issue"}"#,
    "\n",
    r#"{"id": "evt_a2", "issue_id": "0001", "outcome": "failed", "summary": "b", "timestamp": "2026-01-01T00:00:01Z", "type": "attempt"}"#,
    "\n",
    "\n   \n",
    "{broken\n",
    r#"{"id": "evt_a3", "issue_id": "0002", "files": ["./src/b.py", "src\\c.py"], "summary": "c", "timestamp": "2026-01-01T00:00:02Z", "type": "issue"}"#,
    "\n",
    r#"{"id": "evt_a4", "issue_id": "0002", "summary": "d", "timestamp": "2026-01-01T00:00:03Z", "type": "fix"}"#,
    "\r\n",
    r#"{"id": "evt_a5", "summary": "e", "timestamp": "2026-01-01T00:00:04Z", "type": "decision", "location": "src/a.py"}"#,
    "\n",
    r#"{"id": "evt_a6", "summary": "f", "supersedes": "evt_a5", "timestamp": "2026-01-01T00:00:05Z", "type": "decision"}"#,
    "\n",
);

#[test]
fn builds_events_and_counts_skipped_lines() {
    let log = parse(LOG);
    assert_eq!(log.events().len(), 6);
    assert_eq!(log.skipped().len(), 1);
    assert_eq!(
        log.skipped().first().map(|s| (s.line, s.error)),
        Some((5, LineError::Json))
    );
}

#[test]
fn indexes_issues_retired_events_and_files() {
    let log = parse(LOG);
    assert_eq!(
        log.open_issues().map(IssueId::as_str).collect::<Vec<_>>(),
        ["0001"]
    );
    assert_eq!(log.current_issue().map(IssueId::as_str), Some("0001"));
    assert!(log.is_closed(&IssueId::normalize("2").unwrap()));
    assert!(!log.is_closed(&IssueId::from("0001")));
    assert!(log.is_retired(&EventId::from("evt_a5")));
    assert!(!log.is_retired(&EventId::from("evt_a6")));
    assert_eq!(ids(&log, "src/a.py"), ["evt_a1", "evt_a2"]);
    assert_eq!(ids(&log, "src/b.py"), ["evt_a3", "evt_a4"]);
    assert_eq!(ids(&log, "src/c.py"), ["evt_a3", "evt_a4"]);
    assert_eq!(ids(&log, "src/d.py"), Vec::<&str>::new());
    let mut files: Vec<_> = log
        .indexed_files()
        .map(|path| path.as_str().to_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["src/a.py", "src/b.py", "src/c.py"]);
}

#[test]
fn a_utf8_bom_does_not_break_the_event_log() {
    let log = parse("\u{feff}{\"type\": \"note\", \"summary\": \"first\"}\n");
    assert_eq!(
        log.events()
            .iter()
            .map(|e| e.summary.as_str())
            .collect::<Vec<_>>(),
        ["first"]
    );
    assert_eq!(log.skipped(), []);
}

#[test]
fn a_bom_after_the_first_byte_is_an_error() {
    let log = parse(
        "{\"type\": \"note\", \"summary\": \"first\"}\n\u{feff}{\"type\": \"note\", \"summary\": \"x\"}\n",
    );
    assert_eq!(log.events().len(), 1);
    assert_eq!(log.skipped().first().map(|s| s.line), Some(2));
}

#[test]
fn resolve_event_ref_ambiguous_and_missing() {
    let log = parse(concat!(
        r#"{"id": "evt_aaaa1111", "type": "note", "summary": "a"}"#,
        "\n",
        r#"{"id": "evt_aaaa2222", "type": "note", "summary": "b"}"#,
        "\n",
    ));
    assert_eq!(
        log.resolve_event_ref("evt_aaaa1111")
            .unwrap()
            .summary
            .as_str(),
        "a"
    );
    assert_eq!(
        log.resolve_event_ref("aaaa2222").unwrap().summary.as_str(),
        "b"
    );
    let ambiguous = log.resolve_event_ref("aaaa").unwrap_err();
    assert_eq!(
        ambiguous,
        EventRefError::Ambiguous {
            reference: "aaaa".into(),
            count: 2
        }
    );
    assert!(ambiguous.to_string().contains("ambiguous"));
    let missing = log.resolve_event_ref("zzzz").unwrap_err();
    assert!(missing.to_string().starts_with("No event matches 'zzzz'"));
    assert_eq!(
        log.resolve_event_ref("  ").unwrap_err(),
        EventRefError::Empty
    );
}

#[test]
fn an_event_inherits_the_file_of_the_last_issue_event_with_its_id() {
    let log = parse(concat!(
        r#"{"id": "e1", "issue_id": "0001", "location": "old.py", "type": "issue", "summary": "a"}"#,
        "\n",
        r#"{"id": "e2", "issue_id": "0001", "type": "hypothesis", "summary": "b"}"#,
        "\n",
        r#"{"id": "e3", "issue_id": "0001", "location": "new.py", "type": "issue", "summary": "c"}"#,
        "\n",
        r#"{"id": "e4", "issue_id": "0009", "type": "attempt", "summary": "d"}"#,
        "\n",
    ));
    assert_eq!(ids(&log, "new.py"), ["e2", "e3"]);
    assert_eq!(ids(&log, "old.py"), ["e1"]);
}

#[test]
fn a_fix_before_its_issue_still_closes_it() {
    let log = parse(concat!(
        r#"{"id": "e1", "issue_id": "0003", "type": "fix", "summary": "a"}"#,
        "\n",
        r#"{"id": "e2", "issue_id": "0003", "type": "issue", "summary": "b"}"#,
        "\n",
        r#"{"id": "e3", "type": "issue", "summary": "no id"}"#,
        "\n",
    ));
    assert_eq!(log.open_issues().count(), 0);
    assert_eq!(log.current_issue(), None);
}

#[test]
fn invalid_utf8_on_one_line_skips_only_that_line() {
    let bytes = b"{\"type\": \"note\", \"summary\": \"a\"}\n\xff\xfe\n{\"type\": \"note\", \"summary\": \"b\"}\n";
    let log = EventLog::parse(bytes, &clock(), &context(), &NoResolve);
    assert_eq!(log.events().len(), 2);
    assert_eq!(
        log.skipped().first().map(|s| (s.line, s.error)),
        Some((2, LineError::Utf8))
    );
}

#[test]
fn every_raw_path_is_normalised_once() {
    struct Counting<'c>(&'c Cell<usize>);
    impl PathResolver for Counting<'_> {
        fn resolve(&self, _: &str) -> Option<String> {
            self.0.set(self.0.get() + 1);
            None
        }
    }
    let line = r#"{"type": "note", "summary": "x", "files": ["/elsewhere/a.py"], "location": "/elsewhere/a.py:3"}"#;
    let text = [line; 50].join("\n");
    let calls = Cell::new(0);
    let log = EventLog::parse(text.as_bytes(), &clock(), &context(), &Counting(&calls));
    assert_eq!(log.events().len(), 50);
    assert_eq!(calls.get(), 1);
    assert_eq!(log.live_events_for_file("/elsewhere/a.py").count(), 50);
}

proptest! {
    #[test]
    fn parsing_any_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2_000)) {
        let log = EventLog::parse(&bytes, &clock(), &context(), &NoResolve);
        prop_assert!(log.events().len() + log.skipped().len() <= bytes.len() + 1);
    }

    #[test]
    fn parsing_event_shaped_lines_never_panics(lines in proptest::collection::vec(
        r#"\{"type": "(issue|fix|note|attempt)", "summary": "[a-z]{1,4}"(, "issue_id": "000[1-3]")?(, "supersedes": "evt_[0-9]")?(, "id": "evt_[0-9]")?(, "files": \["([a-z./]|\\\\){0,6}"\])?(, "location": "[a-z./:0-9]{0,8}")?\}"#,
        0..30,
    )) {
        let text = lines.join("\n");
        let log = EventLog::parse(text.as_bytes(), &clock(), &context(), &NoResolve);
        prop_assert_eq!(log.events().len(), lines.len());
    }
}
