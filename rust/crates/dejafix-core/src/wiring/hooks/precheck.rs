use std::borrow::Cow;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::Path;
use std::thread;

use super::types::{GitAnswers, PrecheckOutcome, PrecheckRequest};
use crate::adapters::fs::{
    FsResolver, MEMORY_DIR, MemoryDir, lock_events, path_context, read_events_file, read_snooze,
    remove_snooze, write_snooze,
};
use crate::adapters::git::{GitHistory, recent_changes, staged_files, working_files};
use crate::core::event::{Event, EventId, EventKind, Extra, Origin, Summary};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::core::precheck::{
    RECENT_DAYS, Severity, Targets, Warning, analyze, expiry_iso, parse_snooze, read_expiry,
    remaining, snooze_note, snoozed_until, targets,
};
use crate::core::redact::{redact_event, redaction_notice};
use crate::core::stale::{STALE_COMMIT_THRESHOLD, find_stale};
use crate::core::time::Timestamp;
use crate::core::write::Ids;
use crate::wiring::record::commit_events;
use crate::wiring::types::{Project, Settings, WiringError};

const MAX_GIT_WORKERS: usize = 8;

pub fn precheck(
    cwd: &Path,
    settings: &Settings,
    request: &PrecheckRequest,
    ids: &mut impl Ids,
) -> Result<PrecheckOutcome, WiringError> {
    let dir = cwd.join(MEMORY_DIR);
    if !dir.exists() {
        return Ok(PrecheckOutcome::NoProject);
    }
    let now = settings.clock.now;
    if request.unsnooze {
        return Ok(PrecheckOutcome::Unsnoozed(remove_snooze(&dir)));
    }
    if let Some(text) = &request.snooze {
        let expiry = match parse_snooze(text).and_then(|delta| snoozed_until(now, delta, text)) {
            Ok(expiry) => expiry,
            Err(error) => return Ok(PrecheckOutcome::BadSnooze(error)),
        };
        write_snooze(&dir, &expiry_iso(expiry))?;
        let notices = note(cwd, &dir, settings, &snooze_note(text, expiry), ids);
        return Ok(PrecheckOutcome::Snoozed { expiry, notices });
    }
    if let Some(left) = active_snooze(&dir, now) {
        return Ok(PrecheckOutcome::Snoozing(left));
    }
    let listing = request.files.is_empty().then(|| {
        let root = cwd.to_path_buf();
        let working = request.working;
        thread::spawn(move || {
            if working {
                working_files(&root)
            } else {
                staged_files(&root)
            }
        })
    });
    let bytes = read_events_file(&dir).ok();
    let context = path_context(cwd).ok();
    let parsed = bytes
        .as_deref()
        .zip(context.as_ref())
        .map(|(bytes, context)| EventLog::parse(bytes, &settings.clock, context, &FsResolver));
    let files = match listing {
        Some(listing) => listing.join().unwrap_or_default(),
        None => request.files.clone(),
    };
    if files.is_empty() {
        return Ok(PrecheckOutcome::NoFiles);
    }
    let (Some(log), Some(context)) = (parsed, context.as_ref()) else {
        return Ok(PrecheckOutcome::Unreadable);
    };
    let history = GitHistory {
        root: cwd.to_path_buf(),
    };
    let warnings = analyze_files(&log, context, &files, now, &history);
    if warnings.is_empty() {
        return Ok(PrecheckOutcome::Clean);
    }
    let blocking = warnings
        .iter()
        .any(|warning| warning.severity == Severity::Block);
    Ok(PrecheckOutcome::Warnings { warnings, blocking })
}

impl GitAnswers for GitHistory {
    fn churn(&self, relative: &str) -> Option<usize> {
        recent_changes(&self.root, relative, RECENT_DAYS)
    }
}

pub(crate) fn analyze_files(
    log: &EventLog<'_>,
    context: &PathContext,
    files: &[String],
    now: jiff::Timestamp,
    history: &impl GitAnswers,
) -> Vec<Warning> {
    let found = targets(files, log, context, &FsResolver);
    let only: HashSet<&str> = found.files.iter().map(String::as_str).collect();
    let (churn, stale) = thread::scope(|scope| {
        let stale = scope.spawn(|| {
            find_stale(
                log,
                context,
                &FsResolver,
                history,
                Some(&only),
                STALE_COMMIT_THRESHOLD,
            )
        });
        let churn = churn_counts(history, &found);
        (churn, stale.join().unwrap_or_default())
    });
    analyze(log, &found, &churn, &stale, now)
}

fn churn_counts(history: &impl GitAnswers, found: &Targets) -> Vec<Option<usize>> {
    let asked: Vec<(usize, &str)> = found
        .files
        .iter()
        .zip(&found.events)
        .enumerate()
        .filter(|(_, (_, events))| !events.is_empty())
        .map(|(position, (file, _))| (position, file.as_str()))
        .collect();
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(MAX_GIT_WORKERS);
    let chunk = asked.len().div_ceil(workers).max(1);
    let answers: Vec<(usize, Option<usize>)> = thread::scope(|scope| {
        let handles: Vec<_> = asked
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|(position, file)| (*position, history.churn(file)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let mut churn = vec![None; found.files.len()];
    for (position, count) in answers {
        if let Some(slot) = churn.get_mut(position) {
            *slot = count;
        }
    }
    churn
}

fn active_snooze(dir: &Path, now: jiff::Timestamp) -> Option<String> {
    let text = match read_snooze(dir) {
        Ok(Some(text)) => text,
        Ok(None) => return None,
        Err(_) => {
            remove_snooze(dir);
            return None;
        }
    };
    match read_expiry(&text) {
        Some(expiry) if expiry > now => Some(remaining(expiry, now)),
        _ => {
            remove_snooze(dir);
            None
        }
    }
}

fn note(
    cwd: &Path,
    dir: &Path,
    settings: &Settings,
    summary: &str,
    ids: &mut impl Ids,
) -> Vec<String> {
    let Ok(context) = path_context(cwd) else {
        return Vec::new();
    };
    let project = Project {
        memory: MemoryDir {
            root: cwd.to_path_buf(),
            dir: dir.to_path_buf(),
        },
        settings: settings.clone(),
        context,
    };
    let Some(summary) = Summary::new(Cow::Owned(summary.to_owned())) else {
        return Vec::new();
    };
    let mut event = Event {
        id: ids.event_id(),
        at: Timestamp::now(&settings.clock),
        kind: EventKind::Note,
        summary,
        issue: None,
        outcome: None,
        files: Vec::new(),
        location: None,
        command: None,
        notes: None,
        supersedes: None::<EventId<'static>>,
        origin: Origin::default(),
        extra: Extra::default(),
    };
    let fired = if settings.redact {
        redact_event(&mut event)
    } else {
        Vec::new()
    };
    let Ok((mut locked, bytes)) = lock_events(dir) else {
        return Vec::new();
    };
    let mut log = EventLog::parse(&bytes, &settings.clock, &project.context, &FsResolver);
    if commit_events(
        &project,
        &mut locked,
        &mut log,
        vec![event],
        None,
        false,
        ids,
    )
    .is_err()
    {
        return Vec::new();
    }
    redaction_notice(&fired).into_iter().collect()
}

impl<T: GitAnswers + ?Sized> GitAnswers for &T {
    fn churn(&self, relative: &str) -> Option<usize> {
        (**self).churn(relative)
    }
}
