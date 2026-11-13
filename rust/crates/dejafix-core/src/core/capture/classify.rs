use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use super::rank::{capture_files, pick_location};
use super::types::{Change, Classified, Rule};
use crate::core::event::{
    CaptureSource, Confidence, Event, EventKind, Extra, Origin, Outcome, Summary,
};
use crate::core::text::{PYTHON_SPACE_CLASS, char_prefix, python_strip};
use crate::core::time::{Clock, Timestamp};
use crate::core::write::Ids;

const FIRST_LINE_CHARS: usize = 120;
const COMMAND: &str = "auto-capture";

const fn classified(
    kind: EventKind,
    outcome: Option<Outcome>,
    prefix: &'static str,
    confidence: Confidence,
) -> Classified {
    Classified {
        kind,
        outcome,
        prefix,
        confidence,
    }
}

const SOURCES: [(&str, Classified); 7] = [
    (
        r"(?i)^revert\b|^Revert\b",
        classified(
            EventKind::Attempt,
            Some(Outcome::Failed),
            "Reverted",
            Confidence::High,
        ),
    ),
    (
        r"(?i)^fix[{SPACE}(:]|^hotfix[{SPACE}(:]|^bugfix[{SPACE}(:]|^patch[{SPACE}(:]",
        classified(EventKind::Fix, None, "Fix", Confidence::High),
    ),
    (
        r"(?i)BREAKING[{SPACE}_-]?CHANGE|^break[{SPACE}(:]",
        classified(
            EventKind::Decision,
            None,
            "Breaking change",
            Confidence::High,
        ),
    ),
    (
        r"(?i)^feat[{SPACE}(:]|^feature[{SPACE}(:]|^add[{SPACE}(:]",
        classified(EventKind::Note, None, "New feature", Confidence::Medium),
    ),
    (
        r"(?i)^refactor[{SPACE}(:]|^cleanup[{SPACE}(:]|^reorganize|^restructure",
        classified(EventKind::Decision, None, "Refactor", Confidence::Medium),
    ),
    (
        r"(?i)^docs?[{SPACE}(:]|^readme|^changelog",
        classified(EventKind::Note, None, "Documentation", Confidence::Low),
    ),
    (
        r"(?i)^test[{SPACE}(:]|^tests?[{SPACE}(:]|^spec[{SPACE}(:]",
        classified(EventKind::Note, None, "Tests", Confidence::Low),
    ),
];

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .filter_map(|(source, classified)| {
            Regex::new(&source.replace("{SPACE}", PYTHON_SPACE_CLASS))
                .ok()
                .map(|regex| Rule {
                    regex,
                    classified: *classified,
                })
        })
        .collect()
});

#[must_use]
pub fn classify(message: &str) -> Option<Classified> {
    RULES
        .iter()
        .find(|rule| rule.regex.is_match(message))
        .map(|rule| rule.classified)
}

const fn rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
    }
}

#[must_use]
pub const fn meets_threshold(confidence: Confidence) -> bool {
    rank(confidence) >= rank(Confidence::Medium)
}

#[must_use]
pub fn first_line(message: &str) -> String {
    python_strip(message)
        .split('\n')
        .next()
        .map_or("", |line| char_prefix(line, FIRST_LINE_CHARS))
        .to_owned()
}

#[must_use]
pub fn capture_event(
    message: &str,
    changes: &[Change],
    commit: Option<String>,
    is_memory: impl Fn(&str) -> bool,
    clock: &Clock,
    ids: &mut impl Ids,
) -> Option<Event<'static>> {
    let matched = classify(message).filter(|matched| meets_threshold(matched.confidence))?;
    let line = first_line(message);
    let summary = Summary::new(Cow::Owned(format!("{}: {line}", matched.prefix)))?;
    let location = pick_location(changes, &is_memory);
    let files = capture_files(changes, location, &is_memory);
    Some(Event {
        id: ids.event_id(),
        at: Timestamp::now(clock),
        kind: matched.kind,
        summary,
        issue: None,
        outcome: matched.outcome,
        files: files.into_iter().map(Cow::Owned).collect(),
        location: location.map(|location| Cow::Owned(location.to_owned())),
        command: Some(Cow::Borrowed(COMMAND)),
        notes: None,
        supersedes: None,
        origin: Origin {
            auto_captured: true,
            source: Some(CaptureSource::GitPostCommit),
            confidence: Some(matched.confidence),
            git_commit: commit.map(Cow::Owned),
            git_message: Some(Cow::Owned(line)),
        },
        extra: Extra::default(),
    })
}

#[must_use]
pub fn merge_event(
    message: &str,
    commit: Option<String>,
    clock: &Clock,
    ids: &mut impl Ids,
) -> Option<Event<'static>> {
    let line = first_line(message);
    let summary = Summary::new(Cow::Owned(format!("Merge: {line}")))?;
    Some(Event {
        id: ids.event_id(),
        at: Timestamp::now(clock),
        kind: EventKind::Note,
        summary,
        issue: None,
        outcome: None,
        files: Vec::new(),
        location: None,
        command: Some(Cow::Borrowed(COMMAND)),
        notes: None,
        supersedes: None,
        origin: Origin {
            auto_captured: true,
            source: Some(CaptureSource::GitPostMerge),
            confidence: Some(Confidence::High),
            git_commit: commit.map(Cow::Owned),
            git_message: Some(Cow::Owned(line)),
        },
        extra: Extra::default(),
    })
}
