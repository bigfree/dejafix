use serde_json::json;

use super::{Entry, Stack, gotcha_entry, is_gotcha_signal, promotable_set, promotion, similar};
use crate::core::event::{EventKind, Outcome};

fn libs(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn promote(
    summary: &str,
    kind: EventKind,
    outcome: Option<Outcome>,
    project: &[&str],
    existing: &[Entry],
) -> Option<String> {
    let promotable = promotable_set(&libs(&["tailwindcss", "gin"]));
    promotion(
        summary,
        kind,
        outcome,
        &promotable,
        &libs(project),
        existing,
    )
}

#[test]
fn only_lessons_are_signals() {
    assert!(is_gotcha_signal(
        "x",
        EventKind::Attempt,
        Some(Outcome::Failed)
    ));
    assert!(is_gotcha_signal(
        "x",
        EventKind::Attempt,
        Some(Outcome::Partial)
    ));
    assert!(!is_gotcha_signal(
        "x",
        EventKind::Attempt,
        Some(Outcome::Worked)
    ));
    assert!(!is_gotcha_signal("x", EventKind::Attempt, None));
    assert!(is_gotcha_signal("  GOTCHA: vite", EventKind::Note, None));
    assert!(is_gotcha_signal("Don't do it", EventKind::Decision, None));
    assert!(!is_gotcha_signal("Use FastAPI", EventKind::Decision, None));
    assert!(!is_gotcha_signal("gotcha: x", EventKind::Fix, None));
}

#[test]
fn a_failed_react_attempt_in_a_react_project_is_promoted() {
    assert_eq!(
        promote(
            "react strict mode double-invokes",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["react", "javascript"],
            &[]
        )
        .as_deref(),
        Some("react")
    );
}

#[test]
fn english_collisions_and_foreign_stacks_are_not_promoted() {
    assert_eq!(
        promote(
            "the next HMR reload",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["vite"],
            &[]
        ),
        None
    );
    assert_eq!(
        promote(
            "imagineering went wrong",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &[],
            &[]
        ),
        None
    );
    assert_eq!(
        promote(
            "Use FastAPI as the core",
            EventKind::Decision,
            None,
            &["fastapi"],
            &[]
        ),
        None
    );
}

#[test]
fn aliases_and_longest_match_win() {
    assert_eq!(
        promote(
            "gotcha: tailwindcss purge drops classes",
            EventKind::Note,
            None,
            &["tailwind"],
            &[]
        )
        .as_deref(),
        Some("tailwindcss")
    );
    assert_eq!(
        promote(
            "gotcha: next and react disagree",
            EventKind::Note,
            None,
            &[],
            &[]
        )
        .as_deref(),
        Some("react")
    );
}

#[test]
fn similar_gotchas_are_not_promoted_twice() {
    let existing: Vec<Entry> = vec![
        json!({"library": "react", "gotcha": "react strict mode double invokes effects"})
            .as_object()
            .cloned()
            .unwrap(),
    ];
    assert_eq!(
        promote(
            "react strict mode double invokes reducers",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &[],
            &existing
        ),
        None
    );
    assert!(
        promote(
            "react portals lose context",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &[],
            &existing
        )
        .is_some()
    );
    let broken: Vec<Entry> = vec![
        json!({"library": "react", "gotcha": 3})
            .as_object()
            .cloned()
            .unwrap(),
    ];
    assert_eq!(
        promote(
            "react portals lose context",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &[],
            &broken
        ),
        None
    );
    assert!(similar("a b c", "A B D"));
    assert!(!similar("a b c d e", "a b c x y"));
    assert!(!similar("", "a"));
}

#[test]
fn entries_have_python_fields() {
    let library = promotion(
        "gotcha: redis eviction",
        EventKind::Note,
        None,
        &promotable_set(&[]),
        &[],
        &[],
    )
    .unwrap();
    let entry = gotcha_entry(
        &library,
        "gotcha: redis eviction",
        "got_000000000001".into(),
        "2027-01-15T08:00:00.000001Z".into(),
        "proj",
    );
    assert_eq!(
        serde_json::Value::Object(entry),
        json!({"id": "got_000000000001", "library": "redis", "gotcha": "gotcha: redis eviction", "tags": [], "source_project": "proj", "version_range": null, "discovered": "2027-01-15T08:00:00.000001Z"})
    );
    let _ = Stack::default();
}
