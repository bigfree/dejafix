use super::project::regenerate_locked;
use super::types::{IssueScope, Project, Recorded, Regenerated, WiringError};
use crate::adapters::fs::{
    FsResolver, LockedLog, StoreError, clear_marker, detect_stack, lock_events, read_marker,
    write_marker,
};
use crate::adapters::git::head_short;
use crate::core::event::Event;
use crate::core::global::{GlobalHint, global_hint, hint_candidate};
use crate::core::log::EventLog;
use crate::core::write::{
    Command, Ids, MarkerChange, MarkerState, ValidationError, WriteContext, checks_marker,
    language_notice, marker_state, plan_command,
};

pub fn record(
    project: &Project,
    command: Command,
    ids: &mut impl Ids,
) -> Result<Recorded, WiringError> {
    record_with(project, |_| Ok(command), ids)
}

pub(crate) fn record_with(
    project: &Project,
    command: impl FnOnce(&EventLog<'_>) -> Result<Command, ValidationError>,
    ids: &mut impl Ids,
) -> Result<Recorded, WiringError> {
    let dir = &project.memory.dir;
    let commit = head_short(&project.memory.root);
    let (mut locked, bytes) = lock_events(dir)?;
    let mut log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    let command = command(&log)?;
    let hints: Vec<String> = language_notice(&command)?
        .map(str::to_owned)
        .into_iter()
        .collect();
    let marker = if checks_marker(&command) {
        let marker = read_marker(dir);
        if marker_state(&log, marker.as_deref()) == MarkerState::Stale {
            clear_marker(dir);
        }
        marker
    } else {
        None
    };
    let context = WriteContext {
        clock: &project.settings.clock,
        git_commit: commit.as_deref(),
        marker: marker.as_deref(),
        redact: project.settings.redact,
    };
    let plan = plan_command(&log, command, &context, ids)?;
    let events = plan
        .events
        .iter()
        .map(|event| event.id.as_str().to_owned())
        .collect();
    let mut hints = hints;
    hints.extend(plan.events.last().and_then(|event| hint(project, event)));
    let incomplete = commit_events(
        project,
        &mut locked,
        &mut log,
        plan.events,
        Some(&plan.marker),
        true,
    )?;
    drop(locked);
    Ok(Recorded {
        events,
        outcome: plan.outcome,
        notices: plan.notices,
        hints,
        incomplete,
    })
}

pub(super) fn commit_events<'a>(
    project: &Project,
    locked: &mut LockedLog,
    log: &mut EventLog<'a>,
    events: Vec<Event<'a>>,
    marker: Option<&MarkerChange>,
    regenerate: bool,
) -> Result<Option<WiringError>, WiringError> {
    let dir = &project.memory.dir;
    let mut lines = Vec::new();
    for event in &events {
        event
            .write_line(&mut lines)
            .map_err(|error| StoreError::Write {
                path: dir.join("events.jsonl"),
                source: error.into(),
            })?;
    }
    locked.append(&lines)?;
    let touched: Vec<String> = events
        .iter()
        .filter_map(|event| event.issue.as_ref().map(|issue| issue.as_str().to_owned()))
        .collect();
    match marker {
        None | Some(MarkerChange::Keep) => {}
        Some(MarkerChange::Set(issue)) => write_marker(dir, issue.as_str()),
        Some(MarkerChange::Clear) => clear_marker(dir),
    }
    for event in events {
        log.push(event, &project.context, &FsResolver);
    }
    Ok(if regenerate {
        regenerate_locked(project, log, &IssueScope::Touched(touched)).err()
    } else {
        None
    })
}

fn hint(project: &Project, event: &Event<'_>) -> Option<String> {
    let summary = event.summary.as_str();
    let candidate = hint_candidate(summary, event.kind, event.outcome);
    if candidate == GlobalHint::None {
        return None;
    }
    let stack = detect_stack(&project.memory.root);
    global_hint(summary, candidate, &stack.libraries)
}

pub fn regenerate(project: &Project) -> Result<Regenerated, WiringError> {
    let (locked, bytes) = lock_events(&project.memory.dir)?;
    let log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    let report = regenerate_locked(project, &log, &IssueScope::All)?;
    drop(locked);
    Ok(report)
}
