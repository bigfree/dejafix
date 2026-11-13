use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use super::types::Block;
use crate::core::hooks::{HOOK_MARKER_END, HOOK_MARKER_START};
use crate::core::path::MEMORY_DIR;
use crate::core::scaffold::{BRIDGE_END, BRIDGE_START};

pub const LEGACY_DIR: &str = ".projectmem";

const LEGACY_CONFIG_TOKENS: [&str; 7] = [
    "projectmem.mcp_server",
    "mcp_servers.projectmem",
    "\"projectmem\":",
    "pjm-mcp",
    "pjm-edit-hook",
    "PJM_EDIT_HOOK_LOG",
    "PROJECTMEM_",
];

pub const HOOK_BLOCK: Block = Block {
    old_start: "# >>> projectmem auto-capture >>>",
    old_end: "# <<< projectmem auto-capture <<<",
    new_start: HOOK_MARKER_START,
    new_end: HOOK_MARKER_END,
};

pub const BRIDGE_BLOCK: Block = Block {
    old_start: "<!-- >>> projectmem bridge >>> -->",
    old_end: "<!-- <<< projectmem bridge <<< -->",
    new_start: BRIDGE_START,
    new_end: BRIDGE_END,
};

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static LEGACY_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.projectmem\b").expect("valid legacy path pattern"));

fn without_old_blocks(text: &str, block: &Block) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(block.old_start) {
        let (before, from_start) = rest.split_at(start);
        let Some(end) = from_start.find(block.old_end) else {
            break;
        };
        kept.push_str(before);
        let after = from_start
            .get(end + block.old_end.len()..)
            .unwrap_or_default();
        rest = after.strip_prefix('\n').unwrap_or(after);
    }
    kept.push_str(rest);
    kept
}

#[must_use]
pub fn convert_block(text: &str, block: &Block) -> Option<String> {
    if !text.contains(block.old_start) {
        return None;
    }
    if text.contains(block.new_start) {
        return Some(without_old_blocks(text, block));
    }
    Some(
        text.replace(block.old_start, block.new_start)
            .replace(block.old_end, block.new_end),
    )
}

#[must_use]
pub fn has_legacy_path(text: &str) -> bool {
    LEGACY_PATH.is_match(text)
}

#[must_use]
pub fn rewrite_legacy_paths(text: &str) -> Option<String> {
    match LEGACY_PATH.replace_all(text, MEMORY_DIR) {
        Cow::Borrowed(_) => None,
        Cow::Owned(updated) => Some(updated),
    }
}

#[must_use]
pub fn is_stale_config(text: &str) -> bool {
    LEGACY_CONFIG_TOKENS
        .iter()
        .any(|token| text.contains(token))
}
