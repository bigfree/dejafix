use super::types::{EventLog, EventRefError, Skipped};
use crate::core::event::{Event, EventId, EventKind, IssueId};
use crate::core::path::RelativePath;
use crate::core::text::python_strip;

impl<'a> EventLog<'a> {
    #[must_use]
    pub fn events(&self) -> &[Event<'a>] {
        &self.events
    }

    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    pub fn open_issues(&self) -> impl Iterator<Item = &IssueId<'a>> {
        self.open
            .iter()
            .filter_map(|index| self.events.get(*index)?.issue.as_ref())
    }

    #[must_use]
    pub fn current_issue(&self) -> Option<&IssueId<'a>> {
        let newest = self.events.iter().rev().find(|event| {
            event.kind == EventKind::Issue
                && event
                    .issue
                    .as_ref()
                    .is_none_or(|issue| !self.closed.contains(issue))
        })?;
        newest.issue.as_ref()
    }

    #[must_use]
    pub fn is_closed(&self, issue: &IssueId<'_>) -> bool {
        self.closed.contains(issue.as_str())
    }

    #[must_use]
    pub fn is_retired(&self, id: &EventId<'_>) -> bool {
        self.retired.contains(id.as_str())
    }

    pub fn live_events_for_file(&self, path: &str) -> impl Iterator<Item = &Event<'a>> {
        self.files
            .get(path)
            .into_iter()
            .flatten()
            .filter_map(|index| self.events.get(*index))
    }

    pub fn indexed_files(&self) -> impl Iterator<Item = &RelativePath<'a>> {
        self.files.keys()
    }

    pub fn resolve_event_ref(&self, reference: &str) -> Result<&Event<'a>, EventRefError> {
        let needle = python_strip(reference);
        if needle.is_empty() {
            return Err(EventRefError::Empty);
        }
        if let Some(exact) = self.events.iter().find(|event| event.id.as_str() == needle) {
            let count = self
                .events
                .iter()
                .filter(|event| event.id.as_str() == needle)
                .count();
            return if count == 1 {
                Ok(exact)
            } else {
                Err(EventRefError::Ambiguous {
                    reference: reference.to_owned(),
                    count,
                })
            };
        }
        let bare = needle.strip_prefix("evt_").unwrap_or(needle);
        let mut matches = self.events.iter().filter(|event| {
            let id = event.id.as_str();
            id.strip_prefix("evt_").unwrap_or(id).starts_with(bare)
        });
        match (matches.next(), matches.count()) {
            (None, _) => Err(EventRefError::NotFound(reference.to_owned())),
            (Some(found), 0) => Ok(found),
            (Some(_), rest) => Err(EventRefError::Ambiguous {
                reference: reference.to_owned(),
                count: rest + 1,
            }),
        }
    }
}
