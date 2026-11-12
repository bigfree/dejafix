use std::borrow::Cow;
use std::collections::HashMap;

use super::types::{EventLog, PathTable};
use crate::core::event::{Event, EventKind};
use crate::core::path::{
    PathContext, PathResolver, RelativePath, location_to_file, project_relative,
};

impl<'a> EventLog<'a> {
    pub fn push(&mut self, event: Event<'a>, context: &PathContext, resolver: &impl PathResolver) {
        let retires = event
            .supersedes
            .as_ref()
            .is_some_and(|id| !id.as_str().is_empty());
        let reopens = event.kind == EventKind::Issue
            && event.issue.as_ref().is_some_and(|issue| {
                !issue.as_str().is_empty()
                    && self
                        .events
                        .iter()
                        .any(|known| known.issue.as_ref() == Some(issue))
            });
        self.events.push(event);
        if retires || reopens {
            self.rebuild(context, resolver);
            return;
        }
        let index = self.events.len() - 1;
        let Some(event) = self.events.get(index) else {
            return;
        };
        let issue = event
            .issue
            .as_ref()
            .filter(|issue| !issue.as_str().is_empty());
        match (event.kind, issue) {
            (EventKind::Fix, Some(issue)) => {
                let issue = issue.clone();
                let events = &self.events;
                self.open.retain(|open| {
                    events.get(*open).and_then(|known| known.issue.as_ref()) != Some(&issue)
                });
                self.closed.insert(issue);
            }
            (EventKind::Issue, Some(_)) => self.open.insert(0, index),
            _ => {}
        }
        if self.retired.contains(&event.id) {
            return;
        }
        let mut paths = own_paths(event, context, resolver);
        if paths.is_empty()
            && let Some(issue) = issue
            && let Some(source) =
                self.events.iter().rev().find(|known| {
                    known.kind == EventKind::Issue && known.issue.as_ref() == Some(issue)
                })
        {
            paths = own_paths(source, context, resolver);
        }
        for path in paths {
            self.files.entry(path).or_default().push(index);
        }
    }

    fn rebuild(&mut self, context: &PathContext, resolver: &impl PathResolver) {
        self.closed.clear();
        self.retired.clear();
        self.open.clear();
        self.files.clear();
        self.index(PathTable {
            context,
            resolver,
            by_spelling: HashMap::new(),
            by_path: HashMap::new(),
            paths: Vec::new(),
            scratch: Vec::new(),
        });
    }
}

fn own_paths(
    event: &Event<'_>,
    context: &PathContext,
    resolver: &impl PathResolver,
) -> Vec<RelativePath<'static>> {
    let location = event.location.as_deref().and_then(location_to_file);
    let mut paths: Vec<RelativePath<'static>> = event
        .files
        .iter()
        .map(Cow::as_ref)
        .chain(location.as_deref())
        .map(|spelling| project_relative(spelling, context, resolver).into_owned())
        .collect();
    paths.sort_unstable_by(|left, right| left.as_str().cmp(right.as_str()));
    paths.dedup_by(|left, right| left.as_str() == right.as_str());
    paths
}
