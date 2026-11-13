use std::fs;
use std::process::Command;

use jiff::SignedDuration;

use crate::support::{Repo, TestResult};

const RULE: &str = "────────────────────────────────────────────────────────────";

const PYTHON_BRIEF: &str = "
dejafix brief — repo
────────────────────────────────────────────────────────────
⚠ Active warnings
   cart.js — 2 failed attempts (last: tried contain layout)
   pay.py — 1 failed attempt (last: ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýá)
   ui.js — 1 failed attempt (last: files only attempt)
   (no file) — 1 failed attempt (last: no file attempt)
⏳ Possibly stale
   decision [evt_b3] core uses a registry — core.py changed 5x since
   decision [evt_b4] missing module owns the cache — missing.py no longer exists
   confirm, or retire: djf decision \"...\" --supersedes <id>
📋 Open issues
   #0001 login redirect loop
   #0002 checkout 500 on empty cart
🕑 Recent decisions
   decision number 2 with a rather long text that goes on and on and on and on
   decision number 3 with a rather long text that goes on and on and on and on
   decision number 4 with a rather long text that goes on and on and on and on
   decision number 5 with a rather long text that goes on and on and on and on
   bad timestamp event
💡 Stack gotchas
   fastapi:\u{20}
   pydantic:\u{20}
   sqlalchemy: a text key
📈 Score  D (37/100) ▲ +14 this week
────────────────────────────────────────────────────────────

";

fn git(repo: &Repo, args: &[&str]) -> TestResult {
    let status = Command::new("git")
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(args)
        .current_dir(&repo.root)
        .status()?;
    assert!(status.success());
    Ok(())
}

