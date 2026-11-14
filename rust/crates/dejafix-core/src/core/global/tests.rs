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

mod report {
    use serde_json::{Value, json};

    use crate::core::global::{Entry, GlobalOutcome, Stack, Usage, filtered, global_report};

    fn entries(values: &[Value]) -> Vec<Entry> {
        values
            .iter()
            .filter_map(|value| value.as_object().cloned())
            .collect()
    }

    fn patterns() -> Vec<Entry> {
        entries(&[
            json!({"id": "pat_1", "pattern": "keep it small", "tags": ["python", "web"], "source_project": "alpha", "confidence": "high"}),
            json!({"id": "pat_2", "pattern": "bare", "tags": [], "source_project": null}),
        ])
    }

    fn gotchas() -> Vec<Entry> {
        entries(&[
            json!({"id": "got_1", "library": "sqlalchemy", "gotcha": "close sessions", "tags": ["python"], "source_project": "beta", "version_range": ">=2"}),
            json!({"id": "got_2", "gotcha": "no library", "tags": []}),
        ])
    }

    fn list(tag: Option<&str>, library: Option<&str>, json: bool) -> GlobalOutcome {
        let (patterns, gotchas) = filtered(patterns(), gotchas(), tag, library);
        GlobalOutcome::List {
            patterns,
            gotchas,
            tag: tag.map(str::to_owned),
            library: library.map(str::to_owned),
            json,
        }
    }

    #[test]
    fn status_counts_and_sorts() {
        let outcome = GlobalOutcome::Status {
            location: "/home/x/.dejafix/global".to_owned(),
            patterns: patterns(),
            gotchas: gotchas(),
            preferences: true,
        };
        insta::assert_snapshot!(global_report(&outcome, false));
        insta::assert_snapshot!(global_report(&outcome, true));
    }

    #[test]
    fn status_of_nothing_omits_the_lists() {
        let outcome = GlobalOutcome::Status {
            location: "/g".to_owned(),
            patterns: Vec::new(),
            gotchas: Vec::new(),
            preferences: false,
        };
        let out = global_report(&outcome, false);
        assert!(out.contains("Stack preferences:   not set\n"));
        assert!(!out.contains("Tags:"));
        assert!(!out.contains("Projects:"));
    }

    #[test]
    fn list_text_with_and_without_colour() {
        insta::assert_snapshot!(global_report(&list(None, None, false), false));
        insta::assert_snapshot!(global_report(&list(None, None, false), true));
    }

    #[test]
    fn list_json_is_python_indent_two() {
        let out = global_report(&list(None, None, true), false);
        assert!(out.starts_with("{\n  \"patterns\": [\n    {\n      \"id\": \"pat_1\","));
        assert!(out.ends_with("  ]\n}\n"));
    }

    #[test]
    fn filters_by_tag_case_insensitively_and_by_library_substring() {
        let (patterns, gotchas) = filtered(self::patterns(), self::gotchas(), Some("PYTHON"), None);
        assert_eq!((patterns.len(), gotchas.len()), (1, 1));
        let (patterns, gotchas) = filtered(self::patterns(), self::gotchas(), None, Some("ALCH"));
        assert_eq!((patterns.len(), gotchas.len()), (2, 1));
        let (patterns, gotchas) = filtered(self::patterns(), self::gotchas(), Some(""), Some(""));
        assert_eq!((patterns.len(), gotchas.len()), (2, 2));
    }

    #[test]
    fn nothing_found_names_the_filter() {
        let out = global_report(&list(Some("zzz"), None, false), false);
        assert_eq!(
            out,
            "No entries found. (filter: tag=zzz, library=None)\n  Add patterns:  djf global add \"your pattern here\"\n  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library> --keyword <phrase> --keyword <phrase>\n"
        );
        let bare = global_report(&list(Some("zzz"), Some("nothing"), false), false);
        assert!(bare.starts_with("No entries found. (filter: tag=zzz, library=nothing)\n"));
    }

