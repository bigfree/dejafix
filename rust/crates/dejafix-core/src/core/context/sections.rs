use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;

use super::types::{FileNotes, Level, Scored};
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::path::location_to_file;
use crate::core::precheck::age;
use crate::core::text::{char_len, char_prefix, is_python_space, python_strip, universal_newlines};

const SENTENCE_ENDS: [&str; 3] = [". ", "! ", "? "];
const WORD_TRAILING: [char; 5] = [',', ';', ':', '—', '-'];
const GOTCHA_FILES: usize = 5;
const ARCH_LINES: usize = 8;

pub(super) fn length(text: &str) -> i64 {
    i64::try_from(char_len(text)).unwrap_or(i64::MAX)
}

pub(super) fn located<'e>(event: &'e Event<'_>) -> Option<Cow<'e, str>> {
    let location = event.location.as_deref().filter(|at| at.contains(':'))?;
    Some(location_to_file(location).unwrap_or(Cow::Borrowed(location)))
}

pub(super) fn smart_truncate(text: &str, limit: usize) -> Cow<'_, str> {
    if char_len(text) <= limit {
        return Cow::Borrowed(text);
    }
    let head = char_prefix(text, limit);
    let sentence_floor = 40.max(limit / 2);
    for separator in SENTENCE_ENDS {
        if let Some(at) = head.rfind(separator)
            && let Some(before) = head.get(..at)
            && char_len(before) >= sentence_floor
            && let Some(sentence) = head.get(..=at)
        {
            return Cow::Owned(format!("{sentence} …"));
        }
    }
    let word_floor = 20.max(limit / 3);
    if let Some(at) = head.rfind(' ')
        && let Some(before) = head.get(..at)
        && char_len(before) >= word_floor
    {
        return Cow::Owned(format!(
            "{} …",
            before.trim_end_matches(WORD_TRAILING.as_slice())
        ));
    }
    Cow::Owned(format!("{}…", head.trim_end_matches(is_python_space)))
}

fn is_failed_attempt(event: &Event<'_>) -> bool {
    event.kind == EventKind::Attempt && event.outcome == Some(Outcome::Failed)
}

fn is_backfill(event: &Event<'_>) -> bool {
    let backfilled = event
        .notes
        .as_deref()
        .is_some_and(|notes| notes.to_lowercase().contains("auto-backfilled"));
    backfilled || (event.files.len() > 5 && event.kind == EventKind::Note)
}

pub(super) fn warnings(scored: &[Scored<'_, '_>], level: Level, now: jiff::Timestamp) -> String {
    let limit = if level.is_tight() { 3 } else { 8 };
    let mut body = String::new();
    for event in scored
        .iter()
        .map(|scored| scored.event)
        .filter(|event| is_failed_attempt(event))
        .take(limit)
    {
        let summary = match level {
            Level::Emergency => smart_truncate(event.summary.as_str(), 60),
            Level::Ultra => smart_truncate(event.summary.as_str(), 100),
            Level::Full | Level::Compressed => Cow::Borrowed(event.summary.as_str()),
        };
        let location = event
            .location
            .as_deref()
            .filter(|at| !at.is_empty() && level != Level::Emergency);
        let _ = write!(body, "- FAILED: {summary}");
        if let Some(location) = location {
            let _ = write!(body, " [{location}]");
        }
        let _ = writeln!(body, " ({})", age(&event.at, now));
    }
    if body.is_empty() {
        return body;
    }
    format!("### WARNINGS — Do Not Repeat\n{body}")
}

