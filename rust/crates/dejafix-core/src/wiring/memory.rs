use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use super::init::detect_and_record;
use super::types::{Project, Settings, WiringError};
use crate::adapters::fs::{
    FsResolver, GlobalStore, path_context, project_name, read_events_file, read_json_text,
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
    let stack = detect_and_record(cwd, settings);
    let store = GlobalStore::new(&settings.dejafix_home);
    let stored = store
        .load_patterns()
        .and_then(|patterns| store.load_gotchas().map(|gotchas| (patterns, gotchas)))
        .ok();
    let gotchas: Vec<&Entry> = stored
        .as_ref()
        .map_or_else(Vec::new, |(patterns, gotchas)| {
            relevant_entries(&stack, None, patterns, gotchas).gotchas
        });
    let name = project_name(cwd);
    Ok(render_brief(&BriefInput {
        name: &name,
        log: &log,
        stale: &stale,
        gotchas: &gotchas,
        now: settings.clock.now,
        color,
    }))
}

fn entry_of<const N: usize>(fields: [(&str, Value); N]) -> Entry {
    fields
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

fn tags_of(tag: Option<String>) -> Value {
    Value::Array(
        tag.filter(|tag| !tag.is_empty())
            .map(Value::String)
            .into_iter()
            .collect(),
    )
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.is_empty())
}

fn import(store: &GlobalStore, cwd: &Path, file: &str) -> Result<GlobalOutcome, WiringError> {
    let Some(text) = read_json_text(&cwd.join(file))? else {
        return Ok(GlobalOutcome::FileNotFound(file.to_owned()));
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(data)) => {
            let (patterns, gotchas) = store.import(&data)?;
            Ok(GlobalOutcome::Imported { patterns, gotchas })
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
        GlobalAction::Status => {
            let patterns = store.load_patterns()?;
            let gotchas = store.load_gotchas()?;
            Ok(GlobalOutcome::Status {
                location: store.dir.display().to_string(),
                patterns,
                gotchas,
                preferences: store.has_preferences(),
            })
        }
        GlobalAction::List { tag, library, json } => {
            let (patterns, gotchas) = filtered(
                store.load_patterns()?,
                store.load_gotchas()?,
                tag.as_deref(),
                library.as_deref(),
            );
            Ok(GlobalOutcome::List {
                patterns,
                gotchas,
                tag,
                library,
                json,
            })
        }
        GlobalAction::Add { text, tag } => {
            add_pattern(&store, &project, settings, non_empty(text), tag, ids)
        }
        GlobalAction::AddGotcha { library, text, tag } => add_gotcha(
            &store,
            &project,
            settings,
            (non_empty(library), non_empty(text)),
            tag,
            ids,
        ),
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
        GlobalAction::Detect { json } => detect(&store, cwd, settings, json),
        GlobalAction::Unknown(action) => Ok(GlobalOutcome::Unknown(action)),
    }
}

fn add_pattern(
    store: &GlobalStore,
    project: &str,
    settings: &Settings,
    text: Option<String>,
    tag: Option<String>,
    ids: &mut impl Ids,
) -> Result<GlobalOutcome, WiringError> {
    let Some(text) = text else {
        return Ok(GlobalOutcome::Usage(Usage::Add));
    };
    let id = ids.entry_id("pat");
    store.append_pattern(&entry_of([
        ("id", Value::String(id.clone())),
        ("pattern", Value::String(text.clone())),
        ("tags", tags_of(tag)),
        ("source_project", Value::String(project.to_owned())),
        ("confidence", Value::String("medium".to_owned())),
        ("times_encountered", Value::from(1)),
        ("created", Value::String(iso_micros(&settings.clock))),
    ]))?;
    Ok(GlobalOutcome::PatternAdded { id, text })
}

fn add_gotcha(
    store: &GlobalStore,
    project: &str,
    settings: &Settings,
    asked: (Option<String>, Option<String>),
    tag: Option<String>,
    ids: &mut impl Ids,
) -> Result<GlobalOutcome, WiringError> {
    let (Some(library), Some(text)) = asked else {
        return Ok(GlobalOutcome::Usage(Usage::AddGotcha));
    };
    let id = ids.entry_id("got");
    store.append_gotcha(&entry_of([
        ("id", Value::String(id.clone())),
        ("library", Value::String(library.to_lowercase())),
        ("gotcha", Value::String(text.clone())),
        ("tags", tags_of(tag)),
        ("source_project", Value::String(project.to_owned())),
        ("version_range", Value::Null),
        ("discovered", Value::String(iso_micros(&settings.clock))),
    ]))?;
    Ok(GlobalOutcome::GotchaAdded { id, library, text })
}

fn detect(
    store: &GlobalStore,
    cwd: &Path,
    settings: &Settings,
    json: bool,
) -> Result<GlobalOutcome, WiringError> {
    let stack = detect_and_record(cwd, settings);
    let patterns = store.load_patterns()?;
    let gotchas = store.load_gotchas()?;
    let relevant = relevant_entries(&stack, None, &patterns, &gotchas);
    let (inherited_patterns, inherited_gotchas) = (relevant.patterns.len(), relevant.gotchas.len());
    Ok(GlobalOutcome::Detected {
        name: project_name(cwd),
        stack,
        inherited_patterns,
        inherited_gotchas,
        json,
    })
}
