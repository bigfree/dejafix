use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::core::log::EventLog;
use crate::core::path::location_to_file;
use crate::core::text::{char_len, char_prefix, collapse_whitespace, is_python_word};

pub const RECORD_CHARS: usize = 300;
const MEMORY_DIR_PREFIX: &str = ".dejafix/";

static MENTION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_./-]+\.[A-Za-z0-9]+(?::\p{Nd}+)?").ok());

#[must_use]
pub fn clip_record(text: &str) -> Cow<'_, str> {
    let text = collapse_whitespace(text);
    if char_len(&text) <= RECORD_CHARS {
        return text;
    }
    let head = char_prefix(&text, RECORD_CHARS - 3);
    let floor = head
        .char_indices()
        .nth(RECORD_CHARS / 2 + 1)
        .map_or(head.len(), |(index, _)| index);
    let head = head
        .get(floor..)
        .and_then(|tail| tail.rfind(' '))
        .and_then(|boundary| head.get(..floor + boundary))
        .unwrap_or(head);
    let mut clipped = head.trim_end_matches([' ', ',', ';', ':']).to_owned();
    clipped.push_str("...");
    Cow::Owned(clipped)
}

fn blocks_mention(character: char) -> bool {
    is_python_word(character) || matches!(character, '/' | '.' | '-')
}

pub(super) fn infer_file_mentions(text: &str) -> Vec<&str> {
    let Some(pattern) = MENTION.as_ref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(candidate) = pattern.find_at(text, at) {
        let blocked = text
            .get(..candidate.start())
            .and_then(|head| head.chars().next_back())
            .is_some_and(blocks_mention);
        if blocked {
            at = candidate.start()
                + text
                    .get(candidate.start()..)
                    .and_then(|tail| tail.chars().next())
                    .map_or(1, char::len_utf8);
            continue;
        }
        found.push(candidate.as_str());
        at = candidate.end().max(candidate.start() + 1);
    }
    found
}

#[must_use]
pub fn key_files(log: &EventLog<'_>) -> Vec<(String, usize)> {
    let mut order: Vec<(String, usize)> = Vec::new();
    let mut position: HashMap<String, usize> = HashMap::new();
    let mut seen: Vec<String> = Vec::new();
    for event in log.events() {
        seen.clear();
        let mentions = event
            .files
            .iter()
            .map(Cow::as_ref)
            .chain(
                event
                    .location
                    .as_deref()
                    .filter(|location| !location.is_empty()),
            )
            .chain(infer_file_mentions(event.summary.as_str()));
        for mention in mentions {
            let collapsed = collapse_whitespace(mention);
            let Some(path) = location_to_file(&collapsed) else {
                continue;
            };
            if path.is_empty()
                || path.starts_with(MEMORY_DIR_PREFIX)
                || seen.iter().any(|known| known == path.as_ref())
            {
                continue;
            }
            seen.push(path.clone().into_owned());
        }
        for path in &seen {
            if let Some(index) = position.get(path) {
                if let Some(entry) = order.get_mut(*index) {
                    entry.1 += 1;
                }
            } else {
                position.insert(path.clone(), order.len());
                order.push((path.clone(), 1));
            }
        }
    }
    order.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    order
}
