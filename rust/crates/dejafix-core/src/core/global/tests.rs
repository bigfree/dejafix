use serde_json::json;

use super::{
    Entry, Stack, gotcha_entry, inherited_block, is_gotcha_signal, promotable_set, promotion,
    relevant_entries, similar,
};
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

fn entry(value: &serde_json::Value) -> Entry {
    value.as_object().unwrap().clone()
}

fn web_stack() -> Stack {
    Stack {
        tags: libs(&["javascript", "react"]),
        libraries: libs(&["react", "vite"]),
        ..Stack::default()
    }
}

#[test]
fn patterns_match_by_tag_and_gotchas_by_library() {
    let patterns = vec![
        entry(&json!({"pattern": "everywhere", "tags": []})),
        entry(&json!({"pattern": "untagged"})),
        entry(&json!({"pattern": "react only", "tags": ["react"]})),
        entry(&json!({"pattern": "python only", "tags": ["python"]})),
        entry(&json!({"pattern": "odd tags", "tags": [1]})),
        entry(&json!({"pattern": "null tags", "tags": null})),
    ];
    let gotchas = vec![
        entry(&json!({"library": "React", "gotcha": "a"})),
        entry(&json!({"library": "vite", "gotcha": "b"})),
        entry(&json!({"library": "django", "gotcha": "c"})),
        entry(&json!({"library": 5, "gotcha": "d"})),
        entry(&json!({"gotcha": "no library"})),
    ];
    let relevant = relevant_entries(&web_stack(), None, &patterns, &gotchas);
    let names: Vec<&str> = relevant
        .patterns
        .iter()
        .map(|p| p["pattern"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["everywhere", "untagged", "react only"]);
    let libraries: Vec<&str> = relevant
        .gotchas
        .iter()
        .map(|g| g["gotcha"].as_str().unwrap())
        .collect();
    assert_eq!(libraries, ["a", "b"]);
}

#[test]
fn filter_tags_replace_the_project_tags() {
    let patterns = vec![
        entry(&json!({"pattern": "react only", "tags": ["react"]})),
        entry(&json!({"pattern": "python only", "tags": ["python"]})),
        entry(&json!({"pattern": "chars", "tags": "py"})),
        entry(&json!({"pattern": "keys", "tags": {"python": 1}})),
    ];
    let gotchas = vec![entry(&json!({"library": "python", "gotcha": "tag match"}))];
    let filter = libs(&["python", "p"]);
    let relevant = relevant_entries(&web_stack(), Some(&filter), &patterns, &gotchas);
    let names: Vec<&str> = relevant
        .patterns
        .iter()
        .map(|p| p["pattern"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["python only", "chars", "keys"]);
    assert_eq!(relevant.gotchas.len(), 1);
    let unfiltered = relevant_entries(&web_stack(), Some(&[]), &patterns, &gotchas);
    assert_eq!(unfiltered.patterns.len(), 1);
}

#[test]
fn nothing_relevant_builds_no_block() {
    let relevant = relevant_entries(&web_stack(), None, &[], &[]);
    assert_eq!(inherited_block(&relevant), None);
}

#[test]
fn the_block_renders_like_python() {
    let patterns = vec![
        entry(&json!({"pattern": "Prefer small PRs", "source_project": "api"})),
        entry(&json!({"pattern": 1.5, "source_project": ""})),
    ];
    let gotchas = vec![
        entry(
            &json!({"library": "react", "gotcha": "strict mode mounts twice", "source_project": "web", "version_range": ">=18"}),
        ),
        entry(
            &json!({"library": "vite", "version_range": null, "gotcha": null, "source_project": 0}),
        ),
        entry(&json!({"library": "vite", "gotcha": ["x"], "source_project": true})),
    ];
    let relevant = relevant_entries(&web_stack(), None, &patterns, &gotchas);
    assert_eq!(
        inherited_block(&relevant).unwrap(),
        "## Global Memory — Inherited Knowledge\n\nThe following patterns and gotchas were inherited from your global memory\n(~/.dejafix/global/). They represent lessons learned from other projects.\n\n### Library Gotchas\n\n- **react** [>=18]: strict mode mounts twice (from web)\n- **vite**: None\n- **vite**: [\"x\"] (from True)\n\n### Cross-Project Patterns\n\n- Prefer small PRs (from api)\n- 1.5\n"
    );
    let only_patterns = relevant_entries(&Stack::default(), None, &patterns, &[]);
    let block = inherited_block(&only_patterns).unwrap();
    assert!(!block.contains("### Library Gotchas"));
    assert!(block.ends_with("- 1.5\n"));
}
