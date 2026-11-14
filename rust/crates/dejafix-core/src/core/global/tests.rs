use serde_json::json;

use super::{Entry, GlobalHint, Stack, global_hint, hint_candidate, relevant_entries};
use crate::core::event::{EventKind, Outcome};

fn libs(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn hint(
    summary: &str,
    kind: EventKind,
    outcome: Option<Outcome>,
    stack: &[&str],
) -> Option<String> {
    global_hint(
        summary,
        hint_candidate(summary, kind, outcome),
        &libs(stack),
    )
}

fn hint_for(library: &str) -> String {
    format!(
        "If this applies beyond this project, also call add_global_gotcha(library=\"{library}\", ...) in English without project details."
    )
}

#[test]
fn only_lessons_and_failed_attempts_are_candidates() {
    for (summary, kind, outcome, expected) in [
        ("gotcha: x", EventKind::Note, None, GlobalHint::AnyLibrary),
        (
            "  Lesson: x",
            EventKind::Decision,
            None,
            GlobalHint::AnyLibrary,
        ),
        ("never do x", EventKind::Note, None, GlobalHint::AnyLibrary),
        ("plain note", EventKind::Note, None, GlobalHint::None),
        (
            "x",
            EventKind::Attempt,
            Some(Outcome::Failed),
            GlobalHint::NamedLibrary,
        ),
        (
            "x",
            EventKind::Attempt,
            Some(Outcome::Partial),
            GlobalHint::NamedLibrary,
        ),
        (
            "x",
            EventKind::Attempt,
            Some(Outcome::Worked),
            GlobalHint::None,
        ),
        ("gotcha: x", EventKind::Issue, None, GlobalHint::None),
        ("gotcha: x", EventKind::Fix, None, GlobalHint::None),
    ] {
        assert_eq!(
            hint_candidate(summary, kind, outcome),
            expected,
            "{summary}"
        );
    }
}

#[test]
fn a_failed_attempt_hints_only_when_it_names_a_stack_library() {
    assert_eq!(
        hint(
            "vite dev server kept the old env",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["react", "vite"]
        ),
        Some(hint_for("vite"))
    );
    assert_eq!(
        hint(
            "the server kept the old env",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["react", "vite"]
        ),
        None
    );
    assert_eq!(
        hint(
            "react-router and React both",
            EventKind::Attempt,
            Some(Outcome::Partial),
            &["react", "react-router"]
        ),
        Some(hint_for("react-router"))
    );
}

#[test]
fn scoped_package_names_are_named_in_the_hint_but_not_inside_other_names() {
    assert_eq!(
        hint(
            "@nx/js build caches stale output",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["@nx/js"]
        ),
        Some(hint_for("@nx/js"))
    );
    assert_eq!(
        hint(
            "react-router lost the state",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["react"]
        ),
        None
    );
    assert_eq!(
        hint(
            "vite.config breaks again",
            EventKind::Attempt,
            Some(Outcome::Failed),
            &["vite"]
        ),
        Some(hint_for("vite"))
    );
}

#[test]
fn a_lesson_note_hints_with_the_named_library_or_a_placeholder() {
    assert_eq!(
        hint(
            "gotcha: Prisma drops the index",
            EventKind::Note,
            None,
            &["prisma"]
        ),
        Some(hint_for("prisma"))
    );
    assert_eq!(
        hint(
            "gotcha: the cache is per worker",
            EventKind::Note,
            None,
            &[]
        ),
        Some(hint_for("<library>"))
    );
    assert_eq!(
        hint(
            "the cache is per worker",
            EventKind::Note,
            None,
            &["prisma"]
        ),
        None
    );
    assert_eq!(global_hint("x", GlobalHint::None, &libs(&["x"])), None);
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
            "Unknown action: frob\nAvailable: status, list, add, add-gotcha, query, edit, upgrade, remove, export, import, prune, detect\n"
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

mod query {
    use serde_json::{Value, json};

    use super::super::{GlobalIndex, Libraries, StoredGotcha, global_reply, parse_store};

    fn store() -> GlobalIndex {
        let lines = [
            json!({"v": 2, "id": "got_33f8cfe2c2d5", "library": "bullmq", "keywords": ["shutdown", "worker close", "sigterm"], "versions": ">=6.2.0 <6.3", "gotcha": "bullmq 6.2.0 Worker.close() never resolves against an unreachable Redis, so the process never exits on SIGTERM; pass a timeout.", "source_project": "home-log", "discovered": "2026-08-27", "reviewed": "2026-11-14"}),
            json!({"v": 2, "id": "got_000000000002", "library": "bullmq", "keywords": ["repeatable jobs", "cron"], "versions": null, "gotcha": "bullmq repeatable jobs keep their old cron pattern after a deploy until they are removed.", "source_project": "project-b", "discovered": "2026-09-01"}),
            json!({"v": 2, "id": "got_000000000003", "library": "next", "keywords": ["app router", "fetch cache"], "versions": "15", "gotcha": "next 15 app router caches fetch results by default in production; pass cache: 'no-store'."}),
            json!({"id": "got_000000000004", "library": "nextjs", "gotcha": "next dev server ignores the custom server file when started with turbo.", "source_project": "web"}),
        ];
        let entries = lines.iter().filter_map(Value::as_object).cloned().collect();
        let libraries = Libraries::from_value(&json!({"next": ["nextjs", "next.js"]})).unwrap();
        GlobalIndex::build(parse_store(entries), libraries)
    }

    #[test]
    fn a_query_replies_in_the_spec_format() {
        insta::assert_snapshot!(global_reply(
            &store(),
            "worker never exits on sigterm",
            None,
            2_000
        ));
    }

    #[test]
    fn the_budget_cuts_but_keeps_the_first_record() {
        let reply = global_reply(&store(), "", None, 1);
        assert!(
            reply.starts_with(
                "Global gotchas (1 of 4 shown):\n- got_000000000002 · bullmq · from project-b\n"
            ),
            "{reply}"
        );
        assert_eq!(reply.lines().count(), 3);
        let all = global_reply(&store(), "", None, 8_000);
        assert!(all.starts_with("Global gotchas (4 of 4 shown):"));
    }

    #[test]
    fn the_library_filter_resolves_aliases() {
        let reply = global_reply(&store(), "", Some("Next.js"), 2_000);
        assert_eq!(
            reply,
            "Global gotchas of Next.js (2 of 2 shown):\n- got_000000000003 · next 15\n  next 15 app router caches fetch results by default in production; pass cache: 'no-store'.\n- got_000000000004 · nextjs · from web, not upgraded\n  next dev server ignores the custom server file when started with turbo."
        );
        let query = global_reply(&store(), "dev server turbo", Some("next"), 2_000);
        assert!(query.starts_with("Global gotchas for \"dev server turbo\" in next (1 of 1 shown):\n- got_000000000004"), "{query}");
    }

    #[test]
    fn a_library_named_in_the_query_scores_but_does_not_filter() {
        let reply = global_reply(&store(), "bullmq cron deploy", None, 2_000);
        assert!(reply.contains("- got_000000000002 · bullmq"), "{reply}");
        assert!(
            reply.starts_with("Global gotchas for \"bullmq cron deploy\" (1 of 1 shown)"),
            "{reply}"
        );
    }

    #[test]
    fn no_match_names_the_libraries_with_counts() {
        assert_eq!(
            global_reply(&store(), "kubernetes ingress certificate", None, 2_000),
            "No global gotcha matches this query.\nLibraries with records: bullmq (2), next (2)."
        );
        assert_eq!(
            global_reply(&store(), "", Some("prisma"), 2_000),
            "No global gotcha matches this query.\nLibraries with records: bullmq (2), next (2)."
        );
        assert_eq!(
            global_reply(&GlobalIndex::default(), "anything", None, 2_000),
            "Global memory is empty. Add a lesson with add_global_gotcha."
        );
    }

    #[test]
    fn version_one_records_are_found_by_their_text() {
        let reply = global_reply(&store(), "custom server file", None, 2_000);
        assert!(
            reply.contains("- got_000000000004 · nextjs · from web, not upgraded"),
            "{reply}"
        );
        assert!(matches!(
            parse_store(vec![json!({"id": "x"}).as_object().cloned().unwrap()]).first(),
            Some(StoredGotcha::V1(_))
        ));
    }
}

mod upgrade {
    use serde_json::{Value, json};

    use super::super::{
        CheckFailure, CheckReport, Draft, DraftError, Entry, GotchaInput, Libraries, StoredGotcha,
        apply_upgrade, export_notes, legacy_input, line, parse_drafts, parse_store,
        touches_version_one, upgrade_drafts,
    };

    const LESSON: &str =
        "BullMQ Worker.close() never resolves when Redis is unreachable; pass a timeout.";

    fn entry(value: &Value) -> Entry {
        value.as_object().cloned().unwrap()
    }

    fn v1(id: &str, library: &str, text: &str) -> Entry {
        entry(&json!({
            "id": id,
            "library": library,
            "gotcha": text,
            "source_project": "home-log",
            "discovered": "2026-08-27T10:00:00Z",
            "tags": ["node"],
            "version_range": ">=6",
        }))
    }

    fn v2_entry(id: &str) -> Entry {
        entry(&json!({
            "v": 2, "id": id, "library": "zod", "keywords": ["a", "b"], "versions": null,
            "gotcha": "Zod parse results drop unknown keys unless strict is used.",
            "discovered": "2026-01-01",
        }))
    }

    fn store() -> Vec<StoredGotcha> {
        parse_store(vec![
            v1("got_1", "bull", "stará poznámka"),
            v2_entry("got_2"),
            v1("got_3", "react", "effects run twice"),
        ])
    }

    fn draft(id: &str, library: &str, gotcha: &str, keywords: &[&str], drop: bool) -> Draft {
        Draft {
            id: id.to_owned(),
            input: GotchaInput {
                library: library.to_owned(),
                keywords: keywords
                    .iter()
                    .map(|keyword| (*keyword).to_owned())
                    .collect(),
                gotcha: gotcha.to_owned(),
                versions: None,
            },
            drop,
        }
    }

    fn passing(_: &str, _: &GotchaInput) -> CheckReport {
        CheckReport::default()
    }

    fn libraries() -> Libraries {
        Libraries::from_value(&json!({"bullmq": ["bull", "bullmq-pro"]})).unwrap()
    }

    #[test]
    fn export_lists_only_version_one_records_with_their_notes() {
        let lines = upgrade_drafts(&store(), |entry| {
            vec![format!("note for {}", legacy_input(entry).library)]
        });
        assert_eq!(lines.len(), 3);
        let header: Value = serde_json::from_str(lines.first().unwrap()).unwrap();
        assert!(header["instructions"].as_str().unwrap().contains("English"));
        assert_eq!(header.as_object().unwrap().len(), 1);
        let first: Value = serde_json::from_str(lines.get(1).unwrap()).unwrap();
        assert_eq!(
            first,
            json!({
                "id": "got_1", "library": "bull", "source_project": "home-log",
                "discovered": "2026-08-27T10:00:00Z", "original": "stará poznámka",
                "gotcha": "", "keywords": [], "versions": ">=6", "drop": false,
                "checks": ["note for bull"],
            })
        );
        assert!(lines.get(1).unwrap().contains("stará"));
        assert!(lines.get(2).unwrap().contains("got_3"));
        assert!(!lines.join("\n").contains("got_2"));
    }

    #[test]
    fn export_notes_leave_out_what_every_version_one_record_lacks() {
        let report = CheckReport {
            failures: vec![
                CheckFailure::KeywordCount(0),
                CheckFailure::EmptyKeyword,
                CheckFailure::Length(5),
            ],
            warnings: Vec::new(),
            duplicate_of: None,
        };
        assert_eq!(
            export_notes(&report),
            ["the gotcha must be 30 to 600 characters long, not 5"]
        );
    }

    #[test]
    fn a_draft_file_is_parsed_and_its_header_skipped() {
        let text = format!(
            "{}\n\n{}\n{}\n",
            r#"{"instructions": "fill in"}"#,
            json!({"id": "got_1", "library": "BullMQ", "original": "x", "gotcha": LESSON,
                   "keywords": ["shutdown", "close"], "versions": ">=6", "drop": false,
                   "checks": ["n"], "source_project": "x"}),
            json!({"id": "got_3", "drop": true}),
        );
        let drafts = parse_drafts(&text).unwrap();
        assert_eq!(drafts.len(), 2);
        assert_eq!(
            drafts.first().unwrap(),
            &Draft {
                id: "got_1".into(),
                input: GotchaInput {
                    library: "BullMQ".into(),
                    keywords: vec!["shutdown".into(), "close".into()],
                    gotcha: LESSON.into(),
                    versions: Some(">=6".into()),
                },
                drop: false,
            }
        );
        assert!(drafts.get(1).unwrap().drop);
        assert_eq!(parse_drafts("").unwrap(), Vec::new());
    }

    #[test]
    fn malformed_lines_are_reported_with_their_line_numbers() {
        let text = [
            r#"{"instructions": "x"}"#,
            "not json",
            "[1]",
            r#"{"library": "x"}"#,
            r#"{"id": ""}"#,
            r#"{"id": 4}"#,
            r#"{"id": "a", "keywords": "one"}"#,
            r#"{"id": "b", "keywords": [1]}"#,
            r#"{"id": "c", "drop": "yes"}"#,
            r#"{"id": "d", "gotcha": 5}"#,
            r#"{"id": "e", "library": ["x"]}"#,
            r#"{"id": "f", "versions": 2}"#,
            r#"{"id": "g"}"#,
            r#"{"id": "g"}"#,
        ]
        .join("\n");
        let errors = parse_drafts(&text).unwrap_err();
        let wrong = |line, field| DraftError::WrongType { line, field };
        assert!(matches!(
            errors.first(),
            Some(DraftError::Json { line: 2, .. })
        ));
        assert_eq!(
            errors.get(1..).unwrap(),
            [
                DraftError::NotObject { line: 3 },
                DraftError::MissingId { line: 4 },
                DraftError::MissingId { line: 5 },
                wrong(6, "id"),
                wrong(7, "keywords"),
                wrong(8, "keywords"),
                wrong(9, "drop"),
                wrong(10, "gotcha"),
                wrong(11, "library"),
                wrong(12, "versions"),
                DraftError::DuplicateId {
                    line: 14,
                    id: "g".into()
                },
            ]
        );
        assert_eq!(
            errors.get(1).unwrap().to_string(),
            "line 3: not a JSON object"
        );
    }

    #[test]
    fn an_accepted_draft_replaces_the_record_and_keeps_its_origin() {
        let records = store();
        let drafts = [draft(
            "got_1",
            "Bull",
            LESSON,
            &["shutdown", "close"],
            false,
        )];
        let outcome = apply_upgrade(&records, &drafts, "2026-11-14", &libraries(), passing);
        assert_eq!((outcome.replaced, outcome.dropped), (1, 0));
        assert_eq!(outcome.kept, Vec::<(String, Vec<String>)>::new());
        assert_eq!(outcome.libraries, ["bullmq"]);
        assert_eq!(outcome.lines.len(), 3);
        let replaced: Value = serde_json::from_str(outcome.lines.first().unwrap()).unwrap();
        assert_eq!(
            replaced,
            json!({
                "v": 2, "id": "got_1", "library": "bullmq",
                "keywords": ["shutdown", "close"], "versions": null, "gotcha": LESSON,
                "source_project": "home-log", "discovered": "2026-08-27T10:00:00Z",
                "reviewed": "2026-11-14", "tags": ["node"],
            })
        );
        assert!(
            outcome
                .lines
                .first()
                .unwrap()
                .starts_with(r#"{"v": 2, "id": "got_1", "library": "bullmq", "keywords""#)
        );
        let StoredGotcha::V2(untouched) = records.get(1).unwrap() else {
            panic!("not version 2");
        };
        assert_eq!(outcome.lines.get(1).unwrap(), &line(untouched));
        assert_eq!(
            outcome.lines.get(2).unwrap(),
            r#"{"discovered": "2026-08-27T10:00:00Z", "gotcha": "effects run twice", "id": "got_3", "library": "react", "source_project": "home-log", "tags": ["node"], "version_range": ">=6"}"#
        );
    }

    #[test]
    fn a_draft_can_set_versions_and_the_library_is_normalised() {
        let mut filled = draft(
            "got_3",
            " React ",
            "Effects run twice in strict mode, so clean up.",
            &["strict mode", "effects"],
            false,
        );
        filled.input.versions = Some(" >=18 ".into());
        let outcome = apply_upgrade(&store(), &[filled], "2026-11-14", &libraries(), passing);
        let line: Value = serde_json::from_str(outcome.lines.get(2).unwrap()).unwrap();
        assert_eq!(line["library"], "react");
        assert_eq!(line["versions"], ">=18");
        assert_eq!(outcome.libraries, ["react"]);
    }

    #[test]
    fn drop_removes_the_record() {
        let drafts = [draft("got_1", "", "", &[], true)];
        let outcome = apply_upgrade(&store(), &drafts, "2026-11-14", &libraries(), passing);
        assert_eq!((outcome.replaced, outcome.dropped), (0, 1));
        assert_eq!(outcome.lines.len(), 2);
        assert!(!outcome.lines.join("\n").contains("got_1"));
        assert_eq!(outcome.libraries, Vec::<String>::new());
    }

    #[test]
    fn a_draft_failing_a_hard_check_keeps_the_version_one_record() {
        let drafts = [draft("got_1", "bull", "short", &["a"], false)];
        let outcome = apply_upgrade(&store(), &drafts, "2026-11-14", &libraries(), |_, input| {
            CheckReport {
                failures: vec![CheckFailure::Length(input.gotcha.chars().count())],
                ..CheckReport::default()
            }
        });
        assert_eq!((outcome.replaced, outcome.dropped), (0, 0));
        assert_eq!(
            outcome.kept,
            [(
                "got_1".to_owned(),
                vec!["the gotcha must be 30 to 600 characters long, not 5".to_owned()]
            )]
        );
        assert!(outcome.lines.first().unwrap().contains(r#""id": "got_1""#));
        assert!(!outcome.lines.first().unwrap().contains(r#""v": 2"#));
    }

    #[test]
    fn the_check_sees_the_record_id_and_the_normalised_input() {
        let drafts = [draft("got_1", "Bull", LESSON, &["a", "b"], false)];
        let outcome = apply_upgrade(
            &store(),
            &drafts,
            "2026-11-14",
            &libraries(),
            |id, input| {
                assert_eq!((id, input.library.as_str()), ("got_1", "bullmq"));
                CheckReport::default()
            },
        );
        assert_eq!(outcome.replaced, 1);
    }

    #[test]
    fn unknown_and_already_upgraded_ids_change_nothing_and_are_listed() {
        let drafts = [
            draft("got_2", "zod", LESSON, &["a", "b"], false),
            draft("got_9", "zod", LESSON, &["a", "b"], true),
        ];
        let records = store();
        let outcome = apply_upgrade(&records, &drafts, "2026-11-14", &libraries(), passing);
        assert_eq!((outcome.replaced, outcome.dropped), (0, 0));
        assert_eq!(
            outcome.kept,
            [
                (
                    "got_2".to_owned(),
                    vec!["the record is already version 2".to_owned()]
                ),
                ("got_9".to_owned(), vec!["no record has this id".to_owned()]),
            ]
        );
        assert!(!touches_version_one(&records, &drafts));
        assert!(touches_version_one(
            &records,
            &[draft("got_3", "", "", &[], true)]
        ));
    }

    #[test]
    fn applying_the_same_drafts_again_changes_nothing() {
        let drafts = [
            draft("got_1", "Bull", LESSON, &["shutdown", "close"], false),
            draft("got_3", "", "", &[], true),
        ];
        let first = apply_upgrade(&store(), &drafts, "2026-11-14", &libraries(), passing);
        assert_eq!((first.replaced, first.dropped), (1, 1));
        let entries: Vec<Entry> = first
            .lines
            .iter()
            .map(|line| entry(&serde_json::from_str(line).unwrap()))
            .collect();
        let second = apply_upgrade(
            &parse_store(entries),
            &drafts,
            "2026-12-01",
            &libraries(),
            passing,
        );
        assert_eq!((second.replaced, second.dropped), (0, 0));
        assert_eq!(second.lines, first.lines);
        assert_eq!(second.kept.len(), 2);
    }
}

mod relevance {
    use serde_json::Value;

    use super::super::{
        GlobalIndex, GotchaInput, Libraries, StoredGotcha, check_gotcha, global_reply, parse_store,
    };

    const GOTCHAS: &str = include_str!("../../../tests/data/relevance/gotchas.jsonl");
    const QUERIES: &str = include_str!("../../../tests/data/relevance/queries.json");

    fn records() -> Vec<StoredGotcha> {
        parse_store(
            GOTCHAS
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .map(|value| value.as_object().unwrap().clone())
                .collect(),
        )
    }

    fn first_id(reply: &str) -> Option<&str> {
        reply
            .lines()
            .find_map(|line| line.strip_prefix("- "))
            .and_then(|line| line.split(' ').next())
    }

    #[test]
    fn every_fixture_record_passes_the_write_checks() {
        let records = records();
        assert_eq!(records.len(), 80);
        for (index, record) in records.iter().enumerate() {
            let StoredGotcha::V2(gotcha) = record else {
                panic!("{} is not version 2", record.id());
            };
            let input = GotchaInput {
                library: gotcha.library.clone(),
                keywords: gotcha.keywords.clone(),
                gotcha: gotcha.gotcha.clone(),
                versions: gotcha.versions.clone(),
            };
            let report = check_gotcha(
                &input,
                &Libraries::default(),
                &[
                    "project-a",
                    "project-b",
                    "project-c",
                    "project-d",
                    "project-e",
                ],
                &records[..index],
                None,
            );
            assert_eq!(report.failures, [], "{}", gotcha.id);
            assert_eq!(report.warnings, [], "{}", gotcha.id);
        }
    }

    #[test]
    fn queries_find_the_expected_record_first_and_off_topic_ones_nothing() {
        let index = GlobalIndex::build(records(), Libraries::default());
        let set: Value = serde_json::from_str(QUERIES).unwrap();
        let queries = set["queries"].as_array().unwrap();
        let misses: Vec<String> = queries
            .iter()
            .filter_map(|case| {
                let query = case["query"].as_str().unwrap();
                let expected = case["first"].as_str().unwrap();
                let reply = global_reply(&index, query, None, 2_000);
                (first_id(&reply) != Some(expected))
                    .then(|| format!("{query}: expected {expected}, got\n{reply}"))
            })
            .collect();
        assert_eq!(queries.len(), 21);
        assert!(
            misses.is_empty(),
            "{} of 21 missed:\n{}",
            misses.len(),
            misses.join("\n\n")
        );
        for query in set["off_topic"].as_array().unwrap() {
            let query = query.as_str().unwrap();
            let reply = global_reply(&index, query, None, 2_000);
            assert!(
                reply.starts_with("No global gotcha matches this query."),
                "{query}:\n{reply}"
            );
        }
    }
}

mod review_fixes {
    use serde_json::json;

    use super::super::{
        AddOutcome, CheckFailure, CheckReport, CheckWarning, Libraries, StoredGotcha, add_reply,
        upgrade_drafts,
    };

    #[test]
    fn library_names_that_differ_only_in_case_keep_every_alias() {
        let libraries =
            Libraries::from_value(&json!({"Next": ["nextjs"], "next": ["next.js"]})).unwrap();
        assert_eq!(libraries.aliases("next"), ["next.js", "nextjs"]);
        assert_eq!(libraries.canonical("nextjs"), "next");
        assert_eq!(libraries.canonical("next.js"), "next");
    }

    #[test]
    fn a_refused_add_also_names_the_warnings() {
        let reply = add_reply(&AddOutcome::Rejected(CheckReport {
            failures: vec![CheckFailure::KeywordCount(1)],
            warnings: vec![CheckWarning::ProjectName("alpha".into())],
            duplicate_of: None,
        }));
        assert_eq!(
            reply,
            "Not added: give 2 to 8 keywords, not 1.\nWarning: it names the project 'alpha'."
        );
    }

    #[test]
    fn a_version_one_record_without_an_id_gets_no_draft() {
        let records = [
            StoredGotcha::from_entry(
                json!({"library": "zod", "gotcha": "no id"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
            StoredGotcha::from_entry(
                json!({"id": "got_1", "library": "zod", "gotcha": "with id"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        ];
        let drafts = upgrade_drafts(&records, |_| Vec::new());
        assert_eq!(drafts.len(), 2);
        assert!(drafts.get(1).is_some_and(|line| line.contains("got_1")));
    }
}
