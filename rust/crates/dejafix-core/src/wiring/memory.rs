use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use super::global::{add_action, edit_action, global_query, non_empty, upgrade_action};
use super::links::links_text;
use super::types::{Project, Settings, WiringError};
use crate::adapters::fs::{
    FsResolver, GlobalStore, detect_stack, path_context, project_name, read_events_file,
    read_json_text,
};
use crate::adapters::git::GitHistory;
use crate::core::brief::{BriefInput, render_brief};
use crate::core::global::{Entry, GlobalAction, GlobalOutcome, Usage, filtered, relevant_entries};
use crate::core::log::EventLog;
use crate::core::stale::{STALE_COMMIT_THRESHOLD, find_stale};
use crate::core::time::iso_micros;
use crate::core::write::Ids;

pub fn brief(cwd: &Path, settings: &Settings, color: bool) -> Result<String, WiringError> {
    let project = Project::open(cwd, settings.clone())?;
    let bytes = read_events_file(&project.memory.dir)?;
    let log = EventLog::parse(&bytes, &settings.clock, &project.context, &FsResolver);
    let context = path_context(cwd)?;
    let history = GitHistory {
        root: cwd.to_path_buf(),
    };
    let stale = find_stale(
        &log,
        &context,
        &FsResolver,
        &history,
        None::<&HashSet<&str>>,
        STALE_COMMIT_THRESHOLD,
    );
    let stack = detect_stack(cwd);
    let store = GlobalStore::new(&settings.dejafix_home);
    let stored = store.load_gotchas().ok();
    let gotchas: Vec<&Entry> = stored
        .as_deref()
        .map_or_else(Vec::new, |stored| relevant_entries(&stack, None, stored));
    let name = project_name(cwd);
    let links = links_text(&settings.dejafix_home, &project.memory.root);
    Ok(render_brief(&BriefInput {
        name: &name,
        log: &log,
        stale: &stale,
        gotchas: &gotchas,
        links: &links,
        now: settings.clock.now,
        color,
    }))
}

fn import(store: &GlobalStore, cwd: &Path, file: &str) -> Result<GlobalOutcome, WiringError> {
    let Some(text) = read_json_text(&cwd.join(file))? else {
        return Ok(GlobalOutcome::FileNotFound(file.to_owned()));
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(data)) => {
            let (gotchas, skipped_patterns) = store.import(&data)?;
            Ok(GlobalOutcome::Imported {
                gotchas,
                skipped_patterns,
            })
        }
        Ok(_) => Ok(GlobalOutcome::InvalidJson(
            "top-level value is not an object".to_owned(),
        )),
        Err(error) => Ok(GlobalOutcome::InvalidJson(error.to_string())),
    }
}

pub fn global(
    cwd: &Path,
    settings: &Settings,
    action: GlobalAction,
    ids: &mut impl Ids,
) -> Result<GlobalOutcome, WiringError> {
    let store = GlobalStore::new(&settings.dejafix_home);
    let project = project_name(cwd);
    match action {
        GlobalAction::Status => Ok(GlobalOutcome::Status {
            location: store.dir.display().to_string(),
            gotchas: store.load_gotchas()?,
            preferences: store.has_preferences(),
        }),
        GlobalAction::List { tag, library, json } => Ok(GlobalOutcome::List {
            gotchas: filtered(store.load_gotchas()?, tag.as_deref(), library.as_deref()),
            tag,
            library,
            json,
        }),
        GlobalAction::AddGotcha {
            library,
            text,
            keywords,
            versions,
        } => add_action(settings, &project, library, text, keywords, versions, ids),
        GlobalAction::Query {
            text,
            library,
            budget,
        } => Ok(GlobalOutcome::Reply(global_query(
            settings,
            text.as_deref().unwrap_or(""),
            library.as_deref(),
            budget,
        )?)),
        GlobalAction::Upgrade { export, apply } => upgrade_action(settings, cwd, export, apply),
        action @ GlobalAction::Edit { .. } => edit_action(settings, action),
        GlobalAction::Remove(id) => {
            let Some(id) = non_empty(id) else {
                return Ok(GlobalOutcome::Usage(Usage::Remove));
            };
            Ok(if store.remove(&id)? {
                GlobalOutcome::Removed(id)
            } else {
                GlobalOutcome::NotFound(id)
            })
        }
        GlobalAction::Export => Ok(GlobalOutcome::Exported(
            store.export(&iso_micros(&settings.clock))?,
        )),
        GlobalAction::Import(file) => match non_empty(file) {
            Some(file) => import(&store, cwd, &file),
            None => Ok(GlobalOutcome::Usage(Usage::Import)),
        },
        GlobalAction::Prune {
            older_than,
            confidence,
        } => Ok(GlobalOutcome::Pruned {
            removed: store.prune(
                settings.clock.now,
                &settings.clock.zone,
                older_than,
                confidence.as_deref(),
            )?,
            older_than,
        }),
        GlobalAction::Detect { json } => detect(&store, cwd, json),
        GlobalAction::Unknown(action) => Ok(GlobalOutcome::Unknown(action)),
    }
}

fn detect(store: &GlobalStore, cwd: &Path, json: bool) -> Result<GlobalOutcome, WiringError> {
    let stack = detect_stack(cwd);
    let gotchas = store.load_gotchas()?;
    let inherited_gotchas = relevant_entries(&stack, None, &gotchas).len();
    Ok(GlobalOutcome::Detected {
        name: project_name(cwd),
        stack,
        inherited_gotchas,
        json,
    })
}