    #[test]
    fn short_messages_match_python() {
        let green = "\u{1b}[32m[global]\u{1b}[0m";
        let cases = [
            (
                GlobalOutcome::PatternAdded {
                    id: "pat_1".into(),
                    text: "t".into(),
                },
                format!("{green} Pattern added: pat_1\n  t\n"),
            ),
            (
                GlobalOutcome::GotchaAdded {
                    id: "got_1".into(),
                    library: "SQL".into(),
                    text: "t".into(),
                    warnings: vec!["it names the project 'alpha'".into()],
                },
                format!(
                    "{green} Gotcha added: got_1\n  [SQL] t\n  Warning: it names the project 'alpha'\n"
                ),
            ),
            (
                GlobalOutcome::GotchaRejected {
                    failures: vec!["a".into(), "b".into()],
                    warnings: vec!["c".into()],
                },
                "Not added: a; b.\n  Warning: c\n".to_owned(),
            ),
            (
                GlobalOutcome::Removed("x".into()),
                format!("{green} Removed: x\n"),
            ),
            (
                GlobalOutcome::Imported {
                    patterns: 2,
                    gotchas: 1,
                },
                format!("{green} Imported: 2 patterns, 1 gotchas\n"),
            ),
            (
                GlobalOutcome::Pruned {
                    removed: 3,
                    older_than: 365,
                },
                format!("{green} Pruned 3 entries (older than 365 days)\n"),
            ),
        ];
        for (outcome, expected) in cases {
            assert_eq!(global_report(&outcome, true), expected);
            assert_eq!(
                global_report(&outcome, false),
                expected.replace("\u{1b}[32m", "").replace("\u{1b}[0m", "")
            );
        }
        assert_eq!(
            global_report(&GlobalOutcome::NotFound("x".into()), true),
            "Entry not found: x\n"
        );
        assert_eq!(
            global_report(&GlobalOutcome::FileNotFound("f.json".into()), true),
            "File not found: f.json\n"
        );
        assert_eq!(
            global_report(&GlobalOutcome::InvalidJson("boom".into()), true),
            "Invalid JSON: boom\n"
        );
        assert_eq!(
            global_report(
                &GlobalOutcome::Pruned {
                    removed: 0,
                    older_than: 9
                },
                true
            ),
            "No entries to prune.\n"
        );
    }

    #[test]
    fn usage_and_unknown_lines() {
        let usage = |usage| global_report(&GlobalOutcome::Usage(usage), true);
        assert_eq!(
            usage(Usage::Add),
            "Usage: djf global add \"your pattern here\" [--tag python]\n"
        );
        assert_eq!(
            usage(Usage::AddGotcha),
            "Usage: djf global add-gotcha \"<text>\" --library <name> --keyword <phrase> --keyword <phrase> [--versions <range>]\nWrite one lesson in English, 30 to 600 characters, without project details; give 2 to 8 keywords.\n"
        );
        assert_eq!(usage(Usage::Remove), "Usage: djf global remove <id>\n");
        assert_eq!(
            usage(Usage::Import),
            "Usage: djf global import <file.json>\n"
        );
        assert_eq!(
            global_report(&GlobalOutcome::Unknown("frob".into()), true),
            "Unknown action: frob\nAvailable: status, list, add, add-gotcha, remove, export, import, prune, detect\n"
        );
    }

    #[test]
    fn export_is_ascii_json() {
        let value = json!({"version": "1.0", "preferences": "š"});
        assert_eq!(
            global_report(&GlobalOutcome::Exported(value), false),
            "{\n  \"version\": \"1.0\",\n  \"preferences\": \"\\u0161\"\n}\n"
        );
    }

    fn stack() -> Stack {
        Stack {
            tags: vec!["python".into(), "fastapi".into()],
            libraries: (1..=17).map(|n| format!("lib{n:02}")).collect(),
            frameworks: vec!["fastapi".into()],
            manifest_files: vec!["pyproject.toml", "Dockerfile"],
        }
    }

    fn detected(stack: Stack, patterns: usize, gotchas: usize, json: bool) -> GlobalOutcome {
        GlobalOutcome::Detected {
            name: "demo".into(),
            stack,
            inherited_patterns: patterns,
            inherited_gotchas: gotchas,
            json,
        }
    }

    #[test]
    fn detect_text_clips_libraries_at_fifteen() {
        insta::assert_snapshot!(global_report(&detected(stack(), 2, 1, false), false));
        insta::assert_snapshot!(global_report(&detected(stack(), 2, 1, false), true));
    }

