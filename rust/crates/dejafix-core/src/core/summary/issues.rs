use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;

use super::types::{Group, IssueFile, Listed};
use crate::core::event::{EventKind, IssueId, Outcome};
use crate::core::log::EventLog;
use crate::core::text::decimal_value;

pub(super) fn groups<'l, 'a>(log: &'l EventLog<'a>) -> Vec<Group<'l, 'a>> {
    let mut groups: Vec<Group<'l, 'a>> = Vec::new();
    let mut position: HashMap<&'l str, usize> = HashMap::new();
    for event in log.events() {
        let Some(issue) = event
            .issue
            .as_ref()
            .map(IssueId::as_str)
            .filter(|issue| !issue.is_empty())
        else {
            continue;
        };
        if let Some(group) = position.get(issue).and_then(|index| groups.get_mut(*index)) {
            group.events.push(event);
        } else {
            position.insert(issue, groups.len());
            groups.push(Group {
                id: issue,
                events: vec![event],
            });
        }
    }
    groups
}

fn issue_order(left: &str, right: &str) -> Ordering {
    match (decimal_value(left), decimal_value(right)) {
        (Some(left_value), Some(right_value)) => {
            left_value.cmp(&right_value).then_with(|| left.cmp(right))
        }
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => left.cmp(right),
    }
}

pub(super) fn listed<'l, 'a>(groups: &'l [Group<'l, 'a>]) -> Vec<Listed<'l, 'a>> {
    let mut listed: Vec<Listed<'l, 'a>> = groups
        .iter()
        .filter_map(|group| {
            let issue = group
                .events
                .iter()
                .find(|event| event.kind == EventKind::Issue)?;
            let fix = group
                .events
                .iter()
                .rev()
                .find(|event| event.kind == EventKind::Fix)
                .copied();
            Some(Listed {
                id: group.id,
                issue,
                events: &group.events,
                fix,
            })
        })
        .collect();
    listed.sort_by(|left, right| issue_order(right.id, left.id));
    listed
}

#[must_use]
pub fn slugify_title(text: &str) -> String {
    let lowered = text.to_lowercase();
    let mut slug = String::with_capacity(lowered.len());
    let mut in_run = false;
    for character in lowered.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    let base = if trimmed.is_empty() { "issue" } else { trimmed };
    let cut = base
        .get(..base.len().min(48))
        .unwrap_or(base)
        .trim_matches('-');
    if cut.is_empty() {
        "issue".to_owned()
    } else {
        cut.to_owned()
    }
}

fn safe_file_stem(issue: &str) -> bool {
    !issue.contains(['/', '\\', '\0']) && (!cfg!(windows) || !issue.contains(':'))
}

#[must_use]
pub fn issue_files<'l>(log: &'l EventLog<'_>) -> Vec<IssueFile<'l>> {
    groups(log)
        .into_iter()
        .filter_map(|group| {
            let issue = group
                .events
                .iter()
                .find(|event| event.kind == EventKind::Issue)?;
            let mut contents = format!("# #{} {}\n\n", group.id, issue.summary.as_str());
            for event in &group.events {
                let _ = write!(
                    contents,
                    "- {} `{}`: {}",
                    event.at.as_str(),
                    event.kind.as_str(),
                    event.summary.as_str()
                );
                if let Some(location) = event
                    .location
                    .as_deref()
                    .filter(|location| !location.is_empty())
                {
                    let _ = write!(contents, " [{location}]");
                }
                if let Some(outcome) = event.outcome {
                    let _ = write!(contents, " ({})", Outcome::as_str(outcome));
                }
                contents.push('\n');
            }
            let name = safe_file_stem(group.id)
                .then(|| format!("{}-{}.md", group.id, slugify_title(issue.summary.as_str())));
            Some(IssueFile {
                issue: group.id,
                name,
                contents,
            })
        })
        .collect()
}
