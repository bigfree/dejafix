use std::borrow::Cow;

use super::issue::{issue_exists, latest_open_within, marker_state, next_issue_id};
use super::types::{
    Command, Ids, MarkerChange, MarkerState, ValidationError, WriteContext, WriteOutcome, WritePlan,
};
use crate::core::event::{Event, EventId, EventKind, Extra, IssueId, Origin, Outcome, Summary};
use crate::core::log::EventLog;
use crate::core::redact::{SecretKind, redact, redaction_notice};
use crate::core::text::{char_prefix, python_strip};
use crate::core::time::Timestamp;

struct Draft<'d> {
    kind: EventKind,
    text: &'d str,
    location: Option<&'d str>,
    issue: Option<IssueId<'static>>,
    outcome: Option<Outcome>,
    supersedes: Option<EventId<'static>>,
}

struct Planner<'p, 'c, I> {
    context: &'p WriteContext<'c>,
    ids: &'p mut I,
    events: Vec<Event<'static>>,
    notices: Vec<String>,
}

#[must_use]
pub fn checks_marker(command: &Command) -> bool {
    match command {
        Command::Fix { .. } => true,
        Command::Attempt { issue, .. } => issue.as_deref().is_none_or(str::is_empty),
        Command::Log { .. } | Command::Decision { .. } | Command::Note { .. } => false,
    }
}

pub fn plan_command(
    log: &EventLog<'_>,
    command: Command,
    context: &WriteContext<'_>,
    ids: &mut impl Ids,
) -> Result<WritePlan, ValidationError> {
    let mut planner = Planner {
        context,
        ids,
        events: Vec::new(),
        notices: Vec::new(),
    };
    let (marker, outcome) = match command {
        Command::Log { text, location } => {
            let issue = next_issue_id(log);
            planner.add(&Draft {
                kind: EventKind::Issue,
                text: &text,
                location: location.as_deref(),
                issue: Some(issue.clone()),
                outcome: None,
                supersedes: None,
            })?;
            (
                MarkerChange::Set(issue.clone()),
                WriteOutcome::Logged(issue),
            )
        }
        Command::Attempt {
            text,
            outcomes,
            location,
            issue,
            auto_issue,
        } => planner.attempt(
            log,
            &text,
            &outcomes,
            location.as_deref(),
            issue.as_deref(),
            auto_issue,
        )?,
        Command::Fix {
            text,
            location,
            issue,
        } => planner.fix(log, &text, location.as_deref(), issue.as_deref())?,
        Command::Decision {
            text,
            location,
            supersedes,
        } => {
            let superseded = match supersedes.as_deref() {
                Some(reference) if !reference.is_empty() => {
                    let found = log.resolve_event_ref(reference)?;
                    Some((
                        EventId::from_string(found.id.as_str().to_owned()),
                        char_prefix(found.summary.as_str(), 60).to_owned(),
                    ))
                }
                _ => None,
            };
            planner.add(&Draft {
                kind: EventKind::Decision,
                text: &text,
                location: location.as_deref(),
                issue: None,
                outcome: None,
                supersedes: superseded.as_ref().map(|(id, _)| id.clone()),
            })?;
            (MarkerChange::Keep, WriteOutcome::Decided { superseded })
        }
        Command::Note { text, location } => {
            planner.add(&Draft {
                kind: EventKind::Note,
                text: &text,
                location: location.as_deref(),
                issue: None,
                outcome: None,
                supersedes: None,
            })?;
            (MarkerChange::Keep, WriteOutcome::Noted)
        }
    };
    Ok(WritePlan {
        events: planner.events,
        notices: planner.notices,
        marker,
        outcome,
    })
}

fn valid_marker(log: &EventLog<'_>, context: &WriteContext<'_>) -> Option<IssueId<'static>> {
    match marker_state(log, context.marker) {
        MarkerState::Valid(marker) => Some(IssueId::from_string(marker)),
        MarkerState::Absent | MarkerState::Stale => None,
    }
}

