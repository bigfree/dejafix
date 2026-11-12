use std::collections::HashSet;

use jiff::SignedDuration;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;

use super::types::{Found, Severity, Targets, Warning, WarningKind};
use crate::core::event::{CaptureSource, Event, EventKind, IssueId, Outcome};
use crate::core::log::EventLog;
use crate::core::path::{PathContext, PathResolver, is_memory_path, project_relative};
use crate::core::stale::{CommitsSince, Stale};
use crate::core::text::{char_prefix, python_strip};
use crate::core::time::Timestamp;

pub const HIGH_CHURN_THRESHOLD: usize = 4;
pub const FAILED_ATTEMPT_BLOCK_COUNT: usize = 3;
pub const RECENT_DAYS: u32 = 30;

const SHOWN: usize = 3;
const DAY_SECONDS: i64 = 86_400;

pub fn targets(
    files: &[String],
    log: &EventLog<'_>,
    context: &PathContext,
    resolver: &impl PathResolver,
) -> Targets {
    let mut seen: HashSet<String> = HashSet::new();
    let mut found = Targets::default();
    for file in files {
        if is_memory_path(file, context, resolver) {
            continue;
        }
        let stripped = python_strip(file);
        if stripped.is_empty() {
            continue;
        }
        let relative = project_relative(stripped, context, resolver);
        if relative.as_str() == "." || !seen.insert(relative.as_str().to_owned()) {
            continue;
        }
        let relative = relative.as_str().to_owned();
        let cited = log.live_indices_for_file(&relative);
        let events = log
            .events()
            .iter()
            .enumerate()
            .filter(|(index, event)| {
                !log.is_retired(&event.id)
                    && (cited.binary_search(index).is_ok()
                        || event.summary.as_str().contains(relative.as_str()))
            })
            .map(|(index, _)| index)
            .collect();
        found.files.push(relative);
        found.events.push(events);
    }
    found
}

#[must_use]
pub fn analyze(
    log: &EventLog<'_>,
    targets: &Targets,
    churn: &[Option<usize>],
    stale: &[Stale<'_, '_>],
    now: jiff::Timestamp,
) -> Vec<Warning> {
    let cutoff = TimeZone::UTC
        .to_datetime(now)
        .checked_sub(SignedDuration::from_secs(
            i64::from(RECENT_DAYS) * DAY_SECONDS,
        ))
        .unwrap_or(DateTime::MIN);
    let mut warnings = Vec::new();
    for (position, (file, indices)) in targets.files.iter().zip(&targets.events).enumerate() {
        let events: Vec<&Event<'_>> = indices
            .iter()
            .filter_map(|index| log.events().get(*index))
            .collect();
        if events.is_empty() {
            continue;
        }
        let recent: Vec<&Event<'_>> = events
            .iter()
            .copied()
            .filter(|event| event.at.utc().is_none_or(|at| at >= cutoff))
            .collect();
        let churn = churn.get(position).copied().flatten();
        let found = [
            failed_attempts(&events, now),
            open_issues(log, &events),
            high_churn(churn, &recent),
            recent_revert(&recent, now),
            recent_decision(&recent, now),
            possibly_stale(file, stale),
        ];
        warnings.extend(
            found
                .into_iter()
                .flatten()
                .map(|(severity, kind, title, details)| Warning {
                    file: file.clone(),
                    severity,
                    kind,
                    title,
                    details,
                }),
        );
    }
    warnings
}

fn open_issues(log: &EventLog<'_>, events: &[&Event<'_>]) -> Found {
    let open: Vec<&Event<'_>> = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == EventKind::Issue
                && event
                    .issue
                    .as_ref()
                    .is_none_or(|issue| !log.is_closed(issue))
        })
        .collect();
    if open.is_empty() {
        return None;
    }
    Some((
        Severity::Warn,
        WarningKind::OpenIssues,
        format!(
            "{} unresolved issue{} on this file",
            open.len(),
            plural(open.len())
        ),
        open.iter()
            .take(SHOWN)
            .map(|issue| {
                format!(
                    "#{}: {}",
                    issue.issue.as_ref().map_or("None", IssueId::as_str),
                    char_prefix(issue.summary.as_str(), 80)
                )
            })
            .collect(),
    ))
}

