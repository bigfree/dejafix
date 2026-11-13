use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::types::{EventLog, PathTable, Skipped};
use crate::core::event::{Event, EventKind, IssueId, LineContext, LineError, decode_line};
use crate::core::path::{PathContext, PathResolver, location_to_file, project_relative};
use crate::core::text::{python_lines, python_strip};
use crate::core::time::Clock;

const BOM: &[u8] = b"\xef\xbb\xbf";

fn decoded_lines<'a, 'c>(
    bytes: &'a [u8],
    line_context: &'c LineContext<'c>,
) -> impl Iterator<Item = (usize, Result<Option<Event<'a>>, LineError>)> + 'c
where
    'a: 'c,
{
    let body = bytes.strip_prefix(BOM).unwrap_or(bytes);
    python_lines(body).map(move |(line, raw)| {
        let decoded = std::str::from_utf8(raw)
            .map_err(|_| LineError::Utf8)
            .and_then(|text| {
                if python_strip(text).is_empty() {
                    Ok(None)
                } else {
                    decode_line(text, line_context).map(Some)
                }
            });
        (line, decoded)
    })
}

#[must_use]
pub fn skipped_lines(bytes: &[u8], clock: &Clock) -> Vec<Skipped> {
    let line_context = LineContext::new(clock);
    decoded_lines(bytes, &line_context)
        .filter_map(|(line, decoded)| decoded.err().map(|error| Skipped { line, error }))
        .collect()
}

impl<'a> EventLog<'a> {
    pub fn parse(
        bytes: &'a [u8],
        clock: &Clock,
        context: &PathContext,
        resolver: &impl PathResolver,
    ) -> Self {
        let line_context = LineContext::new(clock);
        let mut events = Vec::new();
        let mut skipped = Vec::new();
        for (line, decoded) in decoded_lines(bytes, &line_context) {
            match decoded {
                Ok(Some(event)) => events.push(event),
                Ok(None) => {}
                Err(error) => skipped.push(Skipped { line, error }),
            }
        }
        let mut log = EventLog {
            events,
            skipped,
            ..EventLog::default()
        };
        log.index(PathTable {
            context,
            resolver,
            by_spelling: HashMap::new(),
            by_path: HashMap::new(),
            paths: Vec::new(),
            scratch: Vec::new(),
        });
        log
    }

    pub(super) fn index<R: PathResolver>(&mut self, mut paths: PathTable<'_, 'a, R>) {
        for event in &self.events {
            if event.kind == EventKind::Fix
                && let Some(issue) = event
                    .issue
                    .as_ref()
                    .filter(|issue| !issue.as_str().is_empty())
            {
                self.closed.insert(issue.clone());
            }
            if let Some(id) = event
                .supersedes
                .as_ref()
                .filter(|id| !id.as_str().is_empty())
            {
                self.retired.insert(id.clone());
            }
        }
        let mut seen: HashSet<&str> = HashSet::new();
        self.open = self
            .events
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, event)| event.kind == EventKind::Issue)
            .filter_map(|(index, event)| {
                let issue = event
                    .issue
                    .as_ref()
                    .filter(|issue| !issue.as_str().is_empty())?;
                (!self.closed.contains(issue) && seen.insert(issue.as_str())).then_some(index)
            })
            .collect();
        let mut cited = Vec::new();
        let mut spans = Vec::with_capacity(self.events.len());
        for event in &self.events {
            let start = cited.len();
            paths.own(event, &mut cited);
            spans.push(start..cited.len());
        }
        let mut last_issue: HashMap<&IssueId<'a>, usize> = HashMap::new();
        for (index, event) in self.events.iter().enumerate() {
            if event.kind == EventKind::Issue
                && let Some(issue) = event
                    .issue
                    .as_ref()
                    .filter(|issue| !issue.as_str().is_empty())
            {
                last_issue.insert(issue, index);
            }
        }
        let mut by_path: Vec<Vec<usize>> = vec![Vec::new(); paths.paths.len()];
        for (index, (event, span)) in self.events.iter().zip(&spans).enumerate() {
            if self.retired.contains(&event.id) {
                continue;
            }
            let span = if span.is_empty() {
                event
                    .issue
                    .as_ref()
                    .and_then(|issue| spans.get(*last_issue.get(issue)?))
                    .unwrap_or(span)
            } else {
                span
            };
            for path in cited.get(span.clone()).unwrap_or_default() {
                if let Some(events) = by_path.get_mut(*path) {
                    events.push(index);
                }
            }
        }
        self.files = paths
            .paths
            .into_iter()
            .zip(by_path)
            .filter(|(_, events)| !events.is_empty())
            .collect();
    }
}

impl<'a, R: PathResolver> PathTable<'_, 'a, R> {
    fn own(&mut self, event: &Event<'a>, cited: &mut Vec<usize>) {
        let location = event.location.as_ref().and_then(|location| match location {
            Cow::Borrowed(text) => location_to_file(text),
            Cow::Owned(text) => location_to_file(text).map(|file| Cow::Owned(file.into_owned())),
        });
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        for path in event.files.iter().chain(location.as_ref()) {
            scratch.push(self.id(path));
        }
        scratch.sort_unstable();
        scratch.dedup();
        cited.extend_from_slice(&scratch);
        self.scratch = scratch;
    }

    #[expect(
        clippy::ptr_arg,
        reason = "whether the spelling is borrowed decides if the path keeps the log's lifetime"
    )]
    fn id(&mut self, spelling: &Cow<'a, str>) -> usize {
        if let Some(id) = self.by_spelling.get(spelling.as_ref()) {
            return *id;
        }
        let relative = match spelling {
            Cow::Borrowed(text) => project_relative(text, self.context, self.resolver),
            Cow::Owned(text) => project_relative(text, self.context, self.resolver).into_owned(),
        };
        let id = if let Some(id) = self.by_path.get(&relative) {
            *id
        } else {
            let id = self.paths.len();
            self.by_path.insert(relative.clone(), id);
            self.paths.push(relative);
            id
        };
        self.by_spelling.insert(spelling.clone(), id);
        id
    }
}
