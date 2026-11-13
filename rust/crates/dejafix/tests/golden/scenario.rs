use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

use dejafix_core::testing::{FIXTURE_SEED, write_project};
use serde_json::Value;

use crate::harness::{diff, run};
use crate::types::{Implementation, Run};

type TestResult = Result<(), Box<dyn Error>>;

fn canonical_key_files(text: &str) -> String {
    let newline = if text.contains("## Key files\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let header = format!("## Key files{newline}");
    let Some(start) = text.find(&header) else {
        return text.to_owned();
    };
    let body = start + header.len();
    let end = text
        .get(body..)
        .and_then(|rest| rest.find(&newline.repeat(2)))
        .map_or(text.len(), |at| body + at);
    let mut lines: Vec<&str> = text.get(body..end).unwrap_or("").lines().collect();
    lines.sort_unstable();
    format!("{}{}{}", &text[..body], lines.join(newline), &text[end..])
}

const EMPTY_SUMMARY: &str = "Event summary cannot be empty";

pub fn canonical(mut run: Run) -> Run {
    for step in &mut run.steps {
        step.stdout = canonical_key_files(&step.stdout);
        if step.stderr.contains("Traceback") && step.stderr.contains(EMPTY_SUMMARY) {
            step.stderr = format!("Error: {EMPTY_SUMMARY}\n");
        }
    }
    for text in run.files.values_mut() {
        *text = canonical_key_files(text);
    }
    run
}

fn live_decision(root: &Path) -> Result<String, Box<dyn Error>> {
    let text = fs::read_to_string(root.join(".dejafix/events.jsonl"))?;
    let events: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let retired: BTreeSet<&str> = events
        .iter()
        .filter_map(|event| event["supersedes"].as_str())
        .collect();
    events
        .iter()
        .rev()
        .filter(|event| event["type"] == "decision")
        .filter_map(|event| event["id"].as_str())
        .find(|id| !retired.contains(id))
        .map(str::to_owned)
        .ok_or_else(|| "no live decision in the fixture".into())
}

#[test]
fn write_scenario_matches_python() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), 3_000, FIXTURE_SEED)?;
    fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"react": "18", "vite": "5"}}"#,
    )?;
    let superseded = live_decision(dir.path())?;
    let secret = format!("token sk-{} leaked in a log", "a".repeat(44));
    let steps: Vec<Vec<&str>> = vec![
        vec!["regenerate"],
        vec![
            "log",
            "Checkout total is off by one cent",
            "--at",
            "src/cart/total.py:88",
        ],
        vec![
            "attempt",
            "gotcha: react strict mode double-invokes the reducer",
            "--failed",
            "--at",
            "src/cart/total.py:90",
        ],
        vec!["attempt", "Round half-even in the reducer", "--partial"],
        vec![
            "fix",
            "Use Decimal for line totals",
            "--at",
            "src/cart/total.py:91",
        ],
        vec![
            "decision",
            "Money is Decimal end to end",
            "--supersedes",
            &superseded,
        ],
        vec!["note", "gotcha: vite caches env vars until restart"],
        vec!["note", &secret],
        vec!["attempt", "Orphan attempt", "--issue", "9999"],
        vec!["attempt", "Two outcomes", "--failed", "--worked"],
        vec!["fix", "Nothing open"],
        vec!["note", "   "],
        vec!["attempt", "Fresh parent", "--auto-issue", "--worked"],
        vec!["search", "decimal"],
        vec!["search", "cart|money", "--regex"],
        vec!["search", "cart|money"],
        vec!["search", "reducer", "--failed-only"],
        vec!["search", "STRASSE"],
        vec!["show"],
    ];
    let steps: Vec<&[&str]> = steps.iter().map(Vec::as_slice).collect();
    let python = canonical(run(Implementation::Python, dir.path(), &steps)?);
    let rust = canonical(run(Implementation::Rust, dir.path(), &steps)?);
    let differences = diff(&python, &rust);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert!(
        python
            .files
            .contains_key("home/.dejafix/global/library_gotchas.jsonl")
    );
    Ok(())
}
