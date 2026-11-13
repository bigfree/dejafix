use super::types::BridgeChange;

pub const BRIDGE_START: &str = "<!-- >>> dejafix bridge >>> -->";
pub const BRIDGE_END: &str = "<!-- <<< dejafix bridge <<< -->";

const BRIDGE_BODY: &str = r#"SESSION START — call `start_session()` once, BEFORE answering ANY
question about this project. It returns the project's mandatory
workflow rules, the memory summary and the current plan in one
response. Do NOT answer from conversation history or by re-reading
package.json / README / source files. Call `get_project_map()` only
when the file layout matters.

BEFORE modifying ANY file:
  - Call `precheck_file(file_path)` — check failure history first.

DURING work — use MCP write tools, NEVER edit `.dejafix/`
files directly via filesystem write:
  - On a bug discovery → `log_issue(summary, location)`.
  - After each fix attempt → `record_attempt(summary, outcome)`.
  - After confirmation → `record_fix(summary)`.
  - On a design choice → `add_decision(summary)`.
  - On a gotcha / setup detail → `add_note(summary)`.
  - When a new decision REPLACES an older one → pass
    `supersedes="<old event id>"` to `add_decision`. The log stays
    append-only; the old decision is tagged retired and drops out of
    summary.md, so the summary never shows two answers to the same
    question. add_decision replies with each decision's id;
    search_events(query) finds older ones.

Editing `.dejafix/summary.md` or `.dejafix/PROJECT_MAP.md`
directly bypasses event logging and breaks audit replay. The
summary file regenerates from `events.jsonl` automatically — write
via the MCP tools and the summary will follow.

Do not re-scan source files when MCP tools can give you the same
answer for a fraction of the tokens a re-scan costs. This is not optional.
"#;

#[must_use]
pub fn bridge_block(project_name: Option<&str>) -> String {
    let named = project_name
        .filter(|name| !name.is_empty())
        .map(|name| {
            format!(
                "This project is registered with dejafix as **{name}**.\nPass `project=\"{name}\"` on any dejafix tool call.\n\n"
            )
        })
        .unwrap_or_default();
    format!(
        "{BRIDGE_START}\n## dejafix (MANDATORY)\n\nThis project uses dejafix for persistent memory + workflow rules.\n\n{named}{BRIDGE_BODY}{BRIDGE_END}\n"
    )
}

#[must_use]
pub fn apply_bridge(
    existing: Option<&str>,
    title: &str,
    project_name: Option<&str>,
) -> BridgeChange {
    let bridge = bridge_block(project_name);
    let Some(content) = existing else {
        return BridgeChange::Created(format!("# {title}\n\n{bridge}"));
    };
    if let (Some(start), Some(end)) = (content.find(BRIDGE_START), content.find(BRIDGE_END)) {
        let head = content.get(..start).unwrap_or_default();
        let tail = content.get(end + BRIDGE_END.len()..).unwrap_or_default();
        let refreshed = format!("{head}{}{tail}", bridge.trim_end());
        if refreshed == content {
            return BridgeChange::Unchanged;
        }
        return BridgeChange::Refreshed(refreshed);
    }
    BridgeChange::Appended(format!("{}\n\n{bridge}", content.trim_end_matches('\n')))
}
