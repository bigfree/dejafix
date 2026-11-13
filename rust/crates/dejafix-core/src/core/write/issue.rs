use jiff::ToSpan;
use jiff::tz::TimeZone;

use super::types::{MarkerState, ValidationError};
use crate::core::event::{EventKind, IssueId};
use crate::core::log::EventLog;
use crate::core::text::{DecimalKey, decimal_value};
use crate::core::time::Clock;

const OPEN_ISSUES_LISTED: usize = 10;
const AUTO_ATTACH_MINUTES: i64 = 5;

#[must_use]
pub fn next_issue_id(log: &EventLog<'_>) -> IssueId<'static> {
    let highest: Option<DecimalKey> = log
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Issue)
        .filter_map(|event| decimal_value(event.issue.as_ref()?.as_str()))
        .max();
    let next = highest.map_or_else(|| "0001".to_owned(), |highest| highest.successor(4));
    IssueId::from_string(next)
}

#[must_use]
pub fn issue_exists(log: &EventLog<'_>, issue: &str) -> bool {
    log.events().iter().any(|event| {
        event.kind == EventKind::Issue
            && event
                .issue
                .as_ref()
                .is_some_and(|known| known.as_str() == issue)
    })
}

#[must_use]
pub fn marker_state(log: &EventLog<'_>, marker: Option<&str>) -> MarkerState {
    match marker {
        None => MarkerState::Absent,
        Some(marker) if issue_exists(log, marker) => MarkerState::Valid(marker.to_owned()),
        Some(_) => MarkerState::Stale,
    }
}

pub(super) fn latest_open_within(log: &EventLog<'_>, clock: &Clock) -> Option<IssueId<'static>> {
    let newest = log.events().iter().rev().find(|event| {
        event.kind == EventKind::Issue
            && event
                .issue
                .as_ref()
                .is_none_or(|issue| !log.is_closed(issue))
    })?;
    let opened = newest.at.utc()?;
    let now = TimeZone::UTC.to_datetime(clock.now);
    let cutoff = now.checked_sub(AUTO_ATTACH_MINUTES.minutes()).ok()?;
    if opened < cutoff {
        return None;
    }
    newest
        .issue
        .as_ref()
        .map(|issue| IssueId::from_string(issue.as_str().to_owned()))
}

pub fn sole_open_issue<'l>(
    log: &'l EventLog<'_>,
) -> Result<Option<&'l IssueId<'l>>, ValidationError> {
    let open: Vec<&IssueId<'_>> = log.open_issues().collect();
    if open.len() > 1 {
        let shown: Vec<String> = open
            .iter()
            .take(OPEN_ISSUES_LISTED)
            .map(|issue| format!("#{}", issue.as_str()))
            .collect();
        let hidden = open.len().saturating_sub(OPEN_ISSUES_LISTED);
        return Err(ValidationError::SeveralOpen {
            count: open.len(),
            shown: shown.join(", "),
            more: if hidden > 0 {
                format!(" and {hidden} more")
            } else {
                String::new()
            },
        });
    }
    Ok(open.into_iter().next())
}
