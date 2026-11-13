use super::types::{
    HOOK_MARKER_END, HOOK_MARKER_START, HOOK_SHEBANG, HookChange, OLD_HOOK_SHEBANGS, Removal,
};
use crate::core::text::python_strip;

fn outside_blocks(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(HOOK_MARKER_START) {
        let (before, from_start) = rest.split_at(start);
        let after_start = from_start
            .get(HOOK_MARKER_START.len()..)
            .unwrap_or_default();
        let Some(end) = after_start.find(HOOK_MARKER_END) else {
            break;
        };
        kept.push_str(before);
        rest = after_start
            .get(end + HOOK_MARKER_END.len()..)
            .unwrap_or_default();
    }
    kept.push_str(rest);
    kept
}

fn with_current_shebang(updated: String) -> String {
    let (first, rest) = updated.split_once('\n').unwrap_or((&updated, ""));
    if !OLD_HOOK_SHEBANGS.contains(&first) {
        return updated;
    }
    if python_strip(&outside_blocks(rest)).is_empty() {
        format!("{HOOK_SHEBANG}{rest}")
    } else {
        updated
    }
}

#[must_use]
pub fn sync_hook(existing: Option<&str>, snippet: &str) -> HookChange {
    let Some(content) = existing else {
        return HookChange::Installed(format!("{HOOK_SHEBANG}{snippet}"));
    };
    let Some(start) = content.find(HOOK_MARKER_START) else {
        return HookChange::Installed(format!("{}\n\n{snippet}", content.trim_end_matches('\n')));
    };
    let (head, from_start) = content.split_at(start);
    let Some(end) = from_start.find(HOOK_MARKER_END) else {
        return HookChange::Skipped;
    };
    let after = from_start
        .get(end + HOOK_MARKER_END.len()..)
        .unwrap_or_default();
    let tail = after.strip_prefix('\n').unwrap_or(after);
    let updated = with_current_shebang(format!("{head}{snippet}{tail}"));
    if updated == content {
        HookChange::Current
    } else {
        HookChange::Refreshed(updated)
    }
}

#[must_use]
pub fn remove_blocks(content: &str) -> Removal {
    if !content.contains(HOOK_MARKER_START) {
        return Removal::Untouched;
    }
    let mut joined = String::with_capacity(content.len());
    let mut skipping = false;
    let mut first = true;
    for line in content.split('\n') {
        if line.contains(HOOK_MARKER_START) {
            skipping = true;
            continue;
        }
        if line.contains(HOOK_MARKER_END) {
            skipping = false;
            continue;
        }
        if skipping {
            continue;
        }
        if !first {
            joined.push('\n');
        }
        first = false;
        joined.push_str(line);
    }
    let remaining = python_strip(&joined);
    let only_shebang =
        remaining == HOOK_SHEBANG.trim_end_matches('\n') || OLD_HOOK_SHEBANGS.contains(&remaining);
    if remaining.is_empty() || only_shebang {
        Removal::Delete
    } else {
        Removal::Write(format!("{remaining}\n"))
    }
}
