use std::error::Error;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::m3::{assert_same, clock, compare, compare_with_home, djf, precheck_fixture};
use crate::types::{Action, McpSession};

type TestResult = Result<(), Box<dyn Error>>;

const INIT: &[&str] = &[
    "init",
    "--no-hooks",
    "--no-mcp-config",
    "--no-structure",
    "--no-backfill",
    "--no-watch",
];

fn session(cwd: Option<&'static str>, calls: Vec<(&'static str, Value)>) -> Action<'static> {
    Action::Mcp(McpSession {
        args: Vec::new(),
        cwd,
        calls,
    })
}

fn empty() -> Value {
    json!({})
}

fn plain_fixture(root: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root)?;
    Ok(())
}

#[test]
fn mcp_tools_match_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    plain_fixture(fixture.path())?;
    let actions = vec![
        djf(INIT),
        session(None, vec![("tools/list", empty())]),
        session(Some("elsewhere"), vec![("tools/list", empty())]),
        Action::Mcp(McpSession {
            args: vec!["--root".to_owned(), "{project}".to_owned()],
            cwd: Some("elsewhere"),
            calls: vec![("current_project", empty())],
        }),
    ];
    let (python, differences) = compare(fixture.path(), &actions, clock)?;
    assert!(
        python
            .steps
            .iter()
            .any(|step| step.stdout.contains("THIS SERVER SERVES SEVERAL PROJECTS")),
        "{:?}",
        python.steps
    );
    assert_same("mcp tools", &differences);
    Ok(())
}

fn read_calls() -> Vec<(&'static str, Value)> {
    vec![
        ("start_session", empty()),
        ("get_instructions", empty()),
        ("get_summary", empty()),
        ("get_project_map", empty()),
        ("get_plan", empty()),
        ("precheck_file", json!({"file_path": "src/app.py"})),
        (
            "precheck_files",
            json!({"file_paths": ["src/auth/session.py", "src/api/routes.ts", ".dejafix/summary.md", ".", "src/gone.py", " ./src/app.py"]}),
        ),
        ("get_issue", json!({"issue_id": "1"})),
        ("get_issue", json!({"issue_id": "#2"})),
        ("get_issue", json!({"issue_id": "*"})),
        ("get_issue", json!({"issue_id": "9999"})),
        ("search_events", json!({"query": "zz-nothing-zz"})),
        (
            "search_events",
            json!({"query": "zz-nothing-zz", "limit": " 3 ", "full": "yes"}),
        ),
        (
            "search_events",
            json!({"query": "zz-nothing-zz", "limit": 2.0, "full": 0}),
        ),
        (
            "get_context",
            json!({"tokens": "500.0", "focus": "src/app.py"}),
        ),
        ("get_score", empty()),
        ("get_context", empty()),
        ("get_context", json!({"tokens": 500})),
        ("get_context", json!({"tokens": 8000, "focus": "src/auth/"})),
        ("get_context", json!({"tokens": 150, "focus": "src/app.py"})),
    ]
}

fn write_calls() -> Vec<(&'static str, Value)> {
    vec![
        (
            "record_attempt",
            json!({"summary": "tried the cache again"}),
        ),
        ("record_fix", json!({"summary": "fixed without an id"})),
        (
            "log_issue",
            json!({"summary": "Login double submit", "location": "src/app.py:12"}),
        ),
        (
            "record_attempt",
            json!({"summary": "debounce the button", "outcome": "partial", "issue_id": "1"}),
        ),
        (
            "record_fix",
            json!({"summary": "guard the handler", "issue_id": "0001"}),
        ),
        (
            "record_fix",
            json!({"summary": "unknown issue", "issue_id": "77777"}),
        ),
        (
            "add_decision",
            json!({"summary": "use argon2 for passwords", "location": "src/auth/"}),
        ),
        (
            "add_note",
            json!({"summary": "gotcha: react strict mode renders effects twice", "location": "react"}),
        ),
        (
            "add_decision",
            json!({"summary": "x", "supersedes": "evt_nope"}),
        ),
        ("search_events", json!({"query": "argon2"})),
        ("precheck_file", json!({"file_path": "src/app.py"})),
        ("get_score", empty()),
        ("list_projects", empty()),
        ("current_project", empty()),
        ("current_project", json!({"project": "nope"})),
    ]
}

