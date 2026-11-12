use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use super::{CommitsSince, FileHistory, STALE_COMMIT_THRESHOLD, find_stale, stale_label};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::testing::{NoResolve, fixed_clock};

fn context() -> PathContext {
    PathContext {
        root: "/work/r".into(),
        cwd: "/work/r".into(),
        fold_case: false,
    }
}

struct Fake {
    files: HashMap<&'static str, Option<Vec<&'static str>>>,
    calls: Mutex<Vec<(String, Option<String>)>>,
}

impl Fake {
    fn new(files: &[(&'static str, Option<Vec<&'static str>>)]) -> Self {
        Self {
            files: files.iter().cloned().collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(String, Option<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl FileHistory for Fake {
    fn exists(&self, relative: &str) -> bool {
        self.files.contains_key(relative)
    }

    fn commit_times(&self, relative: &str, since: Option<&str>) -> Option<Vec<String>> {
        self.calls
            .lock()
            .unwrap()
            .push((relative.to_owned(), since.map(str::to_owned)));
        self.files
            .get(relative)?
            .as_ref()
            .map(|times| times.iter().map(|time| (*time).to_owned()).collect())
    }
}

fn event(id: &str, at: &str, location: &str, extra: &str) -> String {
    format!(
        r#"{{"id": "{id}", "type": "decision", "summary": "claim {id}", "timestamp": "{at}", "location": "{location}"{extra}}}"#
    )
}

fn log(lines: &[String]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

fn flagged(
    bytes: &[u8],
    history: &Fake,
    only: Option<&HashSet<&str>>,
) -> Vec<(String, String, CommitsSince)> {
    let log = EventLog::parse(bytes, &fixed_clock(), &context(), &NoResolve);
    find_stale(
        &log,
        &context(),
        &NoResolve,
        history,
        only,
        STALE_COMMIT_THRESHOLD,
    )
    .into_iter()
    .map(|stale| {
        (
            stale.event.id.as_str().to_owned(),
            stale.file,
            stale.commits_since,
        )
    })
    .collect()
}

const TIMES: [&str; 5] = [
    "2026-06-01T10:00:00Z",
    "2026-05-01T10:00:00Z",
    "2026-04-01T10:00:00Z",
    "2026-03-01T10:00:00Z",
    "2026-01-01T10:00:00Z",
];

#[test]
fn one_history_per_file_bounded_by_the_oldest_event() {
    let lines: Vec<String> = (0..40)
        .map(|index| {
            event(
                &format!("evt_{index:03}"),
                &format!("2026-01-0{}T10:00:0{}Z", index % 9 + 1, index % 9),
                "src/f.py",
                "",
            )
        })
        .collect();
    let history = Fake::new(&[("src/f.py", Some(TIMES.to_vec()))]);
    let found = flagged(&log(&lines), &history, None);
    assert_eq!(found.len(), 40);
    assert_eq!(
        history.calls(),
        [(
            "src/f.py".to_owned(),
            Some("2026-01-01T10:00:00Z".to_owned())
        )]
    );
}

#[test]
fn threshold_missing_files_and_unknown_history() {
    let lines = [
        event("old", "2026-01-15T10:00:00Z", "src/f.py", ""),
        event("recent", "2026-05-15T10:00:00Z", "src/f.py", ""),
        event("gone", "2026-01-15T10:00:00Z", "src/deleted.py", ""),
        event("nogit", "2026-01-15T10:00:00Z", "src/plain.py", ""),
    ];
    let history = Fake::new(&[("src/f.py", Some(TIMES.to_vec())), ("src/plain.py", None)]);
    let found = flagged(&log(&lines), &history, None);
    assert_eq!(
        found,
        [
            (
                "old".to_owned(),
                "src/f.py".to_owned(),
                CommitsSince::Count(4)
            ),
            (
                "gone".to_owned(),
                "src/deleted.py".to_owned(),
                CommitsSince::Missing
            ),
        ]
    );
    assert!(
        history
            .calls()
            .iter()
            .all(|(file, _)| file != "src/deleted.py")
    );
}

#[test]
fn only_files_restricts_the_work_and_the_answer() {
    let lines = [
        event("a", "2026-01-15T10:00:00Z", "src/f.py", ""),
        event("b", "2026-01-15T10:00:00Z", "src/other.py", ""),
    ];
    let history = Fake::new(&[
        ("src/f.py", Some(TIMES.to_vec())),
        ("src/other.py", Some(TIMES.to_vec())),
    ]);
    let only: HashSet<&str> = ["src/f.py"].into_iter().collect();
    let found = flagged(&log(&lines), &history, Some(&only));
    assert_eq!(found.len(), 1);
    assert_eq!(found.first().unwrap().1, "src/f.py");
    assert_eq!(history.calls().len(), 1);
}

#[test]
fn retired_attempts_and_auto_captured_records_are_skipped() {
    let lines = [
        event("old", "2026-01-15T10:00:00Z", "src/f.py", ""),
        event("new", "2026-07-01T10:00:00Z", "src/f.py", r#", "supersedes": "old""#),
        r#"{"id": "fix", "type": "fix", "summary": "Fix: x", "timestamp": "2026-02-01T10:00:00Z", "location": "src/f.py", "auto_captured": true, "capture_source": "git_post_commit"}"#.to_owned(),
        r#"{"id": "note", "type": "note", "summary": "New feature: x", "timestamp": "2026-02-01T10:00:00Z", "location": "src/f.py", "auto_captured": true}"#.to_owned(),
        r#"{"id": "auto", "type": "decision", "summary": "Refactor: x", "timestamp": "2026-02-01T10:00:00Z", "location": "src/f.py", "auto_captured": true}"#.to_owned(),
        r#"{"id": "try", "type": "attempt", "outcome": "failed", "summary": "t", "timestamp": "2026-02-01T10:00:00Z", "location": "src/f.py"}"#.to_owned(),
        event("summary", "2026-01-15T10:00:00Z", ".dejafix/summary.md", ""),
        event("prose", "2026-01-15T10:00:00Z", "deploy pipeline", ""),
    ];
    let history = Fake::new(&[
        ("src/f.py", Some(TIMES.to_vec())),
        (".dejafix/summary.md", Some(TIMES.to_vec())),
    ]);
    let ids: Vec<String> = flagged(&log(&lines), &history, None)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, ["auto"]);
}

#[test]
fn an_absolute_location_is_judged_project_relative() {
    let lines = [event(
        "abs",
        "2026-02-01T10:00:00Z",
        "/work/r/src/f.py:3",
        "",
    )];
    let history = Fake::new(&[("src/f.py", Some(TIMES.to_vec()))]);
    let only: HashSet<&str> = ["src/f.py"].into_iter().collect();
    let found = flagged(&log(&lines), &history, Some(&only));
    assert_eq!(
        found,
        [(
            "abs".to_owned(),
            "src/f.py".to_owned(),
            CommitsSince::Count(4)
        )]
    );
}

#[test]
fn labels_name_the_reason_and_the_fix() {
    let bytes = log(&[
        event("evt_1", "2026-01-15T10:00:00Z", "src/f.py", ""),
        event("evt_2", "2026-01-15T10:00:00Z", "gone.py", ""),
    ]);
    let history = Fake::new(&[("src/f.py", Some(TIMES.to_vec()))]);
    let log = EventLog::parse(&bytes, &fixed_clock(), &context(), &NoResolve);
    let labels: Vec<String> = find_stale(
        &log,
        &context(),
        &NoResolve,
        &history,
        None::<&HashSet<&str>>,
        3,
    )
    .iter()
    .map(stale_label)
    .collect();
    assert_eq!(
        labels,
        [
            "decision [evt_1] \"claim evt_1\" — predates 4 commits to src/f.py. Confirm, or retire with: djf decision \"...\" --supersedes evt_1",
            "decision [evt_2] \"claim evt_2\" — cited file gone.py no longer exists. Confirm, or retire with: djf decision \"...\" --supersedes evt_2",
        ]
    );
}
