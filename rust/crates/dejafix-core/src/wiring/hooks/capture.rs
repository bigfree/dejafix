use std::path::Path;

use super::types::CaptureOutcome;
use crate::adapters::fs::{
    FsResolver, MEMORY_DIR, MemoryDir, lock_events, path_context, read_optional_text,
};
use crate::adapters::git::{head_short, last_changes, last_message};
use crate::core::capture::{capture_event, first_line, merge_event};
use crate::core::log::EventLog;
use crate::core::path::is_memory_path;
use crate::core::redact::{redact_event, redaction_notice};
use crate::core::write::Ids;
use crate::wiring::record::commit_events;
use crate::wiring::types::{Project, Settings};

const DISABLED: &str = "auto_capture = false";

pub fn auto_capture(
    cwd: &Path,
    settings: &Settings,
    trigger: &str,
    ids: &mut impl Ids,
) -> CaptureOutcome {
    let dir = cwd.join(MEMORY_DIR);
    if !dir.exists() {
        return CaptureOutcome::Skipped;
    }
    if read_optional_text(&dir.join("config.toml"))
        .ok()
        .flatten()
        .is_some_and(|config| config.contains(DISABLED))
    {
        return CaptureOutcome::Skipped;
    }
    let merge = match trigger {
        "commit" => false,
        "merge" => true,
        _ => return CaptureOutcome::Skipped,
    };
    let Ok(context) = path_context(cwd) else {
        return CaptureOutcome::Skipped;
    };
    let project = Project {
        memory: MemoryDir {
            root: cwd.to_path_buf(),
            dir,
        },
        settings: settings.clone(),
        context,
    };
    let Some(message) = last_message(cwd) else {
        return CaptureOutcome::Skipped;
    };
    let changes = if merge { Vec::new() } else { last_changes(cwd) };
    let commit = head_short(cwd);
    capture(&project, &message, &changes, commit, merge, ids).unwrap_or(CaptureOutcome::Skipped)
}

fn capture(
    project: &Project,
    message: &str,
    changes: &[crate::core::capture::Change],
    commit: Option<String>,
    merge: bool,
    ids: &mut impl Ids,
) -> Option<CaptureOutcome> {
    let (mut locked, bytes) = lock_events(&project.memory.dir).ok()?;
    let mut log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    if let Some(commit) = commit.as_deref()
        && log
            .events()
            .iter()
            .any(|event| event.origin.git_commit.as_deref() == Some(commit))
    {
        return None;
    }
    let clock = &project.settings.clock;
    let mut event = if merge {
        merge_event(message, commit, clock, ids)?
    } else {
        let is_memory = |path: &str| is_memory_path(path, &project.context, &FsResolver);
        capture_event(message, changes, commit, is_memory, clock, ids)?
    };
    let kind = event.kind;
    let summary = event.summary.as_str().to_owned();
    let fired = if project.settings.redact {
        redact_event(&mut event)
    } else {
        Vec::new()
    };
    let notices: Vec<String> = redaction_notice(&fired).into_iter().collect();
    let incomplete =
        commit_events(project, &mut locked, &mut log, vec![event], None, true, ids).ok()?;
    drop(locked);
    if incomplete.is_some() {
        return None;
    }
    Some(if merge {
        CaptureOutcome::Merged {
            first_line: first_line(message),
            notices,
        }
    } else {
        CaptureOutcome::Captured {
            kind,
            summary,
            notices,
        }
    })
}
