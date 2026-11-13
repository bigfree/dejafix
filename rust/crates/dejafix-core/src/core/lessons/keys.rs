use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use crate::core::json::write_python_json;
use crate::core::text::python_strip;

const SKIPPED_PREFIXES: [&str; 2] = [".dejafix/", ".claude/"];
const SESSION_CHARS: usize = 128;

static PATCH_FILE_HEADER: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(?m)^\*\*\* (?:Update File|Add File|Delete File|Move to): (.+)$").ok()
});

static WORKTREE_SEGMENT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"/\.claude/worktrees/[^/]+/").ok());

const CRC_POLYNOMIAL: u32 = 0xEDB8_8320;

#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(u32::MAX, |crc, byte| {
        (0..8).fold(crc ^ u32::from(*byte), |value, _| {
            if value & 1 == 1 {
                (value >> 1) ^ CRC_POLYNOMIAL
            } else {
                value >> 1
            }
        })
    })
}

#[must_use]
pub fn marker_name(file_key: &str) -> String {
    format!("{:08x}", crc32(file_key.as_bytes()))
}

#[must_use]
pub fn session_key(id: &str) -> String {
    id.chars()
        .take(SESSION_CHARS)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[must_use]
pub fn edited_paths(tool_input: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(input)) = tool_input else {
        return Vec::new();
    };
    if let Some(Value::String(file_path)) = input.get("file_path") {
        return vec![file_path.clone()];
    }
    let command = match input.get("command") {
        Some(Value::String(command)) => command.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return Vec::new(),
    };
    PATCH_FILE_HEADER
        .as_ref()
        .map(|pattern| {
            pattern
                .captures_iter(&command)
                .filter_map(|found| found.get(1))
                .map(|path| python_strip(path.as_str()).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[must_use]
pub fn main_checkout(path: &str) -> String {
    WORKTREE_SEGMENT.as_ref().map_or_else(
        || path.to_owned(),
        |pattern| pattern.replace(path, "/").into_owned(),
    )
}

#[must_use]
pub fn is_skipped(relative: &str) -> bool {
    SKIPPED_PREFIXES
        .iter()
        .any(|prefix| relative.starts_with(prefix))
}

#[must_use]
pub fn hook_output(context: &str) -> String {
    let value = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": context,
        }
    });
    python_json(&value)
}

#[must_use]
pub fn python_json(value: &Value) -> String {
    let mut out = Vec::new();
    let _ = write_python_json(&mut out, value);
    String::from_utf8(out).unwrap_or_default()
}
