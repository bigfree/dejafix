use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use dejafix_core::{McpSettings, McpState};
use regex::Regex;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use serde_json::{Value, json};

use super::DejafixServer;
use super::instructions;
use super::lax::{lax_boolean, lax_integer};
use super::schema::tools;

const NAMES: [&str; 19] = [
    "start_session",
    "get_instructions",
    "get_summary",
    "get_project_map",
    "get_plan",
    "precheck_file",
    "precheck_files",
    "get_issue",
    "search_events",
    "get_score",
    "get_context",
    "get_global_gotchas",
    "log_issue",
    "record_attempt",
    "record_fix",
    "add_decision",
    "add_note",
    "list_projects",
    "current_project",
];

fn table() -> Vec<Tool> {
    tools().unwrap().to_vec()
}

fn properties(tool: &Tool) -> BTreeSet<String> {
    tool.input_schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

fn required(tool: &Tool) -> BTreeSet<String> {
    tool.input_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn split_args(list: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut depth = 0_i32;
    for character in list.chars() {
        match (quote, character) {
            (Some(open), _) => {
                if character == open {
                    quote = None;
                }
            }
            (None, '"' | '\'') => quote = Some(character),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                args.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    args.push(current);
    args.into_iter()
        .map(|arg| arg.trim().to_owned())
        .filter(|arg| !arg.is_empty())
        .collect()
}

fn calls(text: &str, tool: &str) -> Vec<Vec<String>> {
    let pattern = Regex::new(&format!(r"(?:^|[^\w.]){tool}\(([^()]*)\)")).unwrap();
    pattern
        .captures_iter(text)
        .map(|found| split_args(&found[1]))
        .collect()
}

fn names_shown(args: &[String]) -> Vec<String> {
    let keyword = Regex::new(r"^(\w+)\s*=[^=]").unwrap();
    let bare = Regex::new(r"^[A-Za-z_]\w*$").unwrap();
    args.iter()
        .filter_map(|arg| {
            if let Some(found) = keyword.captures(&format!("{arg} ")) {
                return Some(found[1].to_owned());
            }
            (bare.is_match(arg) && !["None", "True", "False"].contains(&arg.as_str()))
                .then(|| arg.clone())
        })
        .collect()
}

#[test]
fn the_table_lists_the_nineteen_tools_in_python_s_order() {
    let names: Vec<String> = table().iter().map(|tool| tool.name.to_string()).collect();
    assert_eq!(names, NAMES);
}

#[test]
fn every_call_shown_in_the_instructions_uses_real_and_required_arguments() {
    let text = instructions(true);
    let mut wrong = Vec::new();
    for tool in table() {
        let params = properties(&tool);
        let needed = required(&tool);
        for args in calls(&text, &tool.name) {
            let named: BTreeSet<String> = names_shown(&args).into_iter().collect();
            wrong.extend(
                named
                    .iter()
                    .filter(|name| !params.contains(*name))
                    .map(|name| format!("{}({name})", tool.name)),
            );
            if needed.difference(&named).count() > args.len() - named.len() {
                wrong.push(format!("{} misses {needed:?}", tool.name));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
    assert_ne!(calls(&text, "precheck_file"), Vec::<Vec<String>>::new());
}

#[test]
fn the_instructions_document_supersedes_start_session_and_parallel_calls() {
    let text = instructions(false);
    assert!(text.to_lowercase().contains("supersede"));
    assert!(text.contains("start_session()"));
    assert!(text.contains("in parallel"));
    assert!(!text.contains("Do NOT batch"));
    assert!(!text.contains("1. get_instructions") && !text.contains("1. `get_instructions"));
    assert!(!text.contains("SERVES SEVERAL PROJECTS"));
    let global = instructions(true).to_lowercase();
    assert!(global.contains("do not look the project up before you start"));
    assert!(global.contains("only if a call has actually failed"));
}

#[test]
fn every_write_tool_states_its_record_length() {
    let limits = BTreeMap::from([
        ("log_issue", 140),
        ("record_attempt", 300),
        ("record_fix", 300),
        ("add_decision", 300),
        ("add_note", 300),
    ]);
    for tool in table() {
        let Some(limit) = limits.get(tool.name.as_ref()) else {
            continue;
        };
        let description = tool.input_schema["properties"]["summary"]["description"]
            .as_str()
            .unwrap_or_default();
        assert!(
            description.contains(&format!("under {limit} characters")),
            "{}",
            tool.name
        );
    }
}

fn project(base: &Path, name: &str) -> PathBuf {
    let root = base.join(name);
    fs::create_dir_all(root.join(".dejafix/issues")).unwrap();
    fs::write(root.join(".dejafix/config.toml"), "").unwrap();
    fs::write(root.join(".dejafix/events.jsonl"), "").unwrap();
    root
}

fn server(base: &Path, pinned: Option<&Path>) -> DejafixServer {
    DejafixServer::new(McpState::new(McpSettings {
        pinned: pinned.map(Path::to_path_buf),
        cwd: base.to_path_buf(),
        home: Some(base.join("home")),
        dejafix_home: base.join("home/.dejafix"),
        redact: true,
        zone: jiff::tz::TimeZone::UTC,
    }))
    .unwrap()
}

fn text(result: &CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|block| block.as_text())
        .map(|text| text.text.clone())
        .unwrap_or_default()
}

fn arguments(value: &Value) -> serde_json::Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[tokio::test]
async fn a_client_sees_the_instructions_tools_and_replies() {
    let temp = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(temp.path()).unwrap();
    let root = project(&base, "alpha");
    let (server_side, client_side) = tokio::io::duplex(1 << 16);
    let running = server(&base, Some(&root));
    tokio::spawn(async move {
        if let Ok(service) = running.serve(server_side).await {
            let _ = service.waiting().await;
        }
    });
    let client = ().serve(client_side).await.unwrap();
    let info = client.peer_info().unwrap();
    assert_eq!(
        info.instructions.as_deref(),
        Some(instructions(false).as_str())
    );
    let listed = client.list_all_tools().await.unwrap();
    assert_eq!(listed.len(), 19);
    let noted = client
        .call_tool(
            CallToolRequestParams::new("add_note")
                .with_arguments(arguments(&json!({"summary": "models live in src"}))),
        )
        .await
        .unwrap();
    assert_eq!(noted.is_error, Some(false));
    assert!(text(&noted).starts_with("Recorded note evt_"));
    assert_eq!(
        noted.structured_content,
        Some(json!({"result": text(&noted)}))
    );
    let events = fs::read_to_string(root.join(".dejafix/events.jsonl")).unwrap();
    assert!(events.contains("models live in src"));
    let precheck = client
        .call_tool(
            CallToolRequestParams::new("precheck_file")
                .with_arguments(arguments(&json!({"file_path": ".dejafix/summary.md"}))),
        )
        .await
        .unwrap();
    assert!(text(&precheck).contains("memory file"));
    let invalid = client
        .call_tool(
            CallToolRequestParams::new("search_events")
                .with_arguments(arguments(&json!({"query": "x", "limit": 0}))),
        )
        .await
        .unwrap();
    assert_eq!(invalid.is_error, Some(true));
    assert!(text(&invalid).starts_with("Error executing tool search_events: limit"));
    let missing = client
        .call_tool(CallToolRequestParams::new("get_issue"))
        .await
        .unwrap();
    assert_eq!(missing.is_error, Some(true));
    assert!(text(&missing).contains("missing field `issue_id`"));
    let unknown = client
        .call_tool(CallToolRequestParams::new("nothing"))
        .await
        .unwrap();
    assert_eq!(text(&unknown), "Unknown tool: nothing");
    let refused = client
        .call_tool(
            CallToolRequestParams::new("add_note")
                .with_arguments(arguments(&json!({"summary": "x", "project": "beta"}))),
        )
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    assert!(
        text(&refused).starts_with("dejafix tool error: ResolutionError: This server is pinned")
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_global_server_adds_the_routing_instructions() {
    let temp = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(temp.path()).unwrap();
    let running = server(&base, None);
    let (server_side, client_side) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(service) = running.serve(server_side).await {
            let _ = service.waiting().await;
        }
    });
    let client = ().serve(client_side).await.unwrap();
    let info = client.peer_info().unwrap();
    assert_eq!(
        info.instructions.as_deref(),
        Some(instructions(true).as_str())
    );
    let listed = client
        .call_tool(CallToolRequestParams::new("list_projects"))
        .await
        .unwrap();
    assert!(text(&listed).starts_with("No projects registered."));
    client.cancel().await.unwrap();
}

#[test]
fn integers_are_read_as_leniently_as_pydantic_reads_them() {
    for (value, expected) in [
        (json!(5), Some(5)),
        (json!("5"), Some(5)),
        (json!(" 5 "), Some(5)),
        (json!("5.0"), Some(5)),
        (json!(5.0), Some(5)),
        (json!("+5"), Some(5)),
        (json!("-5"), Some(-5)),
        (json!("5_0"), Some(50)),
        (json!(true), Some(1)),
        (json!(5.5), None),
        (json!("0x5"), None),
        (json!(""), None),
        (json!("５"), None),
        (json!("5__0"), None),
        (json!("_5"), None),
        (json!("5.5"), None),
        (json!(null), None),
    ] {
        assert_eq!(lax_integer(&value), expected, "{value}");
    }
}

#[test]
fn booleans_are_read_as_leniently_as_pydantic_reads_them() {
    for (value, expected) in [
        (json!(true), Some(true)),
        (json!("true"), Some(true)),
        (json!("True"), Some(true)),
        (json!("yes"), Some(true)),
        (json!("on"), Some(true)),
        (json!("t"), Some(true)),
        (json!("1"), Some(true)),
        (json!(1), Some(true)),
        (json!(0), Some(false)),
        (json!("off"), Some(false)),
        (json!("f"), Some(false)),
        (json!("n"), Some(false)),
        (json!("2"), None),
        (json!(2), None),
        (json!(""), None),
        (json!(null), None),
    ] {
        assert_eq!(lax_boolean(&value), expected, "{value}");
    }
}
