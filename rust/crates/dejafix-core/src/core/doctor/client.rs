use serde_json::{Map, Value};
use toml::de::{DeTable, DeValue};

const PIN_MARKERS: [&str; 4] = ["--root", "\"cwd\"", "cwd =", "DEJAFIX_ROOT"];
const ROOT_FLAG: &str = "--root";
const ROOT_VARIABLE: &str = "DEJAFIX_ROOT";
const ENTRY_KEYS: [&str; 5] = ["command", "args", "url", "cwd", "env"];
const SERVER_BINARIES: [&str; 2] = ["djf-mcp", "djf-mcp.exe"];
const PACKAGE: &str = "dejafix";
const PACKAGE_SEPARATORS: [char; 3] = ['.', '@', '='];
const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

#[must_use]
pub fn mentions_dejafix(text: &str) -> bool {
    text.contains(PACKAGE) || text.contains("djf-mcp")
}

fn is_dejafix_program(token: &str) -> bool {
    let lower = token.to_lowercase();
    let name = lower.rsplit(PATH_SEPARATORS).next().unwrap_or(&lower);
    if SERVER_BINARIES.contains(&name) {
        return true;
    }
    !lower.contains(PATH_SEPARATORS)
        && (lower == PACKAGE
            || lower
                .strip_prefix(PACKAGE)
                .is_some_and(|rest| rest.starts_with(PACKAGE_SEPARATORS)))
}

fn strings(entry: &Map<String, Value>, key: &str) -> Vec<String> {
    match entry.get(key) {
        Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn tokens(entry: &Map<String, Value>) -> Vec<String> {
    let mut all = strings(entry, "command");
    all.extend(strings(entry, "args"));
    all
}

fn is_dejafix_entry(key: &str, entry: &Map<String, Value>) -> bool {
    if !ENTRY_KEYS.iter().any(|name| entry.contains_key(*name)) {
        return false;
    }
    key.to_lowercase().contains(PACKAGE)
        || tokens(entry).iter().any(|token| is_dejafix_program(token))
}

fn is_root_token(token: &str) -> bool {
    token == ROOT_FLAG
        || token
            .strip_prefix(ROOT_FLAG)
            .is_some_and(|rest| rest.starts_with('='))
        || token
            .strip_prefix(ROOT_VARIABLE)
            .and_then(|rest| rest.strip_prefix('='))
            .is_some_and(|value| !value.trim().is_empty())
}

fn entry_is_pinned(entry: &Map<String, Value>) -> bool {
    let root_token = tokens(entry).iter().any(|token| is_root_token(token));
    let root_variable = entry
        .get("env")
        .and_then(Value::as_object)
        .and_then(|env| env.get(ROOT_VARIABLE))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    root_token || root_variable || entry.contains_key("cwd")
}

fn collect<'v>(value: &'v Value, entries: &mut Vec<&'v Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match child {
                    Value::Object(entry) if is_dejafix_entry(key, entry) => entries.push(entry),
                    _ => collect(child, entries),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, entries)),
        _ => {}
    }
}

fn json_of(value: &DeValue<'_>) -> Value {
    match value {
        DeValue::String(text) => Value::String(text.to_string()),
        DeValue::Boolean(flag) => Value::Bool(*flag),
        DeValue::Array(items) => {
            Value::Array(items.iter().map(|item| json_of(item.get_ref())).collect())
        }
        DeValue::Table(table) => Value::Object(object_of(table)),
        DeValue::Integer(_) | DeValue::Float(_) | DeValue::Datetime(_) => Value::Null,
    }
}

fn object_of(table: &DeTable<'_>) -> Map<String, Value> {
    table
        .iter()
        .map(|(key, value)| (key.get_ref().to_string(), json_of(value.get_ref())))
        .collect()
}

fn parsed(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text).ok().or_else(|| {
        DeTable::parse(text)
            .ok()
            .map(|table| Value::Object(object_of(table.get_ref())))
    })
}

#[must_use]
pub fn is_pinned_config(text: &str) -> bool {
    let Some(value) = parsed(text) else {
        return PIN_MARKERS.iter().any(|marker| text.contains(marker));
    };
    let mut entries = Vec::new();
    collect(&value, &mut entries);
    entries.into_iter().any(entry_is_pinned)
}
