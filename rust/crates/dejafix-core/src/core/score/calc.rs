use std::borrow::Cow;
use std::collections::HashMap;

use jiff::tz::TimeZone;
use serde_json::{Map, Value};

use super::types::{Score, ScoreError, ScoreEvent};
use crate::core::event::{Event, Outcome};
use crate::core::global::is_truthy;
use crate::core::path::location_to_file;
use crate::core::text::{python_lines, python_strip};

const HOURS_PER_FAILED_APPROACH: f64 = 0.5;
const HOURS_PER_FIX_WITH_CONTEXT: f64 = 0.25;
const HOURS_PER_DECISION: f64 = 0.15;
const HOURS_PER_CHURN_FLAG: f64 = 0.75;
const TOKENS_PER_FAILED_APPROACH: u64 = 3000;
const TOKENS_PER_CONTEXT_REBUILD: u64 = 2000;
const TOKENS_PER_DECISION: u64 = 500;
const USD_PER_MILLION_TOKENS: f64 = 10.0;
const HIGH_CHURN_MENTIONS: usize = 4;
const COMPONENT_CAP: f64 = 20.0;
const COMPONENT_CAP_COUNT: usize = 20;
const SCORE_CAP: u32 = 100;
const GRADES: [(u32, &str); 5] = [(90, "A+"), (80, "A"), (60, "B"), (40, "C"), (20, "D")];

pub fn raw_events(text: &str) -> Result<Vec<Map<String, Value>>, ScoreError> {
    python_lines(text.as_bytes())
        .map(|(line, bytes)| (line, String::from_utf8_lossy(bytes)))
        .filter(|(_, content)| !python_strip(content).is_empty())
        .map(
            |(line, content)| match serde_json::from_str::<Value>(&content) {
                Ok(Value::Object(object)) => Ok(object),
                Ok(_) => Err(ScoreError::Json {
                    line,
                    message: "expected a JSON object".to_owned(),
                }),
                Err(error) => Err(ScoreError::Json {
                    line,
                    message: error.to_string(),
                }),
            },
        )
        .collect()
}

#[must_use]
pub fn score_event(raw: &Map<String, Value>) -> ScoreEvent<'_> {
    let mut files: Vec<Cow<'_, str>> = match raw.get("files") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(Cow::Borrowed)
            .collect(),
        _ => Vec::new(),
    };
    if let Some(Value::String(location)) = raw.get("location")
        && let Some(file) = location_to_file(location)
    {
        files.push(file);
    }
    ScoreEvent {
        kind: raw.get("type").and_then(Value::as_str).unwrap_or_default(),
        outcome: raw.get("outcome").and_then(Value::as_str),
        auto: raw.get("auto_captured").is_some_and(is_truthy),
        files,
        at: None,
    }
}

#[must_use]
pub fn score_event_of<'e>(event: &'e Event<'_>) -> ScoreEvent<'e> {
    let mut files: Vec<Cow<'e, str>> = event
        .files
        .iter()
        .map(|file| Cow::Borrowed(file.as_ref()))
        .collect();
    if let Some(location) = event.location.as_deref()
        && let Some(file) = location_to_file(location)
    {
        files.push(file);
    }
    ScoreEvent {
        kind: event.kind.as_str(),
        outcome: event.outcome.map(Outcome::as_str),
        auto: event.origin.auto_captured,
        files,
        at: event
            .at
            .utc()
            .and_then(|civil| TimeZone::UTC.to_timestamp(civil).ok()),
    }
}

#[must_use]
pub fn calculate_score(events: &[ScoreEvent<'_>]) -> Score {
    let mut failed = 0_usize;
    let mut worked = 0_usize;
    let mut fixes = 0_usize;
    let mut decisions = 0_usize;
    let mut notes = 0_usize;
    let mut auto = 0_usize;
    let mut mentions: HashMap<&str, usize> = HashMap::new();
    for event in events {
        if event.auto {
            auto += 1;
        }
        for file in &event.files {
            *mentions.entry(file.as_ref()).or_default() += 1;
        }
        match (event.kind, event.outcome) {
            ("attempt", Some("failed")) => failed += 1,
            ("attempt", Some("worked")) => worked += 1,
            ("fix", _) => fixes += 1,
            ("decision", _) => decisions += 1,
            ("note", _) => notes += 1,
            _ => {}
        }
    }
    let high_churn_files = mentions
        .values()
        .filter(|count| **count >= HIGH_CHURN_MENTIONS)
        .count();
    let files_with_gotchas = mentions.len();

    let base_hours = round_to(
        count_float(failed) * HOURS_PER_FAILED_APPROACH
            + count_float(fixes) * HOURS_PER_FIX_WITH_CONTEXT
            + count_float(decisions) * HOURS_PER_DECISION,
        1,
    );
    let hours = base_hours + round_to(count_float(high_churn_files) * HOURS_PER_CHURN_FLAG, 1);
    let tokens = wide(failed) * TOKENS_PER_FAILED_APPROACH
        + wide(fixes) * TOKENS_PER_CONTEXT_REBUILD
        + wide(decisions) * TOKENS_PER_DECISION
        + wide(auto) * TOKENS_PER_CONTEXT_REBUILD;
    let usd = round_to(
        (wide_float(tokens) / 1_000_000.0) * USD_PER_MILLION_TOKENS,
        2,
    );

    let failed_score = count_float((failed * 2).min(COMPONENT_CAP_COUNT));
    let decision_score = (count_float(decisions) * 1.5).min(COMPONENT_CAP);
    let fix_score = count_float((fixes * 2).min(COMPONENT_CAP_COUNT));
    let gotcha_score = (count_float(files_with_gotchas) * 0.5).min(COMPONENT_CAP);
    let coverage_score = (count_float(notes) * 0.5 + count_float(auto) * 0.3 + count_float(worked))
        .min(COMPONENT_CAP);
    let sum = failed_score + decision_score + fix_score + gotcha_score + coverage_score;
    let score = (0..=SCORE_CAP)
        .rev()
        .find(|candidate| f64::from(*candidate) <= sum)
        .unwrap_or(0);

    Score {
        score,
        grade: grade(score),
        failed,
        worked,
        decisions,
        fixes,
        notes,
        files_with_gotchas,
        high_churn_files,
        auto,
        manual: events.len() - auto,
        total: events.len(),
        hours,
        tokens,
        usd,
    }
}

fn grade(score: u32) -> &'static str {
    GRADES
        .iter()
        .find(|(floor, _)| score >= *floor)
        .map_or("F", |(_, grade)| grade)
}

fn round_to(value: f64, digits: usize) -> f64 {
    format!("{value:.digits$}").parse().unwrap_or(value)
}

fn wide(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn count_float(count: usize) -> f64 {
    wide_float(wide(count))
}

fn wide_float(value: u64) -> f64 {
    let high = u32::try_from(value >> 32).unwrap_or(u32::MAX);
    let low = u32::try_from(value & u64::from(u32::MAX)).unwrap_or(0);
    f64::from(high) * 4_294_967_296.0 + f64::from(low)
}
