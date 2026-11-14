use super::types::{EventDoc, SearchQuery};
use crate::core::event::{Event, EventKind, Outcome};
use crate::core::log::EventLog;
use crate::core::rank::{Document, FieldSpec, Index, RankParams};

pub const EVENT_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "summary",
        weight: 2.0,
    },
    FieldSpec {
        name: "location",
        weight: 2.0,
    },
    FieldSpec {
        name: "notes",
        weight: 1.0,
    },
];

impl Document for EventDoc<'_, '_> {
    fn id(&self) -> &str {
        self.0.id.as_str()
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        match field {
            0 => visit(self.0.summary.as_str()),
            1 => {
                if let Some(location) = self.0.location.as_deref() {
                    visit(location);
                }
                for file in &self.0.files {
                    visit(file);
                }
            }
            _ => {
                if let Some(notes) = self.0.notes.as_deref() {
                    visit(notes);
                }
            }
        }
    }
}

pub(super) fn is_failed_attempt(event: &Event<'_>) -> bool {
    event.kind == EventKind::Attempt && event.outcome == Some(Outcome::Failed)
}

#[must_use]
pub fn event_index(log: &EventLog<'_>) -> Index {
    let docs: Vec<EventDoc<'_, '_>> = log.events().iter().map(EventDoc).collect();
    Index::build(EVENT_FIELDS, &docs)
}

#[must_use]
pub fn ranked<'l, 'a>(
    log: &'l EventLog<'a>,
    index: &Index,
    query: &SearchQuery<'_>,
) -> Vec<&'l Event<'a>> {
    let events = log.events();
    let event = |doc: u32| usize::try_from(doc).ok().and_then(|doc| events.get(doc));
    index
        .rank_where(
            &index.query_terms(query.text),
            &RankParams::DEFAULT,
            |doc| event(doc).is_some_and(|event| !query.failed_only || is_failed_attempt(event)),
        )
        .iter()
        .filter_map(|scored| event(scored.doc))
        .collect()
}
