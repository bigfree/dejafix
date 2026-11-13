use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::num::NonZeroUsize;
use std::thread;

use super::types::{CommitsSince, FileHistory, History, Stale};
use crate::core::event::{Event, EventKind};
use crate::core::log::EventLog;
use crate::core::path::{PathContext, PathResolver, location_file};
use crate::core::text::char_prefix;

pub const STALE_COMMIT_THRESHOLD: usize = 3;

const MAX_WORKERS: usize = 8;

pub fn find_stale<'l, 'a, S: BuildHasher>(
    log: &'l EventLog<'a>,
    context: &PathContext,
    resolver: &impl PathResolver,
    history: &impl FileHistory,
    only_files: Option<&HashSet<&str, S>>,
    threshold: usize,
) -> Vec<Stale<'l, 'a>> {
    let candidates = candidates(log, context, resolver, only_files);
    let mut oldest: HashMap<&str, &str> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for (event, file) in &candidates {
        let at = event.at.as_str();
        if let Some(current) = oldest.get_mut(file.as_str()) {
            if at < *current {
                *current = at;
            }
        } else {
            oldest.insert(file.as_str(), at);
            order.push(file.as_str());
        }
    }
    let histories = histories(&order, &oldest, history);
    candidates
        .into_iter()
        .filter_map(|(event, file)| {
            let commits_since = match histories.get(&file)? {
                History::Missing => CommitsSince::Missing,
                History::Unknown => return None,
                History::Times(times) => {
                    let logged = event.at.as_str();
                    let count = times.iter().filter(|time| time.as_str() > logged).count();
                    if count < threshold {
                        return None;
                    }
                    CommitsSince::Count(count)
                }
            };
            Some(Stale {
                event,
                file,
                commits_since,
            })
        })
        .collect()
}

fn candidates<'l, 'a, S: BuildHasher>(
    log: &'l EventLog<'a>,
    context: &PathContext,
    resolver: &impl PathResolver,
    only_files: Option<&HashSet<&str, S>>,
) -> Vec<(&'l Event<'a>, String)> {
    let mut located: HashMap<&str, Option<String>> = HashMap::new();
    let mut found = Vec::new();
    for event in log.events() {
        if !matches!(
            event.kind,
            EventKind::Decision | EventKind::Fix | EventKind::Note
        ) || log.is_retired(&event.id)
        {
            continue;
        }
        if event.origin.auto_captured && matches!(event.kind, EventKind::Fix | EventKind::Note) {
            continue;
        }
        let Some(location) = event.location.as_deref() else {
            continue;
        };
        let file = located
            .entry(location)
            .or_insert_with(|| {
                location_file(location, context, resolver)
                    .map(|relative| relative.as_str().to_owned())
            })
            .clone();
        let Some(file) = file.filter(|file| !file.is_empty()) else {
            continue;
        };
        if only_files.is_some_and(|only| !only.contains(file.as_str())) {
            continue;
        }
        found.push((event, file));
    }
    found
}

fn histories(
    order: &[&str],
    oldest: &HashMap<&str, &str>,
    history: &impl FileHistory,
) -> HashMap<String, History> {
    let ask = |file: &&str| {
        let entry = if history.exists(file) {
            let since = oldest.get(file).copied().filter(|since| !since.is_empty());
            history
                .commit_times(file, since)
                .map_or(History::Unknown, History::Times)
        } else {
            History::Missing
        };
        ((*file).to_owned(), entry)
    };
    if order.len() < 2 {
        return order.iter().map(ask).collect();
    }
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(MAX_WORKERS);
    let chunk = order.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = order
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(ask).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

#[must_use]
pub fn stale_label(stale: &Stale<'_, '_>) -> String {
    let event = stale.event;
    let reason = match stale.commits_since {
        CommitsSince::Missing => format!("cited file {} no longer exists", stale.file),
        CommitsSince::Count(count) => format!("predates {count} commits to {}", stale.file),
    };
    format!(
        "{} [{}] \"{}\" — {reason}. Confirm, or retire with: djf decision \"...\" --supersedes {}",
        event.kind.as_str(),
        event.id.as_str(),
        char_prefix(event.summary.as_str(), 70),
        event.id.as_str(),
    )
}

impl<T: FileHistory + ?Sized> FileHistory for &T {
    fn exists(&self, relative: &str) -> bool {
        (**self).exists(relative)
    }

    fn commit_times(&self, relative: &str, since: Option<&str>) -> Option<Vec<String>> {
        (**self).commit_times(relative, since)
    }
}
