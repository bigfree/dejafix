use std::collections::HashSet;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::thread;

use jiff::tz::TimeZone;

use super::types::{IssueScope, Project, Regenerated, Settings, WiringError};
use crate::adapters::fs::{
    MemoryDir, StoreError, file_head, find_memory_dir, issues_dir, path_context, project_name,
    read_text_within, regular_file_names, remove_if_present, write_text, write_text_if_changed,
};
use crate::core::log::EventLog;
use crate::core::summary::{
    SummaryInput, extract_purpose_from_map, extract_purpose_from_summary, is_renamed_issue_file,
    issue_file_names, issue_files_where, render_summary,
};

const SUMMARY_FILE: &str = "summary.md";
const PROJECT_MAP_FILE: &str = "PROJECT_MAP.md";
const HEAD_LIMIT: usize = 256;

impl Project {
    pub fn open(cwd: &Path, settings: Settings) -> Result<Self, WiringError> {
        let memory = find_memory_dir(cwd)?;
        let context = path_context(&memory.root)?;
        Ok(Self {
            memory,
            settings,
            context,
        })
    }

    pub fn at(root: &Path, settings: Settings) -> Result<Self, WiringError> {
        let memory = MemoryDir::at(root)?;
        let context = path_context(root)?;
        Ok(Self {
            memory,
            settings,
            context,
        })
    }

    #[must_use]
    pub fn summary_path(&self) -> PathBuf {
        self.memory.dir.join(SUMMARY_FILE)
    }

    #[must_use]
    pub fn name(&self) -> String {
        project_name(&self.memory.root)
    }
}

pub(super) fn regenerate_locked(
    project: &Project,
    log: &EventLog<'_>,
    scope: &IssueScope,
) -> Result<Regenerated, WiringError> {
    let root = &project.memory.root;
    let map = read_text_within(&project.memory.dir.join(PROJECT_MAP_FILE), root)
        .ok()
        .flatten();
    let path = project.summary_path();
    let existing = read_text_within(&path, root).ok().flatten();
    let purpose = map
        .as_deref()
        .and_then(extract_purpose_from_map)
        .or_else(|| existing.as_deref().and_then(extract_purpose_from_summary));
    let name = project.name();
    let today = TimeZone::UTC.to_datetime(project.settings.clock.now).date();
    let summary = render_summary(
        log,
        &SummaryInput {
            project_name: &name,
            today,
            purpose,
        },
    );
    write_text(&path, &summary)?;
    let mut report = Regenerated {
        path,
        removed: 0,
        not_removed: Vec::new(),
        unreadable: log.skipped().len(),
    };
    let directory = issues_dir(&project.memory);
    if fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Ok(report);
    }
    fs::create_dir_all(&directory).map_err(|source| StoreError::Write {
        path: directory.clone(),
        source,
    })?;
    let present = regular_file_names(&directory)?;
    let names = issue_file_names(log);
    let wanted: Option<HashSet<&str>> = match scope {
        IssueScope::All => None,
        IssueScope::Touched(touched) => {
            let listed: HashSet<&str> = present.iter().map(String::as_str).collect();
            Some(
                names
                    .iter()
                    .filter(|(id, name)| {
                        !listed.contains(name.as_str()) || touched.iter().any(|issue| issue == id)
                    })
                    .map(|(id, _)| *id)
                    .collect(),
            )
        }
    };
    let files: Vec<(PathBuf, String)> = issue_files_where(log, |id| {
        wanted.as_ref().is_none_or(|wanted| wanted.contains(id))
    })
    .into_iter()
    .filter_map(|file| Some((directory.join(file.name?), file.contents)))
    .collect();
    write_issue_files(&files)?;
    if *scope == IssueScope::All {
        remove_renamed(&directory, &names, &present, &mut report);
    }
    Ok(report)
}

fn remove_renamed(
    directory: &Path,
    names: &[(&str, String)],
    present: &[String],
    report: &mut Regenerated,
) {
    let rendered: HashSet<&str> = names.iter().map(|(_, name)| name.as_str()).collect();
    let ids: HashSet<&str> = names.iter().map(|(id, _)| *id).collect();
    for name in present
        .iter()
        .filter(|name| !rendered.contains(name.as_str()))
    {
        let path = directory.join(name);
        let head = file_head(&path, HEAD_LIMIT).unwrap_or_default();
        if !is_renamed_issue_file(name, &head, &rendered, &ids) {
            continue;
        }
        match remove_if_present(&path) {
            Ok(()) => report.removed += 1,
            Err(error) => report
                .not_removed
                .push(format!("Could not remove {name}: {error}")),
        }
    }
}

fn write_issue_files(files: &[(PathBuf, String)]) -> Result<(), StoreError> {
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(8);
    let chunk = files.len().div_ceil(workers).max(1);
    thread::scope(|scope| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter().try_for_each(|(path, contents)| {
                        write_text_if_changed(path, contents).map(drop)
                    })
                })
            })
            .collect();
        handles.into_iter().try_for_each(|handle| {
            handle.join().unwrap_or_else(|_| {
                Err(StoreError::Write {
                    path: PathBuf::new(),
                    source: std::io::Error::other("an issue-file writer stopped"),
                })
            })
        })
    })
}