    #[test]
    fn detect_of_an_empty_project_says_so() {
        let out = global_report(&detected(Stack::default(), 0, 0, false), false);
        assert_eq!(
            out,
            "\nDetected Stack: demo\n──────────────────────────────────────────────────\n  Tags:       (none detected)\n  Libraries:  (none detected)\n  Manifests:  \n\n  No matching global entries for this stack.\n\n"
        );
    }

    #[test]
    fn detect_json_leads_with_the_name() {
        let out = global_report(&detected(stack(), 0, 0, true), false);
        assert!(out.starts_with("{\n  \"name\": \"demo\",\n  \"tags\": [\n    \"python\","));
        assert!(out.contains(
            "  \"manifest_files\": [\n    \"pyproject.toml\",\n    \"Dockerfile\"\n  ]\n}\n"
        ));
    }
}

mod prune {
    use jiff::tz::TimeZone;
    use serde_json::{Value, json};

    use crate::core::global::{Entry, is_prunable};
    use crate::testing::fixed_clock;

    fn entry(value: &Value) -> Entry {
        value.as_object().cloned().unwrap_or_default()
    }

    fn prunable(value: &Value, older_than: i64, confidence: Option<&str>) -> bool {
        is_prunable(
            &entry(value),
            fixed_clock().now,
            &TimeZone::UTC,
            older_than,
            confidence,
        )
    }

    #[test]
    fn age_decides_and_the_boundary_is_strict() {
        let at = |text: &str| json!({"created": text});
        assert!(prunable(&at("2026-01-14T08:00:00Z"), 365, None));
        assert!(!prunable(&at("2026-01-15T08:00:00Z"), 365, None));
        assert!(!prunable(&at("2026-01-15T08:00:01Z"), 365, None));
        assert!(prunable(&at("2026-01-15T07:59:59Z"), 365, None));
    }

    #[test]
    fn discovered_is_read_when_created_is_missing_or_empty() {
        assert!(prunable(
            &json!({"discovered": "2020-01-01T00:00:00Z"}),
            365,
            None
        ));
        assert!(prunable(
            &json!({"created": "", "discovered": "2020-01-01T00:00:00Z"}),
            365,
            None
        ));
        assert!(!prunable(
            &json!({"created": "2099-01-01T00:00:00Z", "discovered": "2020-01-01T00:00:00Z"}),
            365,
            None
        ));
    }

    #[test]
    fn unreadable_dates_count_as_the_epoch() {
        assert!(prunable(&json!({}), 365, None));
        assert!(prunable(&json!({"created": "junk"}), 365, None));
        assert!(prunable(&json!({"created": 5}), 365, None));
        assert!(!prunable(&json!({}), 100_000, None));
    }

    #[test]
    fn confidence_narrows_the_match() {
        let old = json!({"created": "2020-01-01T00:00:00Z", "confidence": "high"});
        assert!(prunable(&old, 365, Some("high")));
        assert!(!prunable(&old, 365, Some("low")));
        assert!(!prunable(
            &json!({"created": "2020-01-01T00:00:00Z"}),
            365,
            Some("high")
        ));
    }
}

mod version_two {
    use serde_json::{Value, json};

    use super::super::{Entry, GotchaV2, Libraries, StoredGotcha, is_upgraded, line, parse_store};

    const SPEC_EXAMPLE: &str = r#"{"v": 2, "id": "got_6448df8c9d45", "library": "bullmq", "keywords": ["bun adapter", "reconnect", "duplicate", "raw client url"], "versions": ">=6.2.0 <6.3", "gotcha": "bullmq 6.2.0's Bun adapter rebuilds the raw client from this.raw.url at three sites", "source_project": "home-log", "discovered": "2026-08-27", "reviewed": "2026-11-14"}"#;

    fn entry(text: &str) -> Entry {
        serde_json::from_str::<Value>(text)
            .unwrap()
            .as_object()
            .cloned()
            .unwrap()
    }

    #[test]
    fn the_spec_example_round_trips_in_field_order() {
        let StoredGotcha::V2(record) = StoredGotcha::from_entry(entry(SPEC_EXAMPLE)) else {
            panic!("not read as version 2");
        };
        assert_eq!(record.library, "bullmq");
        assert_eq!(record.keywords.len(), 4);
        assert_eq!(record.versions.as_deref(), Some(">=6.2.0 <6.3"));
        assert_eq!(line(&record), SPEC_EXAMPLE);
    }

