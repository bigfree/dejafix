use std::collections::BTreeSet;
use std::fs;

use serde_json::{Value, json};

use super::types::SplitMix64;
use super::{FIXTURE_SEED, write_project};
use crate::core::json::write_python_json;

fn line(value: &Value) -> String {
    let mut out = Vec::new();
    write_python_json(&mut out, value).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn splitmix_matches_the_reference_sequence() {
    let mut rng = SplitMix64 { state: 0 };
    assert_eq!(rng.next(), 0xe220_a839_7b1d_cdaf);
    assert_eq!(rng.next(), 0x6e78_9e6a_a1b9_65f4);
}

#[test]
fn pick_stays_inside_the_slice() {
    let mut rng = SplitMix64 { state: 7 };
    let items = ["a", "b", "c"];
    assert!((0..1_000).all(|_| items.contains(&rng.pick(&items))));
    assert_eq!(rng.pick::<&str>(&[]), "");
}

#[test]
fn project_has_the_python_layout() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path(), 10, FIXTURE_SEED).unwrap();
    let mem = dir.path().join(".dejafix");
    assert_eq!(
        fs::read_to_string(mem.join("config.toml")).unwrap(),
        "recent_days = 30\nproject_description = \"\"\n"
    );
    assert!(mem.join("issues").is_dir());
    let log = fs::read_to_string(mem.join("events.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 10);
}

#[test]
fn same_seed_gives_the_same_bytes_and_another_seed_does_not() {
    let read = |seed| {
        let dir = tempfile::tempdir().unwrap();
        write_project(dir.path(), 300, seed).unwrap();
        fs::read(dir.path().join(".dejafix/events.jsonl")).unwrap()
    };
    assert_eq!(read(FIXTURE_SEED), read(FIXTURE_SEED));
    assert_ne!(read(FIXTURE_SEED), read(FIXTURE_SEED + 1));
}

#[test]
fn writing_into_a_file_reports_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "").unwrap();
    let error = write_project(&blocker, 1, FIXTURE_SEED).unwrap_err();
    assert!(error.to_string().contains("blocker"), "{error}");
}

#[test]
fn timestamps_stay_inside_the_window_for_any_size() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path(), 60_000, FIXTURE_SEED).unwrap();
    let text = fs::read_to_string(dir.path().join(".dejafix/events.jsonl")).unwrap();
    let last: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert!(last["timestamp"].as_str().unwrap() < "2026-01-01T00:00:01Z");
}

#[test]
fn every_event_is_valid_for_python() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path(), 3_000, FIXTURE_SEED).unwrap();
    let text = fs::read_to_string(dir.path().join(".dejafix/events.jsonl")).unwrap();
    let mut kinds = BTreeSet::new();
    for raw in text.lines() {
        let event: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(line(&event), raw, "not canonical: {raw}");
        let kind = event["type"].as_str().unwrap();
        assert!(["issue", "hypothesis", "attempt", "fix", "decision", "note"].contains(&kind));
        kinds.insert(kind.to_owned());
        let summary = event["summary"].as_str().unwrap();
        assert!(!summary.is_empty() && summary.trim() == summary);
        let id = event["id"].as_str().unwrap();
        assert!(id.len() == 24 && id.starts_with("evt_"));
        assert!(
            id[4..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        let at = event["timestamp"].as_str().unwrap();
        assert!(at.len() == 20 && at.ends_with('Z') && at.as_bytes()[10] == b'T');
        if let Some(outcome) = event.get("outcome") {
            assert!(["worked", "failed", "partial"].contains(&outcome.as_str().unwrap()));
        }
        let values = event.as_object().unwrap().values();
        assert!(
            values
                .into_iter()
                .all(|v| !v.is_null() && *v != json!(false) && *v != json!([]))
        );
    }
    assert_eq!(kinds.len(), 6);
    assert!(
        text.contains("\\u"),
        "fixture must exercise ensure_ascii escapes"
    );
    assert!(text.contains("\"supersedes\": "));
    assert!(text.contains("\"auto_captured\": true"));
    assert!(text.contains("\"x_fixture_extra\": "));
}
