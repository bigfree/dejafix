use std::path::Path;

use super::types::{HooksAction, HooksOutcome, HooksReport};
use crate::adapters::fs::StoreError;
use crate::adapters::git::{
    HooksLocation, hook_has_crlf, hooks_location, read_hook, remove_hook, write_hook,
};
use crate::core::hooks::{HookChange, HookKind, Removal, remove_blocks, sync_hook};

pub(crate) fn configured_snippets(djf: &str) -> Vec<(HookKind, String)> {
    HookKind::ALL
        .into_iter()
        .map(|kind| (kind, kind.snippet(djf)))
        .collect()
}

pub fn hooks_action(root: &Path, action: HooksAction) -> Result<HooksOutcome, StoreError> {
    let directory = match hooks_location(root) {
        HooksLocation::Folder(directory) => directory,
        HooksLocation::Configured(path) => {
            let snippets = match &action {
                HooksAction::Install { djf } => configured_snippets(djf),
                HooksAction::Uninstall | HooksAction::Unknown(_) => Vec::new(),
            };
            return Ok(HooksOutcome::Configured { path, snippets });
        }
    };
    if !directory.exists() {
        return Ok(HooksOutcome::NoHooksDir);
    }
    match action {
        HooksAction::Install { djf } => {
            install(root, &directory, &djf).map(HooksOutcome::Installed)
        }
        HooksAction::Uninstall => uninstall(&directory).map(HooksOutcome::Removed),
        HooksAction::Unknown(action) => Ok(HooksOutcome::Unknown(action)),
    }
}

pub(crate) fn install(root: &Path, directory: &Path, djf: &str) -> Result<HooksReport, StoreError> {
    let mut report = HooksReport {
        folder: (directory != root.join(".git").join("hooks")).then(|| directory.to_path_buf()),
        ..HooksReport::default()
    };
    for kind in HookKind::ALL {
        let path = directory.join(kind.file_name());
        let Ok(existing) = read_hook(&path) else {
            report.unreadable.push(kind);
            continue;
        };
        match sync_hook(existing.as_deref(), &kind.snippet(djf)) {
            HookChange::Installed(text) => {
                write_hook(&path, &text, existing.is_some())?;
                report.installed.push(kind);
            }
            HookChange::Refreshed(text) => {
                write_hook(&path, &text, true)?;
                report.refreshed.push(kind);
            }
            HookChange::Current => {
                if let Some(text) = existing.as_deref()
                    && hook_has_crlf(&path)
                {
                    write_hook(&path, text, true)?;
                    report.refreshed.push(kind);
                }
            }
            HookChange::Skipped => report.skipped.push(kind),
        }
    }
    Ok(report)
}

fn uninstall(directory: &Path) -> Result<Vec<HookKind>, StoreError> {
    let mut removed = Vec::new();
    for kind in HookKind::ALL {
        let path = directory.join(kind.file_name());
        let Ok(Some(content)) = read_hook(&path) else {
            continue;
        };
        match remove_blocks(&content) {
            Removal::Untouched => continue,
            Removal::Delete => remove_hook(&path)?,
            Removal::Write(text) => write_hook(&path, &text, true)?,
        }
        removed.push(kind);
    }
    Ok(removed)
}