    #[test]
    fn a_line_without_v_reads_as_version_one() {
        let old = entry(
            r#"{"id": "got_1", "library": "react", "gotcha": "effects run twice", "tags": []}"#,
        );
        assert!(!is_upgraded(&old));
        let stored = StoredGotcha::from_entry(old);
        assert!(!stored.is_upgraded());
        assert_eq!(stored.id(), "got_1");
        assert_eq!(stored.library(), "react");
        assert_eq!(stored.text(), "effects run twice");
        let broken = entry(
            r#"{"v": 2, "id": "got_2", "library": "react", "keywords": "one", "gotcha": "x"}"#,
        );
        assert!(matches!(
            StoredGotcha::from_entry(broken),
            StoredGotcha::V1(_)
        ));
        let empty = StoredGotcha::from_entry(Entry::new());
        assert_eq!((empty.id(), empty.library(), empty.text()), ("", "", ""));
    }

    #[test]
    fn unknown_fields_of_a_version_two_line_are_kept() {
        let with_extra = entry(
            r#"{"v": 2, "id": "got_3", "library": "zod", "keywords": ["a", "b"], "versions": null, "gotcha": "text", "note": {"by": "hand"}}"#,
        );
        let records: [StoredGotcha; 1] = parse_store(vec![with_extra]).try_into().unwrap();
        let [StoredGotcha::V2(record)] = records else {
            panic!("not read as version 2");
        };
        assert_eq!(record.extra.get("note"), Some(&json!({"by": "hand"})));
        assert_eq!(
            line(&record),
            r#"{"v": 2, "id": "got_3", "library": "zod", "keywords": ["a", "b"], "versions": null, "gotcha": "text", "note": {"by": "hand"}}"#
        );
        let minimal = GotchaV2 {
            id: "got_4".into(),
            library: "nx".into(),
            gotcha: "g".into(),
            ..GotchaV2::default()
        };
        assert_eq!(
            line(&minimal),
            r#"{"v": 2, "id": "got_4", "library": "nx", "keywords": [], "versions": null, "gotcha": "g"}"#
        );
    }

    #[test]
    fn aliases_resolve_to_the_canonical_library() {
        let mut libraries =
            Libraries::from_value(&json!({"next": ["nextjs", "Next.js"], "nx": ["@nx/js"]}))
                .unwrap();
        assert_eq!(libraries.canonical("nextjs"), "next");
        assert_eq!(libraries.canonical(" NEXT.JS "), "next");
        assert_eq!(libraries.canonical("@nx/js"), "nx");
        assert_eq!(libraries.canonical("Prisma"), "prisma");
        assert_eq!(libraries.aliases("next"), ["nextjs", "next.js"]);
        assert_eq!(libraries.aliases("prisma"), Vec::<String>::new());
        assert!(libraries.add_library("prisma"));
        assert!(!libraries.add_library("prisma"));
        assert!(libraries.add_alias("prisma", "Prisma-Client"));
        assert!(!libraries.add_alias("prisma", "prisma-client"));
        assert!(!libraries.add_alias("prisma", "prisma"));
        assert_eq!(libraries.canonical("prisma-client"), "prisma");
        assert_eq!(
            libraries.to_value(),
            json!({"next": ["nextjs", "next.js"], "nx": ["@nx/js"], "prisma": ["prisma-client"]})
        );
        assert!(Libraries::from_value(&json!(["next"])).is_none());
        assert!(Libraries::from_value(&json!({"next": "nextjs"})).is_none());
        assert!(Libraries::from_value(&json!({"next": [1]})).is_none());
    }
}

mod checks {
    use serde_json::json;

    use super::super::{
        CheckFailure, CheckWarning, GotchaInput, Libraries, StoredGotcha, check_gotcha,
        normalize_input,
    };

    const GOOD: &str =
        "bullmq 6.2 Worker.close() never resolves when Redis is unreachable; pass a timeout.";

    fn input(library: &str, gotcha: &str, keywords: &[&str]) -> GotchaInput {
        GotchaInput {
            library: library.to_owned(),
            keywords: keywords
                .iter()
                .map(|keyword| (*keyword).to_owned())
                .collect(),
            gotcha: gotcha.to_owned(),
            versions: None,
        }
    }

    fn report(gotcha: &str) -> (Vec<CheckFailure>, Vec<CheckWarning>) {
        let report = check_gotcha(
            &input("bullmq", gotcha, &["shutdown", "worker close"]),
            &Libraries::default(),
            &["home-log", "cra-portal"],
            &[],
            None,
        );
        (report.failures, report.warnings)
    }

