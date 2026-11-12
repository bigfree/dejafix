use serde_json::{Value, json};

use super::{
    RECORD_CHARS, SummaryInput, clip_record, extract_purpose_from_map,
    extract_purpose_from_summary, issue_files, key_files, render_summary, slugify_title,
};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::testing::{NoResolve, fixed_clock};

const T: &str = "2027-01-15T07:00:00Z";

struct Events(Vec<Value>);

impl Events {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add(mut self, kind: &str, summary: &str, extra: &Value) -> Self {
        let mut event = json!({
            "id": format!("evt_{:020}", self.0.len() + 1),
            "summary": summary,
            "timestamp": T,
            "type": kind,
        });
        if let (Some(event), Some(extra)) = (event.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                event.insert(key.clone(), value.clone());
            }
        }
        self.0.push(event);
        self
    }

    fn issues(mut self, count: usize, fixed: bool, first: usize) -> Self {
        for i in first..first + count {
            let id = format!("{i:04}");
            self = self.add("issue", &format!("issue {id}"), &json!({"issue_id": id}));
            if fixed {
                self = self.add("fix", &format!("fixed {id}"), &json!({"issue_id": id}));
            }
        }
        self
    }

    fn text(&self) -> String {
        self.0
            .iter()
            .flat_map(|event| [event.to_string(), "\n".to_owned()])
            .collect()
    }
}

fn context() -> PathContext {
    PathContext {
        root: "/work/proj".into(),
        cwd: "/work/proj".into(),
        fold_case: false,
    }
}

fn render(events: &Events) -> String {
    render_with(events, None)
}

fn render_with(events: &Events, purpose: Option<&str>) -> String {
    let text = events.text();
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    render_summary(
        &log,
        &SummaryInput {
            project_name: "proj",
            today: jiff::civil::date(2027, 1, 15),
            purpose,
        },
    )
}

fn recent_issue_lines(summary: &str) -> Vec<String> {
    let section = summary
        .split("## Recent issues")
        .nth(1)
        .and_then(|rest| rest.split("## Decisions").next())
        .unwrap_or("");
    section
        .lines()
        .filter(|line| line.starts_with("- ["))
        .map(str::to_owned)
        .collect()
}

fn section(summary: &str, name: &str) -> String {
    summary
        .split(&format!("## {name}"))
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .unwrap_or("")
        .to_owned()
}

fn listed_key_files(summary: &str) -> Vec<String> {
    section(summary, "Key files")
        .lines()
        .filter(|line| line.starts_with("- `"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn an_empty_log_renders_every_placeholder() {
    insta::assert_snapshot!(render(&Events::new()));
}

#[test]
fn recent_issues_capped_at_ten() {
    let summary = render(&Events::new().issues(12, true, 1));
    let lines = recent_issue_lines(&summary);
    assert_eq!(lines.len(), 10);
    assert!(lines[0].contains("#0012"));
    assert!(lines[9].contains("#0003"));
    assert!(!summary.contains("#0001") && !summary.contains("#0002"));
    assert!(summary.contains("2 older fixed issues not shown"));
}

#[test]
fn recent_issues_under_ten_all_listed() {
    let summary = render(&Events::new().issues(3, false, 1));
    let lines = recent_issue_lines(&summary);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("#0003") && lines[2].contains("#0001"));
    assert!(!summary.contains("not shown"));
}

#[test]
fn an_old_open_issue_is_never_capped_away() {
    let summary = render(&Events::new().issues(1, false, 1).issues(12, true, 2));
    let lines = recent_issue_lines(&summary);
    assert!(lines[0].starts_with("- [OPEN] #0001"));
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("- [DONE]"))
            .count(),
        10
    );
}

#[test]
fn many_open_issues_are_all_listed() {
    assert_eq!(
        recent_issue_lines(&render(&Events::new().issues(15, false, 1))).len(),
        15
    );
}

#[test]
fn a_large_open_backlog_is_named_not_dropped() {
    let summary = render(&Events::new().issues(41, false, 1));
    let lines = recent_issue_lines(&summary);
    let full: Vec<&String> = lines
        .iter()
        .filter(|line| !line.contains(" older open issue"))
        .collect();
    assert_eq!(full.len(), 20);
    assert!(full[0].contains("#0041") && full[19].contains("#0022"));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("- [OPEN] 21 older open issues, by id: #0021, #0020"))
    );
    for i in 1..42 {
        assert!(summary.contains(&format!("#{i:04}")));
    }
}

