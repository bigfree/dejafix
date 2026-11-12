use std::fmt::Write as _;

use regex::{Regex, RegexBuilder};

use super::types::SearchQuery;
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::log::EventLog;
use crate::core::text::casefold;

const REGEX_SIZE_LIMIT: usize = 1 << 20;
const REGEX_HINT: &[char] = &['|', '*', '?', '+', '(', ')', '[', ']', '\\'];

fn fields<'e>(event: &'e Event<'_>) -> impl Iterator<Item = &'e str> {
    std::iter::once(event.summary.as_str())
        .chain(event.notes.as_deref().filter(|notes| !notes.is_empty()))
        .chain(
            event
                .location
                .as_deref()
                .filter(|location| !location.is_empty()),
        )
        .chain(event.files.iter().map(AsRef::as_ref))
}

fn compile(text: &str) -> Option<Regex> {
    RegexBuilder::new(text)
        .case_insensitive(true)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
        .ok()
}

#[must_use]
pub fn search<'l, 'a>(log: &'l EventLog<'a>, query: &SearchQuery<'_>) -> Vec<&'l Event<'a>> {
    let pattern = if query.regex {
        compile(query.text)
    } else {
        None
    };
    let needle = casefold(query.text);
    log.events()
        .iter()
        .filter(|event| match &pattern {
            Some(pattern) => fields(event).any(|field| pattern.is_match(field)),
            None => fields(event).any(|field| casefold(field).contains(needle.as_ref())),
        })
        .filter(|event| {
            !query.failed_only
                || (event.kind == EventKind::Attempt && event.outcome == Some(Outcome::Failed))
        })
        .collect()
}

#[must_use]
pub fn no_match_message(query: &SearchQuery<'_>) -> &'static str {
    if !query.regex && query.text.contains(REGEX_HINT) {
        "No matches. (Tip: substring match is the default — rerun with `--regex` if you intended an OR/regex pattern.)"
    } else {
        "No matches."
    }
}

#[must_use]
pub fn match_line(event: &Event<'_>, retired: bool) -> String {
    let mut line = format!(
        "{} [{}] {}",
        event.at.as_str(),
        event.id.as_str(),
        event.kind.as_str()
    );
    if let Some(issue) = event
        .issue
        .as_ref()
        .filter(|issue| !issue.as_str().is_empty())
    {
        let _ = write!(line, " #{}", issue.as_str());
    }
    if let Some(outcome) = event.outcome {
        let _ = write!(line, " ({})", outcome.as_str());
    }
    let _ = write!(line, ": {}", event.summary.as_str());
    if retired {
        line.push_str(" (superseded)");
    }
    line
}
