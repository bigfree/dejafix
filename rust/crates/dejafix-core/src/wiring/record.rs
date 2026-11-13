use std::path::PathBuf;

use super::project::regenerate_locked;
use super::types::{Project, Recorded, WiringError};
use crate::adapters::fs::{
    FsResolver, GlobalStore, LockedLog, StoreError, clear_marker, detect_stack, lock_events,
    read_marker, write_marker,
};
use crate::adapters::git::head_short;
use crate::core::event::{Event, EventKind};
use crate::core::global::{gotcha_entry, promotable_libraries, promotable_set, promotion};
use crate::core::log::EventLog;
use crate::core::time::iso_micros;
use crate::core::write::{
    Command, Ids, MarkerChange, MarkerState, ValidationError, WriteContext, checks_marker,
    marker_state, plan_command,
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
    let incomplete = commit_events(
        project,
        &mut locked,
        &mut log,
        plan.events,
        Some(&plan.marker),
        true,
        ids,
    )?;
    drop(locked);
    Ok(Recorded {
        events,
        outcome: plan.outcome,
        notices: plan.notices,
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
    ids: &mut impl Ids,
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
    match marker {
        None | Some(MarkerChange::Keep) => {}
        Some(MarkerChange::Set(issue)) => write_marker(dir, issue.as_str()),
        Some(MarkerChange::Clear) => clear_marker(dir),
    }
    for event in &events {
        promote(project, event, ids);
    }
    for event in events {
        log.push(event, &project.context, &FsResolver);
    }
    Ok(if regenerate {
        regenerate_locked(project, log).err()
    } else {
        None
    })
}

fn promote(project: &Project, event: &Event<'_>, ids: &mut impl Ids) {
    if !matches!(
        event.kind,
        EventKind::Attempt | EventKind::Decision | EventKind::Note
    ) {
        return;
    }
    let store = GlobalStore::new(&project.settings.dejafix_home);
    let stack = detect_stack(&project.memory.root);
    let libraries = promotable_libraries(&stack);
    let _ = store.record_promotable(&libraries);
    let Some(cached) = store.promotable() else {
        return;
    };
    let promotable = promotable_set(&cached);
    let source = project.name();
    let discovered = iso_micros(&project.settings.clock);
    let _ = store.append_gotcha_if_new(|existing| {
        promotion(
            event.summary.as_str(),
            event.kind,
            event.outcome,
            &promotable,
            &libraries,
            existing,
        )
        .map(|library| {
            gotcha_entry(
                &library,
                event.summary.as_str(),
                ids.entry_id("got"),
                discovered,
                &source,
            )
        })
    });
}

pub fn regenerate(project: &Project) -> Result<PathBuf, WiringError> {
    let (locked, bytes) = lock_events(&project.memory.dir)?;
    let log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    let path = regenerate_locked(project, &log)?;
    drop(locked);
    Ok(path)
}
