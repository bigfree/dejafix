use std::fmt::Write as _;

use super::files::{clip_record, key_files};
use super::issues::{groups, listed};
use super::types::{Listed, SummaryInput};
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::log::EventLog;
use crate::core::text::collapse_whitespace;

const RECENT_FIXED_ISSUES: usize = 10;
const RECENT_DECISIONS: usize = 15;
const OPEN_IN_FULL: usize = 20;
const RECENT_NOTES: usize = 10;
const KEY_FILES: usize = 20;
const PURPOSE_PLACEHOLDER: &str = "Replace this placeholder with a concise description of what this project does, who it serves, and the main technologies or runtime assumptions.";

fn location_suffix(out: &mut String, location: Option<&str>) {
    let collapsed = collapse_whitespace(location.unwrap_or(""));
    if !collapsed.is_empty() {
        let _ = write!(out, " [{collapsed}]");
    }
}

fn has_location(location: Option<&str>) -> bool {
    !collapse_whitespace(location.unwrap_or("")).is_empty()
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn issue_lines(out: &mut String, item: &Listed<'_, '_>) {
    let summary = clip_record(item.issue.summary.as_str());
    if let Some(fix) = item.fix {
        let _ = write!(out, "- [DONE] #{} {summary}", item.id);
        if has_location(item.issue.location.as_deref()) {
            location_suffix(out, item.issue.location.as_deref());
        } else {
            location_suffix(out, fix.location.as_deref());
        }
        out.push_str(" (fixed)\n");
        return;
    }
    let _ = write!(out, "- [OPEN] #{} {summary}", item.id);
    location_suffix(out, item.issue.location.as_deref());
    out.push_str(" (open)\n");
    let lessons: Vec<&&Event<'_>> = item
        .events
        .iter()
        .filter(|event| {
            event.kind == EventKind::Attempt
                && matches!(event.outcome, Some(Outcome::Failed | Outcome::Partial))
        })
        .collect();
    for lesson in lessons.iter().skip(lessons.len().saturating_sub(3)) {
        let tag = if lesson.outcome == Some(Outcome::Partial) {
            "Partial attempt"
        } else {
            "Failed attempt"
        };
        let _ = write!(out, "  - {tag}: {}", clip_record(lesson.summary.as_str()));
        location_suffix(out, lesson.location.as_deref());
        out.push('\n');
    }
}

fn record_line(out: &mut String, event: &Event<'_>) {
    let _ = write!(out, "- {}", clip_record(event.summary.as_str()));
    location_suffix(out, event.location.as_deref());
    out.push('\n');
}

#[must_use]
pub fn render_summary(log: &EventLog<'_>, input: &SummaryInput<'_>) -> String {
    let mut out = String::with_capacity(8_192);
    let _ = write!(
        out,
        "# dejafix - {}\n\n_Last updated: {}_\n\n## Project purpose\n{}\n\n## Recent issues\n",
        input.project_name,
        input.today,
        input.purpose.unwrap_or(PURPOSE_PLACEHOLDER)
    );
    let groups = groups(log);
    let items = listed(&groups);
    if items.is_empty() {
        out.push_str("- No issues logged yet.\n");
    } else {
        let (open, fixed): (Vec<&Listed<'_, '_>>, Vec<&Listed<'_, '_>>) =
            items.iter().partition(|item| item.fix.is_none());
        for item in open.iter().take(OPEN_IN_FULL) {
            issue_lines(&mut out, item);
        }
        let more_open: Vec<String> = open
            .iter()
            .skip(OPEN_IN_FULL)
            .map(|item| format!("#{}", item.id))
            .collect();
        if !more_open.is_empty() {
            let _ = writeln!(
                out,
                "- [OPEN] {} older open issue{}, by id: {} (`get_issue(issue_id)` for details)",
                more_open.len(),
                plural(more_open.len()),
                more_open.join(", ")
            );
        }
        for item in fixed.iter().take(RECENT_FIXED_ISSUES) {
            issue_lines(&mut out, item);
        }
        let hidden = fixed.len().saturating_sub(RECENT_FIXED_ISSUES);
        if hidden > 0 {
            let _ = writeln!(
                out,
                "- ...and {hidden} older fixed issue{} not shown (`djf search`, or `get_issue(issue_id)` via MCP)",
                plural(hidden)
            );
        }
    }
    out.push_str("\n## Decisions\n");
    let decisions: Vec<&Event<'_>> = log
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Decision && !log.is_retired(&event.id))
        .collect();
    if decisions.is_empty() {
        out.push_str("- No decisions logged yet.\n");
    } else {
        let hidden = decisions.len().saturating_sub(RECENT_DECISIONS);
        if hidden > 0 {
            let _ = writeln!(
                out,
                "- ...{hidden} earlier decision{} not shown (`djf search`, or `search_events(query)` via MCP)",
                plural(hidden)
            );
        }
        for event in decisions.iter().skip(hidden) {
            record_line(&mut out, event);
        }
    }
    out.push_str("\n## Notes\n");
    let notes: Vec<&Event<'_>> = log
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Note)
        .collect();
    if notes.is_empty() {
        out.push_str("- No notes logged yet.\n");
    } else {
        for event in notes.iter().skip(notes.len().saturating_sub(RECENT_NOTES)) {
            record_line(&mut out, event);
        }
    }
    out.push_str("\n## Key files\n");
    let files = key_files(log);
    if files.is_empty() {
        out.push_str("- No key files logged yet.\n");
    } else {
        for (path, _) in files.iter().take(KEY_FILES) {
            let _ = writeln!(out, "- `{path}`");
        }
    }
    out.push_str("\n## Open questions\n- None logged yet.\n");
    out
}
