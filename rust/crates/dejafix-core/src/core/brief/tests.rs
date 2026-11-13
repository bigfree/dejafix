use serde_json::json;

use super::{BriefInput, render_brief};
use crate::core::global::Entry;
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::core::stale::{CommitsSince, Stale};
use crate::testing::{NoResolve, fixed_clock};

const DAY: i64 = 86_400;

fn context() -> PathContext {
    PathContext {
        root: "/work/r".into(),
        cwd: "/work/r".into(),
        fold_case: false,
    }
}

fn stamp(days_ago: i64) -> String {
    let at = fixed_clock().now.as_second() - days_ago * DAY;
    jiff::Timestamp::from_second(at)
        .map(|time| time.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

fn line(id: &str, kind: &str, summary: &str, days_ago: i64, extra: &str) -> String {
    format!(
        r#"{{"id": "{id}", "type": "{kind}", "summary": "{summary}", "timestamp": "{}"{extra}}}"#,
        stamp(days_ago)
    )
}

fn entry(value: &serde_json::Value) -> Entry {
    value.as_object().cloned().unwrap_or_default()
}

fn render(
    lines: &[String],
    gotchas: &[Entry],
    stale: &[(usize, &str, CommitsSince)],
    color: bool,
) -> String {
    let bytes = lines.join("\n").into_bytes();
    let clock = fixed_clock();
    let log = EventLog::parse(&bytes, &clock, &context(), &NoResolve);
    let stale: Vec<Stale<'_, '_>> = stale
        .iter()
        .filter_map(|(index, file, commits_since)| {
            Some(Stale {
                event: log.events().get(*index)?,
                file: (*file).to_owned(),
                commits_since: *commits_since,
            })
        })
        .collect();
    let gotchas: Vec<&Entry> = gotchas.iter().collect();
    render_brief(&BriefInput {
        name: "demo",
        log: &log,
        stale: &stale,
        gotchas: &gotchas,
        now: clock.now,
        color,
    })
}

fn busy() -> Vec<String> {
    vec![
        line(
            "evt_1",
            "issue",
            "login redirect loop",
            40,
            r#", "issue_id": "0001""#,
        ),
        line(
            "evt_2",
            "issue",
            "checkout 500 on empty cart",
            10,
            r#", "issue_id": "0002""#,
        ),
        line("evt_3", "issue", "flaky test", 9, r#", "issue_id": "0003""#),
        line(
            "evt_4",
            "fix",
            "pinned the seed",
            8,
            r#", "issue_id": "0003""#,
        ),
        line(
            "evt_5",
            "attempt",
            "tried debouncing handler in cart.js",
            3,
            r#", "outcome": "failed", "location": "cart.js""#,
        ),
        line(
            "evt_6",
            "attempt",
            "tried contain layout",
            2,
            r#", "outcome": "failed", "location": "cart.js:9""#,
        ),
        line(
            "evt_7",
            "attempt",
            "ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé",
            1,
            r#", "outcome": "failed", "location": "pay.py:12""#,
        ),
        line(
            "evt_8",
            "attempt",
            "files only attempt",
            1,
            r#", "outcome": "failed", "files": ["ui.js"]"#,
        ),
        line(
            "evt_9",
            "attempt",
            "no file attempt",
            1,
            r#", "outcome": "failed""#,
        ),
        line(
            "evt_10",
            "attempt",
            "ancient attempt",
            40,
            r#", "outcome": "failed", "location": "old.py""#,
        ),
        line(
            "evt_11",
            "decision",
            "core uses a registry",
            10,
            r#", "location": "core.py""#,
        ),
        line("evt_12", "decision", "retire me", 11, ""),
        line(
            "evt_13",
            "decision",
            "replacement decision",
            5,
            r#", "supersedes": "evt_12""#,
        ),
        line("evt_14", "note", "a note", 8, ""),
    ]
}

#[test]
fn the_busy_brief_is_stable_without_colour() {
    let stale = [(10, "core.py", CommitsSince::Count(5))];
    let gotchas = [entry(
        &json!({"library": "sqlalchemy", "gotcha": "x", "text": "a text key"}),
    )];
    insta::assert_snapshot!(render(&busy(), &gotchas, &stale, false));
}

#[test]
fn the_busy_brief_is_stable_with_colour() {
    let stale = [(10, "core.py", CommitsSince::Missing)];
    let gotchas = [entry(
        &json!({"library": "sqlalchemy", "text": "a text key"}),
    )];
    insta::assert_snapshot!(render(&busy(), &gotchas, &stale, true));
}

#[test]
fn a_fresh_project_degrades_to_none_lines() {
    insta::assert_snapshot!(render(&[], &[], &[], false));
}

#[test]
fn output_is_utf8_with_the_real_rule() {
    let out = render(&[], &[], &[], false);
    assert!(out.contains(&"─".repeat(60)));
    assert!(out.contains("dejafix brief — demo"));
    assert!(out.contains("⚠ Active warnings"));
    assert!(out.contains("📈 Score"));
    assert!(!out.contains('\u{1b}'));
    assert!(!out.contains("------------------------------------------------------------"));
}

#[test]
fn every_section_shows_its_content() {
    let lines = vec![
        line(
            "evt_1",
            "issue",
            "checkout 500 on empty cart",
            0,
            r#", "issue_id": "0001""#,
        ),
        line(
            "evt_2",
            "attempt",
            "tried debouncing handler in cart.js",
            0,
            r#", "outcome": "failed", "location": "cart.js""#,
        ),
        line("evt_3", "decision", "use argon2 hashing", 0, ""),
    ];
    let out = render(&lines, &[], &[], false);
    for wanted in [
        "Active warnings",
        "cart.js — 1 failed attempt (last: tried debouncing handler in cart.js)",
        "Open issues",
        "#0001 checkout 500 on empty cart",
        "Recent decisions",
        "argon2",
        "Score",
    ] {
        assert!(out.contains(wanted), "{wanted} missing from\n{out}");
    }
    assert!(!out.contains("Possibly stale"));
    assert!(!out.contains("Stack gotchas"));
}

#[test]
fn colour_codes_appear_only_when_asked() {
    let plain = render(&busy(), &[], &[], false);
    let coloured = render(&busy(), &[], &[], true);
    assert!(!plain.contains('\u{1b}'));
    assert!(coloured.contains("\u{1b}[1mdejafix brief — demo\u{1b}[0m"));
    assert!(coloured.contains("\u{1b}[2m"));
    let stripped = coloured
        .split('\u{1b}')
        .enumerate()
        .map(|(index, part)| {
            if index == 0 {
                part
            } else {
                part.split_once('m').map_or(part, |(_, rest)| rest)
            }
        })
        .collect::<String>();
    assert_eq!(stripped, plain);
}

#[test]
fn slices_count_characters_not_bytes() {
    let long = "é".repeat(80);
    let lines = vec![
        line("evt_1", "decision", &long, 0, ""),
        line("evt_2", "issue", &long, 0, r#", "issue_id": "7""#),
    ];
    let out = render(&lines, &[], &[], false);
    assert!(out.contains(&format!("   {}\n", "é".repeat(75))));
    assert!(out.contains(&format!("   #7 {}\n", "é".repeat(70))));
}

#[test]
fn the_score_line_reports_this_weeks_gain() {
    let lines = vec![
        line("evt_1", "decision", "old", 20, ""),
        line("evt_2", "decision", "new one", 1, ""),
        line("evt_3", "decision", "new two", 1, ""),
    ];
    let out = render(&lines, &[], &[], false);
    assert!(
        out.contains("📈 Score  F (4/100) ▲ +3 this week\n"),
        "{out}"
    );
}

#[test]
fn an_unreadable_timestamp_is_left_out_of_the_older_set() {
    let lines = vec![
        line("evt_1", "decision", "old", 20, ""),
        r#"{"id": "evt_2", "type": "decision", "summary": "undated", "timestamp": "garbage"}"#
            .to_owned(),
    ];
    let out = render(&lines, &[], &[], false);
    assert!(out.contains("(3/100) ▲ +2 this week\n"), "{out}");
}

#[test]
fn nothing_new_in_a_week_is_unchanged() {
    let lines = vec![line("evt_1", "decision", "old", 20, "")];
    let out = render(&lines, &[], &[], false);
    assert!(out.contains("(1/100) — unchanged this week\n"), "{out}");
}

#[test]
fn a_gotcha_shows_text_then_summary_then_nothing() {
    let gotchas = [
        entry(&json!({"library": "a", "text": "first", "summary": "ignored"})),
        entry(&json!({"library": "b", "summary": "second"})),
        entry(&json!({"library": "c", "gotcha": "real entries have no text"})),
        entry(&json!({"text": "", "summary": "", "library": "d"})),
    ];
    let out = render(&[], &gotchas, &[], false);
    assert!(
        out.contains("💡 Stack gotchas\n   b: second\n   c: \n   d: \n"),
        "{out}"
    );
    assert!(!out.contains("a: first"));
}

#[test]
fn stale_lists_five_then_the_retire_hint() {
    let lines: Vec<String> = (1..=7)
        .map(|n| {
            line(
                &format!("evt_{n}"),
                "decision",
                &format!("claim {n}"),
                10,
                r#", "location": "core.py""#,
            )
        })
        .collect();
    let stale: Vec<(usize, &str, CommitsSince)> = (0..7)
        .map(|index| (index, "core.py", CommitsSince::Count(4)))
        .collect();
    let out = render(&lines, &[], &stale, false);
    assert_eq!(out.matches("core.py changed 4x since").count(), 5);
    assert!(out.contains("   confirm, or retire: djf decision \"...\" --supersedes <id>\n"));
}

#[test]
fn warnings_keep_the_four_busiest_files_and_a_stable_order() {
    let lines: Vec<String> = [
        ("a.py", 1),
        ("b.py", 2),
        ("c.py", 2),
        ("d.py", 1),
        ("e.py", 3),
    ]
    .iter()
    .flat_map(|(file, count)| {
        (0..*count).map(move |n| {
            line(
                &format!("evt_{file}_{n}"),
                "attempt",
                &format!("try {file} {n}"),
                1,
                &format!(r#", "outcome": "failed", "location": "{file}""#),
            )
        })
    })
    .collect();
    let out = render(&lines, &[], &[], false);
    let shown: Vec<&str> = out
        .lines()
        .filter(|line| line.contains("failed attempt"))
        .map(|line| line.split(' ').nth(3).unwrap_or(""))
        .collect();
    assert_eq!(shown, ["e.py", "b.py", "c.py", "a.py"]);
}
