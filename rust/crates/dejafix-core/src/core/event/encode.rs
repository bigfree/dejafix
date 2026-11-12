use std::io::Write;

use serde::ser::{Error as _, Serialize, SerializeMap, Serializer};
use serde_json::Value as Json;

use super::types::{CaptureSource, Confidence, Event, EventId, IssueId, Outcome, Value};
use crate::core::json::{Sorted, write_python_json};

impl Event<'_> {
    pub fn write_line<W: Write>(&self, out: &mut W) -> Result<(), serde_json::Error> {
        write_python_json(out, self)?;
        out.write_all(b"\n").map_err(serde_json::Error::io)
    }

    fn entries(&self) -> Vec<(&str, Value<'_, '_>)> {
        let mut entries: Vec<(&str, Value<'_, '_>)> = [
            Some(("id", Value::Str(self.id.as_str()))),
            Some(("timestamp", Value::Str(self.at.as_str()))),
            Some(("type", Value::Str(self.kind.as_str()))),
            Some(("summary", Value::Str(self.summary.as_str()))),
            optional("issue_id", self.issue.as_ref().map(IssueId::as_str)),
            optional("outcome", self.outcome.map(Outcome::as_str)),
            (!self.files.is_empty()).then_some(("files", Value::Strs(&self.files))),
            optional("command", self.command.as_deref()),
            optional("notes", self.notes.as_deref()),
            optional("git_commit", self.origin.git_commit.as_deref()),
            optional("location", self.location.as_deref()),
            self.origin
                .auto_captured
                .then_some(("auto_captured", Value::Bool)),
            optional(
                "capture_source",
                self.origin.source.map(CaptureSource::as_str),
            ),
            optional(
                "capture_confidence",
                self.origin.confidence.map(Confidence::as_str),
            ),
            optional("git_message", self.origin.git_message.as_deref()),
            optional("supersedes", self.supersedes.as_ref().map(EventId::as_str)),
        ]
        .into_iter()
        .flatten()
        .chain(
            self.extra
                .0
                .iter()
                .map(|(key, raw)| (key.as_ref(), Value::Raw(raw))),
        )
        .collect();
        entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
        entries
    }
}

impl Serialize for Event<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entries = self.entries();
        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for (key, value) in &entries {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Serialize for Value<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Str(text) => serializer.serialize_str(text),
            Value::Strs(items) => serializer.collect_seq(items.iter()),
            Value::Bool => serializer.serialize_bool(true),
            Value::Raw(raw) => {
                let parsed: Json = serde_json::from_str(raw.get()).map_err(S::Error::custom)?;
                Sorted(&parsed).serialize(serializer)
            }
        }
    }
}

fn optional<'v, 'a>(key: &'v str, value: Option<&'v str>) -> Option<(&'v str, Value<'v, 'a>)> {
    value.map(|value| (key, Value::Str(value)))
}
