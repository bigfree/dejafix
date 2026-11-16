use super::config::sanitize;
use super::types::{Direction, Link, LinkedBlock};
use crate::core::lessons::{Lesson, LessonKind, clip, issue_label, provenance};

pub const MAX_BLOCK_PROJECTS: usize = 5;
const QUERY_CHARS: usize = 80;

#[must_use]
pub fn lesson_line(lesson: &Lesson<'_, '_>) -> String {
    let record = lesson.record;
    let summary = clip(&sanitize(&record.summary, usize::MAX));
    let body = match lesson.kind {
        LessonKind::Issue => format!(
            "- open {}{summary} [{}]",
            issue_label(record),
            clip(lesson.files.first().map_or("", String::as_str))
        ),
        LessonKind::Attempt => {
            let mark = if record.outcome == "failed" { "x" } else { "~" };
            format!("{mark} {summary}{}", provenance(record, true))
        }
        LessonKind::Decision => format!("- decision: {summary}{}", provenance(record, false)),
    };
    format!("    {}", sanitize(&body, usize::MAX))
}

#[must_use]
pub fn linked_report(relative: &str, blocks: Vec<LinkedBlock>) -> String {
    let mut blocks = blocks;
    blocks.sort_unstable_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.id.cmp(&right.id))
    });
    blocks.truncate(MAX_BLOCK_PROJECTS);
    if blocks.is_empty() {
        return String::new();
    }
    let mut lines = vec![format!(
        "dejafix lessons from linked projects for {relative}:"
    )];
    for block in &blocks {
        if block.relation.is_empty() {
            lines.push(format!("  {}:", block.id));
        } else {
            lines.push(format!("  {} (\"{}\"):", block.id, block.relation));
        }
        lines.extend(block.lines.iter().cloned());
        if block.hidden > 0 {
            lines.push(format!(
                "    ... and {} more: search(project=\"{}\", query=\"{}\")",
                block.hidden,
                block.id,
                sanitize(&block.query.replace('/', " "), QUERY_CHARS)
            ));
        }
    }
    lines.join("\n")
}

#[must_use]
pub fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Declared => "links here",
        Direction::Reverse => "linked from there",
        Direction::Both => "linked both ways",
    }
}

#[must_use]
pub fn links_section(entries: &[(&Link, Option<usize>)]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut lines = vec!["Linked projects (read their memory with project=\"<name>\"):".to_owned()];
    for (link, open) in entries {
        let relation = if link.relation.is_empty() {
            String::new()
        } else {
            format!(": \"{}\"", link.relation)
        };
        let count = open.map_or_else(String::new, |open| {
            format!(" {open} open issue(s) on its boundary.")
        });
        lines.push(format!(
            "  - {} ({}), {}{relation}.{count}",
            link.id,
            link.identity,
            direction_text(link.direction)
        ));
    }
    lines.join("\n")
}