fn high_churn(churn: Option<usize>, recent: &[&Event<'_>]) -> Found {
    let count = churn.unwrap_or_else(|| {
        recent
            .iter()
            .filter(|event| {
                event
                    .origin
                    .git_commit
                    .as_deref()
                    .is_some_and(|commit| !commit.is_empty())
            })
            .count()
    });
    (count >= HIGH_CHURN_THRESHOLD).then(|| {
        (
            Severity::Warn,
            WarningKind::HighChurn,
            format!("HIGH CHURN: {count} changes in last {RECENT_DAYS} days"),
            vec!["May indicate unresolved architectural issue".to_owned()],
        )
    })
}

fn recent_revert(recent: &[&Event<'_>], now: jiff::Timestamp) -> Found {
    let revert = recent.iter().rev().find(|event| {
        event.kind == EventKind::Attempt
            && event.outcome == Some(Outcome::Failed)
            && event.origin.source == Some(CaptureSource::GitPostRevert)
    })?;
    let message = revert
        .origin
        .git_message
        .as_deref()
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| char_prefix(revert.summary.as_str(), 80));
    Some((
        Severity::Warn,
        WarningKind::RecentRevert,
        "Recent revert affected this file".to_owned(),
        vec![
            format!("Reverted: {message}"),
            format!("  ({})", age(&revert.at, now)),
        ],
    ))
}

fn recent_decision(recent: &[&Event<'_>], now: jiff::Timestamp) -> Found {
    let decision = recent
        .iter()
        .rev()
        .find(|event| event.kind == EventKind::Decision)?;
    Some((
        Severity::Info,
        WarningKind::RelevantDecision,
        "Recent decision affects this file".to_owned(),
        vec![
            char_prefix(decision.summary.as_str(), 100).to_owned(),
            format!("  ({})", age(&decision.at, now)),
        ],
    ))
}

fn possibly_stale(file: &str, stale: &[Stale<'_, '_>]) -> Found {
    let items: Vec<&Stale<'_, '_>> = stale.iter().filter(|item| item.file == file).collect();
    if items.is_empty() {
        return None;
    }
    let mut details: Vec<String> = items
        .iter()
        .take(SHOWN)
        .map(|item| {
            let reason = match item.commits_since {
                CommitsSince::Missing => "cited file no longer exists".to_owned(),
                CommitsSince::Count(count) => format!("predates {count} commits to this file"),
            };
            format!(
                "{} [{}] \"{}\" — {reason}",
                item.event.kind.as_str(),
                item.event.id.as_str(),
                char_prefix(item.event.summary.as_str(), 70)
            )
        })
        .collect();
    details.push(
        "Confirm it still holds, or retire it: djf decision \"...\" --supersedes <id>".to_owned(),
    );
    let title = if items.len() == 1 {
        "1 possibly-stale memory cites this file".to_owned()
    } else {
        format!("{} possibly-stale memories cite this file", items.len())
    };
    Some((Severity::Warn, WarningKind::PossiblyStale, title, details))
}

fn failed_attempts(events: &[&Event<'_>], now: jiff::Timestamp) -> Found {
    let lessons: Vec<&Event<'_>> = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == EventKind::Attempt
                && matches!(event.outcome, Some(Outcome::Failed | Outcome::Partial))
        })
        .collect();
    if lessons.is_empty() {
        return None;
    }
    let count = lessons.len();
    let failed = lessons
        .iter()
        .filter(|event| event.outcome == Some(Outcome::Failed))
        .count();
    let severity = if failed >= FAILED_ATTEMPT_BLOCK_COUNT {
        Severity::Block
    } else {
        Severity::Warn
    };
    let split = count.saturating_sub(SHOWN);
    let (hidden, shown) = lessons.split_at(split);
    let mut details: Vec<String> = shown
        .iter()
        .rev()
        .map(|attempt| {
            let mark = if attempt.outcome == Some(Outcome::Partial) {
                "~"
            } else {
                "x"
            };
            format!(
                "{mark} {} ({})",
                char_prefix(attempt.summary.as_str(), 90),
                age(&attempt.at, now)
            )
        })
        .collect();
    if count > SHOWN {
        details.push(format!(
            "  ... and {} more{}",
            count - SHOWN,
            hidden_hint(hidden)
        ));
    }
    let partial = count - failed;
    let partly = if partial > 0 {
        format!(", {partial} only partly worked")
    } else {
        String::new()
    };
    Some((
        severity,
        WarningKind::FailedAttempts,
        format!(
            "What already failed here ({count} attempt{}{partly}):",
            plural(count)
        ),
        details,
    ))
}

fn hidden_hint(hidden: &[&Event<'_>]) -> String {
    let mut seen = HashSet::new();
    let ids: Vec<String> = hidden
        .iter()
        .rev()
        .filter_map(|event| event.issue.as_ref())
        .map(IssueId::as_str)
        .filter(|issue| !issue.is_empty() && seen.insert(*issue))
        .map(|issue| format!("#{issue}"))
        .collect();
    if ids.is_empty() {
        " (djf search --failed-only)".to_owned()
    } else {
        format!(", from {} (get_issue(issue_id))", ids.join(", "))
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[must_use]
pub fn age(timestamp: &Timestamp<'_>, now: jiff::Timestamp) -> String {
    let Some(at) = timestamp.utc() else {
        return "unknown".to_owned();
    };
    let Ok(at) = at.to_zoned(TimeZone::UTC) else {
        return "unknown".to_owned();
    };
    let delta = now.duration_since(at.timestamp());
    let micros = delta.as_micros();
    let day = i128::from(DAY_SECONDS) * 1_000_000;
    let days = micros.div_euclid(day);
    let seconds = micros.rem_euclid(day) / 1_000_000;
    match days {
        0 => {
            let hours = seconds / 3_600;
            if hours == 0 {
                "just now".to_owned()
            } else {
                format!("{hours}h ago")
            }
        }
        1 => "yesterday".to_owned(),
        days if days < 7 => format!("{days}d ago"),
        days if days < 30 => format!("{}w ago", days.div_euclid(7)),
        days => format!("{}mo ago", days.div_euclid(30)),
    }
}