fn listing(
    scored: &[Scored<'_, '_>],
    kind: EventKind,
    level: Level,
    remaining: i64,
    header: &str,
    render: impl Fn(&Event<'_>) -> String,
) -> String {
    let limit = if level.is_tight() { 3 } else { 6 };
    let mut body = String::new();
    let mut chars = 0;
    let mut count = 0;
    for event in scored
        .iter()
        .map(|scored| scored.event)
        .filter(|event| event.kind == kind)
    {
        let line = render(event);
        let size = length(&line);
        if chars + size > remaining {
            break;
        }
        body.push_str(&line);
        chars += size;
        count += 1;
        if count >= limit {
            break;
        }
    }
    if body.is_empty() {
        return body;
    }
    format!("{header}{body}")
}

pub(super) fn decisions(
    scored: &[Scored<'_, '_>],
    level: Level,
    remaining: i64,
    now: jiff::Timestamp,
) -> String {
    listing(
        scored,
        EventKind::Decision,
        level,
        remaining,
        "### Recent Decisions\n",
        |event| {
            let summary = if level == Level::Ultra {
                smart_truncate(event.summary.as_str(), 80)
            } else {
                Cow::Borrowed(event.summary.as_str())
            };
            format!("- {summary} ({})\n", age(&event.at, now))
        },
    )
}

pub(super) fn fixes(
    scored: &[Scored<'_, '_>],
    level: Level,
    remaining: i64,
    now: jiff::Timestamp,
) -> String {
    listing(
        scored,
        EventKind::Fix,
        level,
        remaining,
        "### Relevant Fixes\n",
        |event| {
            let summary = if level == Level::Ultra {
                smart_truncate(event.summary.as_str(), 80)
            } else {
                Cow::Borrowed(event.summary.as_str())
            };
            let mut line = format!("- Fixed: {summary} ({})", age(&event.at, now));
            if level.is_roomy() && !event.files.is_empty() {
                let files: Vec<&str> = event.files.iter().take(3).map(AsRef::as_ref).collect();
                let _ = write!(line, "\n  Files: {}", files.join(", "));
            }
            line.push('\n');
            line
        },
    )
}

fn first_note(event: &Event<'_>) -> String {
    let mut label = event.kind.as_str().to_uppercase();
    if let Some(outcome) = event.outcome {
        let _ = write!(label, " ({})", outcome.as_str());
    }
    format!("{label}: {}", smart_truncate(event.summary.as_str(), 60))
}

pub(super) fn file_gotchas(
    scored: &[Scored<'_, '_>],
    focus: Option<&str>,
    level: Level,
    remaining: i64,
) -> String {
    if level == Level::Emergency {
        return String::new();
    }
    let mut index: HashMap<Cow<'_, str>, usize> = HashMap::new();
    let mut files: Vec<FileNotes<'_>> = Vec::new();
    for event in scored
        .iter()
        .map(|scored| scored.event)
        .filter(|event| !is_backfill(event))
    {
        let cited = event
            .files
            .iter()
            .map(|file| Cow::Borrowed(file.as_ref()))
            .chain(located(event));
        for file in cited {
            if focus.is_some_and(|focus| !file.contains(focus)) {
                continue;
            }
            if let Some(&at) = index.get(&*file) {
                if let Some(entry) = files.get_mut(at) {
                    entry.count += 1;
                }
            } else {
                index.insert(file.clone(), files.len());
                files.push(FileNotes {
                    file,
                    count: 1,
                    first: first_note(event),
                });
            }
        }
    }
    if files.is_empty() {
        return String::new();
    }
    files.sort_by_key(|notes| Reverse(notes.count));
    let mut body = String::new();
    let mut chars = 0;
    for notes in files.iter().take(GOTCHA_FILES) {
        let line = format!("- `{}`: {}\n", notes.file, notes.first);
        let size = length(&line);
        if chars + size > remaining {
            break;
        }
        body.push_str(&line);
        chars += size;
    }
    if body.is_empty() {
        return body;
    }
    format!("### File Gotchas\n{body}")
}

pub(super) fn arch_context(map: &str, focus: Option<&str>, remaining: i64) -> String {
    let content = universal_newlines(map);
    if content.to_lowercase().contains("not created yet") {
        return String::new();
    }
    let focus = focus.map(|focus| focus.trim_end_matches('/'));
    let relevant: Vec<&str> = python_strip(&content)
        .split('\n')
        .filter(|line| {
            focus.is_some_and(|focus| line.contains(focus))
                || (line.starts_with("- ") && (line.contains('`') || line.contains('/')))
        })
        .map(python_strip)
        .take(ARCH_LINES)
        .collect();
    if relevant.is_empty() {
        return String::new();
    }
    let mut result = String::from("### Architecture Context\n");
    let mut chars = length(&result);
    for line in relevant {
        let size = length(line) + 1;
        if chars + size > remaining {
            break;
        }
        result.push_str(line);
        result.push('\n');
        chars += size;
    }
    result
}