#[test]
fn mcp_replies_match_python() -> TestResult {
    let now = jiff::Timestamp::now().as_second();
    let fixture = tempfile::tempdir()?;
    precheck_fixture(fixture.path(), now)?;
    let actions = vec![
        djf(INIT),
        session(None, read_calls()),
        session(None, write_calls()),
    ];
    let (python, differences) = compare(fixture.path(), &actions, clock)?;
    assert!(
        python
            .steps
            .iter()
            .any(|step| step.stdout.contains("dejafix precheck: src/app.py")),
        "{:?}",
        python.steps
    );
    assert_same("mcp replies", &differences);
    Ok(())
}

#[test]
fn mcp_routing_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    plain_fixture(fixture.path())?;
    let actions = vec![
        djf(INIT),
        session(
            Some("elsewhere"),
            vec![
                ("list_projects", empty()),
                ("current_project", empty()),
                ("current_project", json!({"project": "project"})),
                ("current_project", json!({"project": "nope"})),
                (
                    "add_note",
                    json!({"summary": "routed note", "project": "project"}),
                ),
                ("get_summary", json!({"project": "PROJECT"})),
                (
                    "search_events",
                    json!({"query": "routed", "project": "project"}),
                ),
            ],
        ),
        Action::Mcp(McpSession {
            args: vec!["--root".to_owned(), "{project}".to_owned()],
            cwd: Some("elsewhere"),
            calls: vec![
                ("list_projects", empty()),
                ("current_project", json!({"project": "project"})),
                ("add_note", json!({"summary": "x", "project": "elsewhere"})),
            ],
        }),
    ];
    let (_, differences) = compare(fixture.path(), &actions, clock)?;
    assert_same("mcp routing", &differences);
    Ok(())
}

#[test]
fn brief_matches_python() -> TestResult {
    let now = jiff::Timestamp::now().as_second();
    let fixture = tempfile::tempdir()?;
    precheck_fixture(fixture.path(), now)?;
    let actions = vec![djf(INIT), djf(&["brief"])];
    let (python, differences) = compare(fixture.path(), &actions, clock)?;
    assert!(
        python
            .steps
            .iter()
            .any(|step| step.stdout.contains("dejafix brief")),
        "{:?}",
        python.steps
    );
    assert_same("brief", &differences);
    let fresh = tempfile::tempdir()?;
    plain_fixture(fresh.path())?;
    let (_, differences) = compare(fresh.path(), &[djf(INIT), djf(&["brief"])], clock)?;
    assert_same("brief on a fresh project", &differences);
    Ok(())
}

#[test]
fn global_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    plain_fixture(fixture.path())?;
    fs::write(
        fixture.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "express": "^4"}}"#,
    )?;
    let actions = vec![
        djf(INIT),
        djf(&["global"]),
        djf(&["global", "list"]),
        djf(&["global", "add"]),
        djf(&[
            "global",
            "add",
            "prefer small modules",
            "--tag",
            "javascript",
        ]),
        djf(&["global", "status"]),
        djf(&["global", "list", "--tag", "javascript"]),
        djf(&["global", "list", "--library", "react", "-f", "json"]),
        djf(&["global", "detect"]),
        djf(&["global", "detect", "--format", "json"]),
        djf(&["global", "remove", "pat_000000000000"]),
        djf(&[
            "global",
            "prune",
            "--older-than",
            "0",
            "--confidence",
            "low",
        ]),
        djf(&["global", "import", "missing.json"]),
    ];
    let home = tempfile::tempdir()?;
    fs::create_dir_all(home.path().join(".dejafix/global"))?;
    fs::write(
        home.path().join(".dejafix/global/library_gotchas.jsonl"),
        "{\"discovered\": \"2026-01-01T00:00:00Z\", \"gotcha\": \"effects run twice in strict mode\", \"id\": \"got_0123456789ab\", \"library\": \"react\", \"source_project\": \"web\", \"tags\": [], \"version_range\": null}\n",
    )?;
    let (_, differences) = compare_with_home(fixture.path(), Some(home.path()), &actions, clock)?;
    assert_same("global", &differences);
    Ok(())
}
