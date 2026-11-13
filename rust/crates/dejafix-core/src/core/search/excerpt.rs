use std::borrow::Cow;
use std::fmt::Write as _;

use super::matcher::search;
use super::types::SearchQuery;
use crate::core::event::Event;
use crate::core::log::EventLog;
use crate::core::summary::{RECORD_CHARS, clip_record};
use crate::core::text::{casefold, char_len, char_prefix, collapse_whitespace};

fn char_find(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let start = haystack
        .char_indices()
        .nth(from)
        .map_or(haystack.len(), |(index, _)| index);
    let found = haystack.get(start..)?.find(needle)?;
    haystack.get(..start + found).map(char_len)
}

fn char_suffix(text: &str, from: usize) -> &str {
    text.char_indices()
        .nth(from)
        .and_then(|(index, _)| text.get(index..))
        .unwrap_or("")
}

#[must_use]
pub fn excerpt(text: &str, query: &str) -> String {
    let flat = collapse_whitespace(text);
    let found = char_find(&casefold(&flat), &casefold(query), 0);
    let fits = |found: usize| found + char_len(query) <= RECORD_CHARS - 3;
    match found {
        Some(found) if char_len(&flat) > RECORD_CHARS && !fits(found) => {
            let from = found.saturating_sub(RECORD_CHARS / 3);
            let start = char_find(&flat, " ", from).map_or(0, |space| space + 1);
            format!("...{}", clip_record(char_suffix(&flat, start)))
        }
        _ => clip_record(&flat).into_owned(),
    }
}

fn match_text(event: &Event<'_>, query: &str, full: bool) -> String {
    let summary = event.summary.as_str();
    let in_summary = casefold(summary).contains(casefold(query).as_ref());
    let notes = event.notes.as_deref().filter(|notes| !notes.is_empty());
    let shown = |source: &str| -> String {
        if full {
            collapse_whitespace(source).into_owned()
        } else {
            excerpt(source, query)
        }
    };
    match notes {
        Some(notes) if !in_summary => {
            format!("{} | notes: {}", clip_record(summary), shown(notes))
        }
        _ => shown(summary),
    }
}

#[must_use]
pub fn mcp_match_line(event: &Event<'_>, query: &str, retired: bool, full: bool) -> String {
    let mut line = format!("  [{}", event.kind.as_str());
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
    let _ = write!(
        line,
        "] {} {}: {}",
        event.id.as_str(),
        char_prefix(event.at.as_str(), 10),
        match_text(event, query, full)
    );
    if let Some(location) = event
        .location
        .as_deref()
        .filter(|location| !location.is_empty())
    {
        let flat: Cow<'_, str> = collapse_whitespace(location);
        let _ = write!(line, " @ {flat}");
    }
    if retired {
        line.push_str(" (superseded)");
    }
    line
}

#[must_use]
pub fn search_reply(log: &EventLog<'_>, query: &str, limit: usize, full: bool) -> String {
    let found = search(
        log,
        &SearchQuery {
            text: query,
            regex: false,
            failed_only: false,
        },
    );
    let shown = found
        .get(found.len().saturating_sub(limit)..)
        .unwrap_or_default();
    if shown.is_empty() {
        return format!("No events match '{query}'.");
    }
    let mut reply = format!("Found {} match(es) for '{query}':", shown.len());
    for event in shown {
        reply.push('\n');
        reply.push_str(&mcp_match_line(
            event,
            query,
            log.is_retired(&event.id),
            full,
        ));
    }
    reply
}