    fn failures(gotcha: &str) -> Vec<CheckFailure> {
        report(gotcha).0
    }

    fn padded(text: &str) -> String {
        format!("{text} and this sentence makes the lesson long enough to pass.")
    }

    #[test]
    fn a_clean_record_passes() {
        assert_eq!(report(GOOD), (Vec::new(), Vec::new()));
    }

    #[test]
    fn library_keyword_and_length_rules() {
        let check = |library: &str, gotcha: &str, keywords: &[&str]| {
            check_gotcha(
                &input(library, gotcha, keywords),
                &Libraries::default(),
                &[],
                &[],
                None,
            )
            .failures
        };
        assert_eq!(check("", GOOD, &["a", "b"]), [CheckFailure::Library]);
        assert_eq!(check("Bull MQ", GOOD, &["a", "b"]), [CheckFailure::Library]);
        assert_eq!(
            check("@nx/js", GOOD, &["a", "b"]),
            Vec::<CheckFailure>::new()
        );
        assert_eq!(
            check("next.js_x-1", GOOD, &["a", "b"]),
            Vec::<CheckFailure>::new()
        );
        assert_eq!(check("zod", GOOD, &["a"]), [CheckFailure::KeywordCount(1)]);
        assert_eq!(
            check("zod", GOOD, &["a", "b", "c", "d", "e", "f", "g", "h", "i"]),
            [CheckFailure::KeywordCount(9)]
        );
        assert_eq!(check("zod", GOOD, &["a", ""]), [CheckFailure::EmptyKeyword]);
        assert_eq!(
            check("zod", GOOD, &["a", "one two three four five"]),
            [CheckFailure::LongKeyword("one two three four five".into())]
        );
        assert_eq!(
            check("zod", GOOD, &["a", "one two three four"]),
            Vec::<CheckFailure>::new()
        );
        assert_eq!(
            check("zod", "too short to be a lesson", &["a", "b"]),
            [CheckFailure::Length(24)]
        );
        assert_eq!(
            check("zod", &"a lesson ".repeat(70), &["a", "b"]),
            [CheckFailure::Length(630)]
        );
    }

    #[test]
    fn other_languages_are_refused_and_terse_text_warned() {
        assert_eq!(
            failures("bullmq Worker.close() sa nikdy neukončí, keď Redis nie je dostupný."),
            [CheckFailure::NotEnglish(
                "too many letters outside ASCII".into()
            )]
        );
        let (_, warnings) =
            report("worker close redis unreachable timeout hang forever restart pod again");
        assert_eq!(warnings, [CheckWarning::Doubtful]);
        let keywords = check_gotcha(
            &input("bullmq", GOOD, &["zatvorenie", "Работник"]),
            &Libraries::default(),
            &[],
            &[],
            None,
        );
        assert_eq!(keywords.failures.len(), 1);
    }

    #[test]
    fn project_paths_are_refused_but_library_paths_pass() {
        assert_eq!(
            failures(&padded("A note citing `apps/heating/` and its config")),
            [CheckFailure::ProjectPath("apps/heating/".into())]
        );
        for path in [
            "/Users/someone/repo/file.ts",
            "/home/ci/build",
            "~/.config/tool",
            "C:\\work\\repo",
            "libs/shared/util.ts",
            "packages/api/src",
            "tools/scripts/run.sh",
        ] {
            assert!(
                matches!(
                    failures(&padded(&format!("It reads {path} first"))).as_slice(),
                    [CheckFailure::ProjectPath(_)]
                ),
                "{path}"
            );
        }
        for path in [
            "node_modules/excalibur/build/dist/*.d.ts",
            "drizzle/meta/*.json",
            "src/apps/x",
            "@nx/js",
        ] {
            assert_eq!(
                failures(&padded(&format!("It reads {path} first"))),
                [],
                "{path}"
            );
        }
    }

