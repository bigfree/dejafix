use std::borrow::Cow;

use super::types::{Event, EventId, Extra, IssueId, Origin, Summary};

fn owned(text: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(text.into_owned())
}

impl EventId<'_> {
    #[must_use]
    pub fn into_owned(self) -> EventId<'static> {
        EventId(owned(self.0))
    }
}

impl IssueId<'_> {
    #[must_use]
    pub fn into_owned(self) -> IssueId<'static> {
        IssueId(owned(self.0))
    }
}

impl Summary<'_> {
    #[must_use]
    pub fn into_owned(self) -> Summary<'static> {
        Summary(owned(self.0))
    }
}

impl Origin<'_> {
    #[must_use]
    pub fn into_owned(self) -> Origin<'static> {
        Origin {
            auto_captured: self.auto_captured,
            source: self.source,
            confidence: self.confidence,
            git_commit: self.git_commit.map(owned),
            git_message: self.git_message.map(owned),
        }
    }
}

impl Extra<'_> {
    #[must_use]
    pub fn into_owned(self) -> Extra<'static> {
        Extra(
            self.0
                .into_iter()
                .map(|(key, raw)| (owned(key), Cow::Owned(raw.into_owned())))
                .collect(),
        )
    }
}

impl Event<'_> {
    #[must_use]
    pub fn into_owned(self) -> Event<'static> {
        Event {
            id: self.id.into_owned(),
            at: self.at.into_owned(),
            kind: self.kind,
            summary: self.summary.into_owned(),
            issue: self.issue.map(IssueId::into_owned),
            outcome: self.outcome,
            files: self.files.into_iter().map(owned).collect(),
            location: self.location.map(owned),
            command: self.command.map(owned),
            notes: self.notes.map(owned),
            supersedes: self.supersedes.map(EventId::into_owned),
            origin: self.origin.into_owned(),
            extra: self.extra.into_owned(),
        }
    }
}
