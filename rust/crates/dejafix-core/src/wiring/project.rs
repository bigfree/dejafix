use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::thread;

use jiff::tz::TimeZone;

use super::types::{Project, Settings, WiringError};
use crate::adapters::fs::{
    StoreError, find_memory_dir, issues_dir, path_context, project_name, read_optional_text,
    write_text, write_text_if_changed,
};
use crate::core::log::EventLog;
use crate::core::summary::{
    SummaryInput, extract_purpose_from_map, extract_purpose_from_summary, issue_files,
    render_summary,
};

const SUMMARY_FILE: &str = "summary.md";
const PROJECT_MAP_FILE: &str = "PROJECT_MAP.md";

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
) -> Result<PathBuf, WiringError> {
    let map = read_optional_text(&project.memory.dir.join(PROJECT_MAP_FILE))
        .ok()
        .flatten();
    let path = project.summary_path();
    let existing = read_optional_text(&path).ok().flatten();
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
    let directory = issues_dir(&project.memory);
    if fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Ok(path);
    }
    fs::create_dir_all(&directory).map_err(|source| StoreError::Write {
        path: directory.clone(),
        source,
    })?;
    let files: Vec<(PathBuf, String)> = issue_files(log)
        .into_iter()
        .filter_map(|file| Some((directory.join(file.name?), file.contents)))
        .collect();
    write_issue_files(&files)?;
    Ok(path)
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