    #[test]
    fn ticket_numbers_are_refused_with_the_standard_exceptions() {
        assert_eq!(
            failures(&padded("Fixed in ECHO-3233 after the upgrade")),
            [CheckFailure::Ticket("ECHO-3233".into())]
        );
        assert_eq!(
            failures(&padded("See issue #1234 upstream for details")),
            [CheckFailure::Ticket("#1234".into())]
        );
        for fine in [
            "UTF-8 input",
            "ISO-8601 dates",
            "SHA-256 hashes",
            "RFC-9110 says",
            "CVE-2024-1234",
            "TLS-13 only",
            "IEEE-754 floats",
            "ECMA-262 spec",
            "#12 is short",
            "AB-12 too short",
            "ECHO-1 too short",
            "ECHOs-123 mixed",
            "xECHO-123 glued",
        ] {
            assert_eq!(failures(&padded(fine)), [], "{fine}");
        }
    }

    #[test]
    fn internal_hosts_and_private_addresses_are_refused() {
        for host in [
            "api.int",
            "db.local",
            "svc.internal",
            "nas.lan",
            "git.corp",
            "router.home",
            "10.1.2.3",
            "172.16.0.9",
            "172.31.255.255",
            "192.168.1.10",
        ] {
            assert_eq!(
                failures(&padded(&format!("It calls {host}:8080 at start"))),
                [CheckFailure::InternalHost(host.into())],
                "{host}"
            );
        }
        for fine in [
            "example.com",
            "localhost",
            "app.localhost",
            "172.32.0.1",
            "8.8.8.8",
            "1.2.3",
            "256.1.1.1",
            "v1.2.3.4.5",
        ] {
            assert_eq!(
                failures(&padded(&format!("It calls {fine} at start"))),
                [],
                "{fine}"
            );
        }
    }

    #[test]
    fn project_names_and_run_statuses_only_warn() {
        let (failures, warnings) = report(&padded("Retry of test docker-image-base failed"));
        assert_eq!(failures, []);
        assert_eq!(warnings, [CheckWarning::RunStatus("retry of".into())]);
        let (_, warnings) = report(&padded("Seen in home-log while deploying"));
        assert_eq!(warnings, [CheckWarning::ProjectName("home-log".into())]);
        for status in [
            "not pushed yet",
            "awaiting review",
            "was reverted",
            "Task 7 is done",
            "in round 2",
        ] {
            assert_eq!(report(&padded(status)).1.len(), 1, "{status}");
        }
        assert_eq!(report(&padded("the task scheduler restarts")).1, []);
    }

    #[test]
    fn a_reworded_duplicate_of_the_same_library_is_refused() {
        let existing = [
            StoredGotcha::from_entry(
                json!({"v": 2, "id": "got_aaaaaaaaaaaa", "library": "bullmq", "keywords": ["shutdown", "worker close"], "versions": null, "gotcha": GOOD})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
            StoredGotcha::from_entry(
                json!({"id": "got_bbbbbbbbbbbb", "library": "redis", "gotcha": GOOD})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        ];
        let reordered = input(
            "bullmq",
            "When Redis is unreachable, bullmq 6.2 Worker.close() never resolves: pass a timeout.",
            &["worker close", "shutdown"],
        );
        let report = check_gotcha(&reordered, &Libraries::default(), &[], &existing, None);
        assert_eq!(report.duplicate_of.as_deref(), Some("got_aaaaaaaaaaaa"));
        assert_eq!(
            report.failures,
            [CheckFailure::Duplicate("got_aaaaaaaaaaaa".into())]
        );
        let replacing = check_gotcha(
            &reordered,
            &Libraries::default(),
            &[],
            &existing,
            Some("got_aaaaaaaaaaaa"),
        );
        assert_eq!(replacing.failures, []);
        let other = input(
            "bullmq",
            "bullmq repeatable jobs keep their old cron pattern after a deploy; remove them first.",
            &["repeatable jobs", "cron"],
        );
        assert_eq!(
            check_gotcha(&other, &Libraries::default(), &[], &existing, None).failures,
            []
        );
    }

    #[test]
    fn input_is_normalised_before_checking() {
        let libraries = Libraries::from_value(&json!({"next": ["nextjs"]})).unwrap();
        let normalised = normalize_input(
            GotchaInput {
                library: " NextJS ".into(),
                keywords: vec!["  app   router ".into()],
                gotcha: "  text  ".into(),
                versions: Some("  ".into()),
            },
            &libraries,
        );
        assert_eq!(
            normalised,
            GotchaInput {
                library: "next".into(),
                keywords: vec!["app router".into()],
                gotcha: "text".into(),
                versions: None,
            }
        );
    }
}