impl<I: Ids> Planner<'_, '_, I> {
    fn attempt(
        &mut self,
        log: &EventLog<'_>,
        text: &str,
        outcomes: &[Outcome],
        location: Option<&str>,
        issue: Option<&str>,
        auto_issue: bool,
    ) -> Result<(MarkerChange, WriteOutcome), ValidationError> {
        if outcomes.len() > 1 {
            return Err(ValidationError::TwoOutcomes);
        }
        let mut marker = MarkerChange::Keep;
        let target = if let Some(requested) = issue.filter(|requested| !requested.is_empty()) {
            let normalized = IssueId::normalize(requested);
            match normalized {
                Some(found) if issue_exists(log, found.as_str()) => Some(found),
                _ => {
                    let shown = normalized.map_or_else(
                        || {
                            let bare = python_strip(requested).trim_start_matches('#');
                            if bare.is_empty() {
                                "?".to_owned()
                            } else {
                                bare.to_owned()
                            }
                        },
                        |id| id.as_str().to_owned(),
                    );
                    return Err(ValidationError::UnknownIssue(shown));
                }
            }
        } else {
            valid_marker(log, self.context).or_else(|| latest_open_within(log, self.context.clock))
        };
        let target = if let Some(target) = target {
            target
        } else {
            if !auto_issue {
                return Err(ValidationError::NoActiveIssue);
            }
            let created = next_issue_id(log);
            self.add(&Draft {
                kind: EventKind::Issue,
                text,
                location,
                issue: Some(created.clone()),
                outcome: None,
                supersedes: None,
            })?;
            marker = MarkerChange::Set(created.clone());
            created
        };
        let outcome = outcomes.first().copied().unwrap_or(Outcome::Partial);
        self.add(&Draft {
            kind: EventKind::Attempt,
            text,
            location,
            issue: Some(target.clone()),
            outcome: Some(outcome),
            supersedes: None,
        })?;
        Ok((
            marker,
            WriteOutcome::Attempted {
                outcome,
                issue: target,
            },
        ))
    }

    fn fix(
        &mut self,
        log: &EventLog<'_>,
        text: &str,
        location: Option<&str>,
        issue: Option<&str>,
    ) -> Result<(MarkerChange, WriteOutcome), ValidationError> {
        let requested = issue.and_then(IssueId::normalize);
        let active = valid_marker(log, self.context).or_else(|| {
            log.current_issue()
                .map(|issue| IssueId::from_string(issue.as_str().to_owned()))
        });
        let target = match &requested {
            Some(requested) => {
                if !issue_exists(log, requested.as_str()) {
                    return Err(ValidationError::UnknownIssue(requested.as_str().to_owned()));
                }
                requested.clone()
            }
            None => active.clone().ok_or(ValidationError::NoOpenIssue)?,
        };
        self.add(&Draft {
            kind: EventKind::Fix,
            text,
            location,
            issue: Some(target.clone()),
            outcome: None,
            supersedes: None,
        })?;
        let marker = if requested.is_none() || active.as_ref() == Some(&target) {
            MarkerChange::Clear
        } else {
            MarkerChange::Keep
        };
        Ok((marker, WriteOutcome::Fixed(target)))
    }

    fn add(&mut self, draft: &Draft<'_>) -> Result<(), ValidationError> {
        let summary =
            Summary::new(Cow::Owned(draft.text.to_owned())).ok_or(ValidationError::EmptySummary)?;
        let mut fired: Vec<SecretKind> = Vec::new();
        let summary = if self.context.redact {
            let redacted = redact(summary.as_str());
            if redacted.fired.is_empty() {
                summary
            } else {
                fired.extend(redacted.fired);
                Summary::new(Cow::Owned(redacted.text.into_owned())).unwrap_or(summary)
            }
        } else {
            summary
        };
        let location = draft.location.map(|location| {
            if self.context.redact && !location.is_empty() {
                let redacted = redact(location);
                fired.extend(redacted.fired);
                Cow::Owned(redacted.text.into_owned())
            } else {
                Cow::Owned(location.to_owned())
            }
        });
        if let Some(notice) = redaction_notice(&fired) {
            self.notices.push(notice);
        }
        self.events.push(Event {
            id: self.ids.event_id(),
            at: Timestamp::now(self.context.clock),
            kind: draft.kind,
            summary,
            issue: draft.issue.clone(),
            outcome: draft.outcome,
            files: Vec::new(),
            location,
            command: None,
            notes: None,
            supersedes: draft.supersedes.clone(),
            origin: Origin {
                git_commit: self
                    .context
                    .git_commit
                    .map(|commit| Cow::Owned(commit.to_owned())),
                ..Origin::default()
            },
            extra: Extra::default(),
        });
        Ok(())
    }
}