#[test]
fn issue_ids_past_9999_order_numerically() {
    let lines = recent_issue_lines(&render(&Events::new().issues(12, true, 9995)));
    assert!(lines[0].contains("#10006"));
    assert!(lines[9].contains("#9997"));
    assert!(!lines.join("\n").contains("#9995"));
}

#[test]
fn non_decimal_ids_sort_after_numbers_in_reverse_text_order() {
    let events = Events::new()
        .add("issue", "a", &json!({"issue_id": "abc"}))
        .add("issue", "b", &json!({"issue_id": "0002"}))
        .add("issue", "c", &json!({"issue_id": "zeta"}))
        .add("issue", "d", &json!({"issue_id": "2"}));
    let lines = recent_issue_lines(&render(&events));
    let order: Vec<&str> = lines
        .iter()
        .map(|line| line.split(' ').nth(2).unwrap_or(""))
        .collect();
    assert_eq!(order, ["#2", "#0002", "#zeta", "#abc"]);
}

#[test]
fn decisions_capped_at_newest_fifteen() {
    let mut events = Events::new();
    for i in 1..21 {
        events = events.add("decision", &format!("decision {i:02}"), &json!({}));
    }
    let decisions = section(&render(&events), "Decisions");
    let shown: Vec<&str> = decisions
        .lines()
        .filter(|line| line.starts_with("- decision"))
        .collect();
    assert_eq!(shown.len(), 15);
    assert_eq!(shown[0], "- decision 06");
    assert_eq!(shown[14], "- decision 20");
    assert!(decisions.contains("5 earlier decisions not shown"));
}

#[test]
fn superseded_decisions_drop_out_but_notes_stay() {
    let events = Events::new()
        .add("decision", "old choice", &json!({}))
        .add(
            "decision",
            "new choice",
            &json!({"supersedes": "evt_00000000000000000001"}),
        )
        .add("note", "a note", &json!({}));
    let summary = render(&events);
    assert!(!summary.contains("old choice"));
    assert!(summary.contains("- new choice\n"));
    assert!(summary.contains("- a note\n"));
}

#[test]
fn an_orphan_attempt_does_not_crash_the_summary() {
    let events = Events::new().issues(1, false, 1).add(
        "attempt",
        "tried x",
        &json!({"issue_id": "0099", "outcome": "failed"}),
    );
    let lines = recent_issue_lines(&render(&events));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("#0001"));
}

#[test]
fn a_fixed_issue_is_listed_by_its_title_alone() {
    for (issue_location, fix_location, shown) in [
        (
            Some("auth/redirect.py"),
            Some("auth/middleware.py"),
            " [auth/redirect.py]",
        ),
        (None, Some("auth/middleware.py"), " [auth/middleware.py]"),
        (None, None, ""),
    ] {
        let events = Events::new()
            .add(
                "issue",
                "login redirect loop",
                &json!({"issue_id": "0001", "location": issue_location}),
            )
            .add(
                "attempt",
                "cleared the session cookie first",
                &json!({"issue_id": "0001", "outcome": "failed"}),
            )
            .add(
                "fix",
                "guarded the redirect in one middleware",
                &json!({"issue_id": "0001", "location": fix_location}),
            );
        let summary = render(&events);
        assert_eq!(
            recent_issue_lines(&summary),
            [format!("- [DONE] #0001 login redirect loop{shown} (fixed)")]
        );
        assert!(!summary.contains("cleared the session cookie first"));
        assert!(!summary.contains("guarded the redirect in one middleware"));
    }
}

#[test]
fn open_issues_show_their_last_three_lessons() {
    let mut events = Events::new().add("issue", "flaky", &json!({"issue_id": "0001"}));
    for (i, outcome) in ["failed", "partial", "worked", "failed", "partial"]
        .iter()
        .enumerate()
    {
        events = events.add(
            "attempt",
            &format!("try {i}"),
            &json!({"issue_id": "0001", "outcome": outcome, "location": "a.py:1"}),
        );
    }
    let summary = render(&events);
    assert!(summary.contains(
        "- [OPEN] #0001 flaky (open)\n  - Partial attempt: try 1 [a.py:1]\n  - Failed attempt: try 3 [a.py:1]\n  - Partial attempt: try 4 [a.py:1]\n"
    ));
}

