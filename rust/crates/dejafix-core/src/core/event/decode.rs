use std::borrow::Cow;
use std::fmt;

use jiff::tz::TimeZone;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::error::Category;
use serde_json::value::RawValue;

use super::types::{
    CaptureSource, Confidence, CowStr, CowStrVisitor, Event, EventId, EventKind, Extra, IssueId,
    LineContext, LineError, Object, ObjectVisitor, Origin, Outcome, Summary,
};
use crate::core::json::{is_dropped_by_to_dict, is_python_truthy};
use crate::core::time::{Clock, Timestamp, normalize_timestamp};

impl<'c> LineContext<'c> {
    pub(crate) fn new(clock: &'c Clock) -> Self {
        LineContext {
            zone: &clock.zone,
            now: Timestamp::now(clock),
        }
    }
}

pub(crate) fn decode_line<'a>(
    line: &'a str,
    context: &LineContext<'_>,
) -> Result<Event<'a>, LineError> {
    let Object(entries) = serde_json::from_str(line).map_err(|error| match error.classify() {
        Category::Data => LineError::NotAnObject,
        Category::Io | Category::Syntax | Category::Eof => LineError::Json,
    })?;
    let mut id = None;
    let mut at = None;
    let mut kind = None;
    let mut summary = None;
    let mut issue = None;
    let mut outcome = None;
    let mut files = Vec::new();
    let mut location = None;
    let mut command = None;
    let mut notes = None;
    let mut supersedes = None;
    let mut origin = Origin::default();
    let mut extra = Vec::new();
    for (key, raw) in &entries {
        let mut keep = || extra.push((key.clone(), *raw));
        match key.as_ref() {
            "type" => {
                kind = choice(raw, EventKind::parse, LineError::Kind)?;
            }
            "summary" => {
                summary = Some(text(raw).and_then(Summary::new).ok_or(LineError::Summary)?);
            }
            "id" => {
                if is_python_truthy(raw) {
                    id = Some(EventId(text(raw).ok_or(LineError::Unsupported("id"))?));
                }
            }
            "timestamp" => at = timestamp(raw, context.zone)?,
            "outcome" => outcome = choice(raw, Outcome::parse, LineError::Outcome)?,
            "capture_source" => {
                origin.source = choice(raw, CaptureSource::parse, LineError::CaptureSource)?;
            }
            "capture_confidence" => {
                origin.confidence = choice(raw, Confidence::parse, LineError::Confidence)?;
            }
            "files" => files = file_list(raw)?,
            "auto_captured" => origin.auto_captured = is_python_truthy(raw),
            "issue_id" => issue = pass_through(raw, &mut keep).map(IssueId),
            "command" => command = pass_through(raw, &mut keep),
            "notes" => notes = pass_through(raw, &mut keep),
            "git_commit" => origin.git_commit = pass_through(raw, &mut keep),
            "location" => location = pass_through(raw, &mut keep),
            "git_message" => origin.git_message = pass_through(raw, &mut keep),
            "supersedes" => supersedes = pass_through(raw, &mut keep).map(EventId),
            _ => keep(),
        }
    }
    for (_, raw) in &extra {
        serde_json::from_str::<serde_json::Value>(raw.get()).map_err(|_| LineError::Json)?;
    }
    let id = match id {
        Some(id) => id,
        None => EventId::stable(&entries).map_err(|_| LineError::Json)?,
    };
    extra.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(Event {
        id,
        at: at.unwrap_or_else(|| context.now.clone()),
        kind: kind.ok_or(LineError::Kind)?,
        summary: summary.ok_or(LineError::Summary)?,
        issue,
        outcome,
        files,
        location,
        command,
        notes,
        supersedes,
        origin,
        extra: Extra(extra),
    })
}

fn text<'a>(raw: &'a RawValue) -> Option<Cow<'a, str>> {
    raw.get()
        .starts_with('"')
        .then(|| serde_json::from_str::<CowStr<'a>>(raw.get()).ok())
        .flatten()
        .map(|found| found.0)
}

fn choice<T>(
    raw: &RawValue,
    parse: impl Fn(&str) -> Option<T>,
    error: LineError,
) -> Result<Option<T>, LineError> {
    if raw.get() == "null" {
        return Ok(None);
    }
    text(raw)
        .and_then(|value| parse(&value))
        .map(Some)
        .ok_or(error)
}

fn timestamp<'a>(raw: &'a RawValue, zone: &TimeZone) -> Result<Option<Timestamp<'a>>, LineError> {
    if !is_python_truthy(raw) {
        return Ok(None);
    }
    let value = text(raw).ok_or(LineError::Timestamp)?;
    normalize_timestamp(value, zone)
        .map(Some)
        .map_err(|_| LineError::Timestamp)
}

fn file_list<'a>(raw: &'a RawValue) -> Result<Vec<Cow<'a, str>>, LineError> {
    if !is_python_truthy(raw) {
        return Ok(Vec::new());
    }
    if !raw.get().starts_with('[') {
        return Err(LineError::Unsupported("files"));
    }
    serde_json::from_str::<Vec<CowStr<'a>>>(raw.get())
        .map(|files| files.into_iter().map(|file| file.0).collect())
        .map_err(|_| LineError::Unsupported("files"))
}

fn pass_through<'a>(raw: &'a RawValue, keep: &mut impl FnMut()) -> Option<Cow<'a, str>> {
    if is_dropped_by_to_dict(raw) {
        return None;
    }
    let found = text(raw);
    if found.is_none() {
        keep();
    }
    found
}

impl<'de> Deserialize<'de> for Object<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor)
    }
}

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Object<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut entries: Vec<(Cow<'de, str>, &'de RawValue)> = Vec::new();
        while let Some(CowStr(key)) = map.next_key::<CowStr<'de>>()? {
            entries.push((key, map.next_value()?));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries.reverse();
        entries.dedup_by(|later, kept| later.0 == kept.0);
        Ok(Object(entries))
    }
}

impl<'de> Deserialize<'de> for CowStr<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(CowStrVisitor)
    }
}

impl<'de> Visitor<'de> for CowStrVisitor {
    type Value = CowStr<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_borrowed_str<E: serde::de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(CowStr(Cow::Borrowed(value)))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(CowStr(Cow::Owned(value.to_owned())))
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(CowStr(Cow::Owned(value)))
    }
}