fn days_ago(days: i64) -> String {
    jiff::Timestamp::now()
        .checked_sub(SignedDuration::from_hours(24 * days))
        .map(|time| time.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

fn event(id: &str, kind: &str, summary: &str, at: &str, extra: &str) -> String {
    format!(
        r#"{{"id": "{id}", "summary": "{summary}", "timestamp": "{at}", "type": "{kind}"{extra}}}"#
    )
}

type Row = (&'static str, &'static str, &'static str, i64, &'static str);

const ROWS: [Row; 16] = [
    (
        "evt_a1",
        "issue",
        "login redirect loop",
        40,
        r#", "issue_id": "0001", "location": "auth.py:10""#,
    ),
    (
        "evt_a2",
        "issue",
        "checkout 500 on empty cart",
        10,
        r#", "issue_id": "0002", "location": "cart.js:3""#,
    ),
    (
        "evt_a3",
        "issue",
        "flaky test",
        9,
        r#", "issue_id": "0003""#,
    ),
    (
        "evt_a4",
        "fix",
        "pinned the seed",
        8,
        r#", "issue_id": "0003""#,
    ),
    (
        "evt_a5",
        "attempt",
        "tried debouncing handler in cart.js",
        3,
        r#", "issue_id": "0002", "location": "cart.js", "outcome": "failed""#,
    ),
    (
        "evt_a6",
        "attempt",
        "tried contain layout",
        2,
        r#", "issue_id": "0002", "location": "cart.js:9", "outcome": "failed""#,
    ),
    (
        "evt_a7",
        "attempt",
        "ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé ščřžýáíé",
        1,
        r#", "issue_id": "0002", "location": "pay.py:12", "outcome": "failed""#,
    ),
    (
        "evt_a8",
        "attempt",
        "files only attempt",
        1,
        r#", "files": ["ui.js"], "outcome": "failed""#,
    ),
    (
        "evt_a9",
        "attempt",
        "no file attempt",
        1,
        r#", "outcome": "failed""#,
    ),
    (
        "evt_b1",
        "attempt",
        "ancient attempt",
        40,
        r#", "location": "old.py", "outcome": "failed""#,
    ),
    (
        "evt_b2",
        "attempt",
        "it worked",
        20,
        r#", "outcome": "worked""#,
    ),
    (
        "evt_b3",
        "decision",
        "core uses a registry",
        10,
        r#", "location": "core.py""#,
    ),
    (
        "evt_b4",
        "decision",
        "missing module owns the cache",
        10,
        r#", "location": "missing.py""#,
    ),
    ("evt_b5", "decision", "use argon2 hashing", 12, ""),
    ("evt_b6", "decision", "retire me", 11, ""),
    (
        "evt_b7",
        "decision",
        "replacement decision",
        5,
        r#", "supersedes": "evt_b6""#,
    ),
];

fn busy_events() -> Vec<String> {
    let mut lines: Vec<String> = ROWS
        .iter()
        .map(|(id, kind, summary, days, extra)| event(id, kind, summary, &days_ago(*days), extra))
        .collect();
    for number in 1..=5 {
        lines.push(event(
            &format!("evt_c{number}"),
            "decision",
            &format!("decision number {number} with a rather long text that goes on and on and on and on past seventy five characters"),
            &days_ago(13 + number),
            "",
        ));
    }
    lines.push(event("evt_d1", "note", "a note", &days_ago(8), ""));
    lines.push(event(
        "evt_d2",
        "fix",
        "a fix with context",
        &days_ago(9),
        r#", "issue_id": "0009""#,
    ));
    lines.push(event(
        "evt_d3",
        "decision",
        "bad timestamp event",
        "garbage",
        "",
    ));
    lines
}

const GOTCHAS: [&str; 5] = [
    r#"{"discovered": "2099-01-01T00:00:00Z", "gotcha": "always close async sessions", "id": "got_000000000001", "library": "sqlalchemy", "source_project": "other", "tags": ["python"], "version_range": null}"#,
    r#"{"discovered": "2024-01-01T00:00:00Z", "gotcha": "fastapi deps are cached", "id": "got_000000000002", "library": "fastapi", "source_project": "other", "tags": ["python", "web"], "version_range": ">=0.100"}"#,
    r#"{"discovered": "2099-02-01T00:00:00Z", "gotcha": "unrelated lib thing", "id": "got_000000000003", "library": "gin", "source_project": "", "tags": [], "version_range": null}"#,
    r#"{"discovered": "2099-03-01T00:00:00Z", "gotcha": "pydantic v2 strictness", "id": "got_000000000004", "library": "pydantic", "tags": ["python"], "version_range": "2.x"}"#,
    r#"{"discovered": "2099-03-02T00:00:00Z", "gotcha": "with text key", "id": "got_000000000005", "library": "sqlalchemy", "summary": "summary wins when no text", "tags": [], "text": "a text key", "version_range": null}"#,
];

fn busy_repo() -> Result<Repo, Box<dyn std::error::Error>> {
    let repo = Repo::new()?;
    fs::write(
        repo.root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\ndependencies = [\"sqlalchemy>=2\", \"fastapi\", \"pydantic\"]\n",
    )?;
    fs::write(repo.root.join("core.py"), "a\n")?;
    git(&repo, &["add", "."])?;
    git(&repo, &["commit", "-qm", "add core"])?;
    for number in 1..=4 {
        fs::write(repo.root.join("core.py"), format!("a\n{number}\n"))?;
        git(&repo, &["commit", "-qam", &format!("core {number}")])?;
    }
    fs::write(
        repo.mem().join("events.jsonl"),
        busy_events().join("\n") + "\n",
    )?;
    let global = repo.registry().join("global");
    fs::create_dir_all(&global)?;
    fs::write(
        global.join("library_gotchas.jsonl"),
        GOTCHAS.join("\n") + "\n",
    )?;
    Ok(repo)
}

#[test]
fn brief_shows_all_sections() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "checkout 500 on empty cart"])
        .assert()
        .success();
    repo.djf(&[
        "attempt",
        "tried debouncing handler in cart.js",
        "--failed",
        "--at",
        "cart.js",
    ])
    .assert()
    .success();
    repo.djf(&["decision", "use argon2 hashing"])
        .assert()
        .success();
    let out = repo.stdout(&["brief"])?;
    assert_eq!(
        out,
        format!(
            "\ndejafix brief — repo\n{RULE}\n⚠ Active warnings\n   cart.js — 1 failed attempt (last: tried debouncing handler in cart.js)\n📋 Open issues\n   #0001 checkout 500 on empty cart\n🕑 Recent decisions\n   use argon2 hashing\n📈 Score  F (4/100) ▲ +4 this week\n{RULE}\n\n"
        )
    );
    for wanted in [
        "Active warnings",
        "cart.js",
        "Open issues",
        "checkout 500",
        "Recent decisions",
        "argon2",
        "Score",
    ] {
        assert!(out.contains(wanted));
    }
    Ok(())
}

#[test]
fn brief_runs_clean_on_fresh_project() -> TestResult {
    let repo = Repo::new()?;
    let assert = repo.djf(&["brief"]).assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone())?;
    assert_eq!(
        out,
        format!(
            "\ndejafix brief — repo\n{RULE}\n⚠ Active warnings\n   none — no failed attempts in 30 days\n📋 Open issues\n   none open\n🕑 Recent decisions\n   none logged yet\n📈 Score  F (0/100) — unchanged this week\n{RULE}\n\n"
        )
    );
    assert!(out.contains("none"));
    Ok(())
}

#[test]
fn brief_output_is_utf8_with_the_real_rule() -> TestResult {
    let repo = Repo::new()?;
    let output = repo.djf(&["brief"]).output()?;
    let out = String::from_utf8(output.stdout)?;
    assert!(out.contains(&"─".repeat(60)));
    assert!(!out.contains("------------"));
    assert!(out.contains("⚠ Active warnings"));
    assert!(out.contains("📈 Score"));
    assert!(!out.contains('\u{1b}'));
    Ok(())
}

#[test]
fn brief_matches_python_on_a_busy_project() -> TestResult {
    let repo = busy_repo()?;
    assert_eq!(repo.stdout(&["brief"])?, PYTHON_BRIEF);
    Ok(())
}

#[test]
fn brief_outside_a_project_fails() -> TestResult {
    let repo = Repo::bare()?;
    let assert = repo.djf(&["brief"]).assert().code(1);
    let err = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(err.starts_with("Error: No .dejafix directory found"));
    Ok(())
}