#[test]
fn long_records_are_clipped_but_keep_their_location() {
    let long_text = "the collector drops its websocket ".repeat(30);
    let events = Events::new()
        .add(
            "issue",
            &long_text,
            &json!({"issue_id": "0001", "location": "src/collector/socket.py"}),
        )
        .add(
            "attempt",
            &long_text,
            &json!({"issue_id": "0001", "outcome": "partial"}),
        )
        .add(
            "attempt",
            "warmed the cache in main.py",
            &json!({"issue_id": "0001", "outcome": "failed"}),
        )
        .add("decision", &long_text, &json!({}))
        .add("note", &long_text, &json!({}));
    let summary = render(&events);
    assert!(recent_issue_lines(&summary)[0].ends_with("... [src/collector/socket.py] (open)"));
    assert!(summary.contains("  - Failed attempt: warmed the cache in main.py\n"));
    let clipped: Vec<&str> = summary
        .lines()
        .filter(|line| line.contains("the collector drops"))
        .collect();
    assert_eq!(clipped.len(), 4);
    for line in clipped {
        let start = line.find("the collector drops").unwrap();
        let kept = line[start..].split("...").next().unwrap();
        assert!(long_text.starts_with(kept));
        assert_eq!(&long_text[kept.len()..=kept.len()], " ");
        assert!(RECORD_CHARS / 2 < kept.len() + 3 && kept.len() + 3 <= RECORD_CHARS);
    }
}

#[test]
fn a_long_path_is_not_clipped_down_to_the_word_before_it() {
    let path = format!("src/{}client.py", "generated/".repeat(40));
    let summary = render(&Events::new().add("note", &format!("see {path}"), &json!({})));
    let note = section(&summary, "Notes");
    let note = note.trim();
    assert!(note.starts_with("- see src/generated/"));
    assert!(note.ends_with("..."));
    assert_eq!(note.chars().count(), 2 + RECORD_CHARS);
}

#[test]
fn clipping_counts_characters_not_bytes() {
    let text = "žluťoučký kůň ".repeat(40);
    let clipped = clip_record(&text);
    assert!(clipped.chars().count() <= RECORD_CHARS);
    assert!(clipped.ends_with("kůň...") || clipped.ends_with("žluťoučký..."));
    assert_eq!(clip_record("  a \n b  "), "a b");
}

#[test]
fn a_multi_line_record_stays_on_one_summary_line() {
    let events = Events::new().add(
        "note",
        "  first line\n## not a heading\n\tthird  ",
        &json!({"location": "a.py\n## not a heading either"}),
    );
    let summary = render(&events);
    assert!(summary.contains("- first line ## not a heading third [a.py ## not a heading either]"));
    assert!(!summary.contains("\n## not a heading"));
}

#[test]
fn a_blank_location_falls_back_to_the_fixs() {
    let events = Events::new()
        .add(
            "issue",
            "blank location",
            &json!({"issue_id": "0001", "location": "   "}),
        )
        .add(
            "fix",
            "patched",
            &json!({"issue_id": "0001", "location": "src/x.py"}),
        );
    let summary = render(&events);
    assert!(summary.contains("- [DONE] #0001 blank location [src/x.py] (fixed)"));
    assert!(!summary.contains("[]"));
}

#[test]
fn key_files_ignore_version_numbers_and_identifiers() {
    let events = Events::new().add(
        "note",
        "bumped to 7.x and 23.1.2 via ts.readConfigFile",
        &json!({}),
    );
    assert_eq!(listed_key_files(&render(&events)), Vec::<String>::new());
}

#[test]
fn key_files_keep_root_level_files_mentioned_in_text() {
    let events = Events::new().add("note", "pinned the version in pyproject.toml", &json!({}));
    assert_eq!(listed_key_files(&render(&events)), ["- `pyproject.toml`"]);
}

#[test]
fn key_files_include_locations_that_name_a_file() {
    let events = Events::new()
        .add(
            "note",
            "gotcha: truncates at 72 bytes",
            &json!({"location": "bcrypt"}),
        )
        .add(
            "decision",
            "keep one entry point",
            &json!({"location": "AuthService.login"}),
        )
        .add(
            "note",
            "guard the redirect",
            &json!({"location": "src/auth.py"}),
        );
    assert_eq!(listed_key_files(&render(&events)), ["- `src/auth.py`"]);
}

