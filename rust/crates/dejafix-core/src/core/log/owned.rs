use super::types::EventLog;
use crate::core::event::{Event, EventId, IssueId};
use crate::core::path::RelativePath;

impl EventLog<'_> {
    #[must_use]
    pub fn into_owned(self) -> EventLog<'static> {
        EventLog {
            events: self.events.into_iter().map(Event::into_owned).collect(),
            skipped: self.skipped,
            closed: self.closed.into_iter().map(IssueId::into_owned).collect(),
            open: self.open,
            retired: self.retired.into_iter().map(EventId::into_owned).collect(),
            files: self
                .files
                .into_iter()
                .map(|(path, indices)| (RelativePath::into_owned(path), indices))
                .collect(),
        }
    }
}
