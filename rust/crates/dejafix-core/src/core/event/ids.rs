use std::borrow::Cow;

use serde_json::Value;
use serde_json::value::RawValue;
use sha1::{Digest, Sha1};

use super::types::{EventId, IssueId, Summary};
use crate::core::json::write_canonical;
use crate::core::text::{is_python_decimal, python_strip, zfill};

impl IssueId<'_> {
    #[must_use]
    pub fn normalize(input: &str) -> Option<IssueId<'static>> {
        let cleaned = python_strip(input).trim_start_matches('#');
        if cleaned.is_empty() {
            return None;
        }
        let normalized = if is_python_decimal(cleaned) {
            zfill(cleaned, 4)
        } else {
            Cow::Borrowed(cleaned)
        };
        Some(IssueId(Cow::Owned(normalized.into_owned())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'a> From<&'a str> for IssueId<'a> {
    fn from(text: &'a str) -> Self {
        IssueId(Cow::Borrowed(text))
    }
}

impl EventId<'_> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn stable(
        entries: &[(Cow<'_, str>, &RawValue)],
    ) -> Result<EventId<'static>, serde_json::Error> {
        let object = entries
            .iter()
            .map(|(key, raw)| {
                Ok((
                    key.as_ref().to_owned(),
                    serde_json::from_str::<Value>(raw.get())?,
                ))
            })
            .collect::<Result<serde_json::Map<_, _>, serde_json::Error>>()?;
        let mut canonical = Vec::new();
        write_canonical(&mut canonical, &Value::Object(object))?;
        let digest = Sha1::digest(&canonical);
        let mut id = String::with_capacity(24);
        id.push_str("evt_");
        for byte in digest.iter().take(10) {
            id.extend(
                [byte >> 4, byte & 0x0f]
                    .map(|nibble| char::from_digit(u32::from(nibble), 16).unwrap_or('0')),
            );
        }
        Ok(EventId(Cow::Owned(id)))
    }
}

impl<'a> From<&'a str> for EventId<'a> {
    fn from(text: &'a str) -> Self {
        EventId(Cow::Borrowed(text))
    }
}

impl<'a> Summary<'a> {
    #[must_use]
    pub fn new(text: Cow<'a, str>) -> Option<Self> {
        let stripped = python_strip(&text);
        if stripped.is_empty() {
            return None;
        }
        if stripped.len() == text.len() {
            return Some(Summary(text));
        }
        Some(Summary(match text {
            Cow::Borrowed(text) => Cow::Borrowed(python_strip(text)),
            Cow::Owned(ref owned) => Cow::Owned(python_strip(owned).to_owned()),
        }))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for IssueId<'_> {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for EventId<'_> {
    fn borrow(&self) -> &str {
        &self.0
    }
}