#[test]
fn key_files_merge_line_suffixes_into_one_file() {
    let events = Events::new()
        .add("note", "one", &json!({"location": "src/auth.py:42"}))
        .add("note", "two, see src/auth.py:7", &json!({}));
    assert_eq!(listed_key_files(&render(&events)), ["- `src/auth.py`"]);
}

#[test]
fn key_files_rank_by_event_count_and_skip_memory_files() {
    let events = Events::new()
        .add("note", "one", &json!({"location": "src/rare.py"}))
        .add("note", "two", &json!({"location": "src/busy.py"}))
        .add("note", "three", &json!({"files": ["src/busy.py"]}))
        .add("note", "four", &json!({"location": ".dejafix/summary.md"}));
    assert_eq!(
        listed_key_files(&render(&events)),
        ["- `src/busy.py`", "- `src/rare.py`"]
    );
}

#[test]
fn a_mention_glued_to_a_word_character_is_not_a_file() {
    let events = Events::new().add("note", "ab.c:12d.py and é/b.py and (c.rs)", &json!({}));
    let text = events.text();
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    let files: Vec<String> = key_files(&log).into_iter().map(|(path, _)| path).collect();
    assert_eq!(files, ["ab.c", "é/b.py", "c.rs"]);
}

#[test]
fn purpose_comes_from_the_map_then_the_summary() {
    assert_eq!(
        extract_purpose_from_map(
            "# Map\n\n## Project purpose\n  \nA forecasting tool.\n\n## Structure\n"
        ),
        Some("A forecasting tool.")
    );
    assert_eq!(
        extract_purpose_from_map("## Project purpose\nNot described yet.\n\n## Structure\n"),
        None
    );
    assert_eq!(extract_purpose_from_map("## Project purpose x\nA\n"), None);
    assert_eq!(
        extract_purpose_from_map("## Project purpose x\nA\n## Project purpose\nB"),
        Some("B")
    );
    assert_eq!(
        extract_purpose_from_map("## Project purpose\n\n## Structure\n- a\n## Next\n"),
        Some("## Structure\n- a")
    );
    assert_eq!(
        extract_purpose_from_summary("## What this project is\nOld text.\n## Decisions\n"),
        Some("Old text.")
    );
    assert_eq!(
        extract_purpose_from_summary("## Project purpose\nReplace this placeholder with x\n"),
        None
    );
    assert_eq!(
        extract_purpose_from_summary("## Project purpose \nA\n"),
        None
    );
}

#[test]
fn purpose_is_rendered_when_given() {
    let summary = render_with(&Events::new(), Some("A prediction ML project."));
    assert!(summary.contains("## Project purpose\nA prediction ML project.\n\n## Recent issues"));
}

#[test]
fn issue_files_list_every_event_of_the_issue() {
    let events = Events::new()
        .add(
            "issue",
            "Login redirect loop!",
            &json!({"issue_id": "0001", "location": "auth.py:1"}),
        )
        .add(
            "attempt",
            "cleared cookies",
            &json!({"issue_id": "0001", "outcome": "failed"}),
        )
        .add("fix", "guarded it", &json!({"issue_id": "0001"}))
        .add("attempt", "orphan", &json!({"issue_id": "0099"}));
    let text = events.text();
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    let files = issue_files(&log);
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].name.as_deref(),
        Some("0001-login-redirect-loop.md")
    );
    insta::assert_snapshot!(files[0].contents);
}

#[test]
fn an_unsafe_issue_id_gets_no_file_but_stays_in_the_summary() {
    let events = Events::new().add("issue", "bad", &json!({"issue_id": "../../escape"}));
    let text = events.text();
    let log = EventLog::parse(text.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    assert_eq!(issue_files(&log)[0].name, None);
    assert!(render(&events).contains("#../../escape bad"));
}

#[test]
fn slugs_follow_python() {
    assert_eq!(slugify_title("Login redirect loop!"), "login-redirect-loop");
    assert_eq!(slugify_title("!!!"), "issue");
    assert_eq!(slugify_title("Ünïcode Title"), "n-code-title");
    assert_eq!(
        slugify_title(&"ab ".repeat(30)),
        "ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab"
    );
}
