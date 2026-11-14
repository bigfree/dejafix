use std::path::Path;

use super::registry::load_registry;
use super::types::{GotchaChanges, Settings, WiringError};
use crate::adapters::fs::{GlobalStore, GotchaWrite, read_json_text};
use crate::core::global::{
    AddOutcome, GlobalAction, GlobalIndex, GlobalOutcome, GotchaInput, GotchaV2, Libraries,
    StoredGotcha, Usage, apply_upgrade, check_gotcha, export_notes, global_reply, is_package_name,
    legacy_input, line, normalize_input, parse_drafts, parse_store, stored_line,
    touches_version_one, upgrade_drafts,
};
use crate::core::write::Ids;

pub(super) fn today(settings: &Settings) -> String {
    settings
        .clock
        .now
        .to_zoned(settings.clock.zone.clone())
        .date()
        .to_string()
}

pub(super) fn project_names(settings: &Settings) -> Vec<String> {
    load_registry(settings)
        .map(|registry| {
            registry
                .projects
                .into_iter()
                .flat_map(|project| std::iter::once(project.id).chain(project.alias))
                .collect()
        })
        .unwrap_or_default()
}

pub fn add_gotcha(
    settings: &Settings,
    input: GotchaInput,
    source_project: Option<&str>,
    ids: &mut impl Ids,
) -> Result<AddOutcome, WiringError> {
    let store = GlobalStore::new(&settings.dejafix_home);
    let mut libraries = store.libraries()?;
    let input = normalize_input(input, &libraries);
    let names = project_names(settings);
    let projects: Vec<&str> = names.iter().map(String::as_str).collect();
    let discovered = today(settings);
    let outcome = store.edit_gotchas(|entries| {
        let records = parse_store(entries);
        let report = check_gotcha(&input, &libraries, &projects, &records, None);
        if !report.failures.is_empty() {
            return (GotchaWrite::Keep, AddOutcome::Rejected(report));
        }
        let record = GotchaV2 {
            id: ids.entry_id("got"),
            library: input.library.clone(),
            keywords: input.keywords.clone(),
            versions: input.versions.clone(),
            gotcha: input.gotcha.clone(),
            source_project: source_project
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            discovered: Some(discovered.clone()),
            ..GotchaV2::default()
        };
        (
            GotchaWrite::Append(vec![line(&record)]),
            AddOutcome::Added {
                id: record.id,
                library: record.library,
                warnings: report.warnings,
            },
        )
    })?;
    if matches!(outcome, AddOutcome::Added { .. }) && libraries.add_library(&input.library) {
        store.save_libraries(&libraries)?;
    }
    Ok(outcome)
}

pub fn global_query(
    settings: &Settings,
    query: &str,
    library: Option<&str>,
    budget: u32,
) -> Result<String, WiringError> {
    let store = GlobalStore::new(&settings.dejafix_home);
    let records = parse_store(store.load_gotchas()?);
    let index = GlobalIndex::build(records, store.libraries()?);
    Ok(global_reply(&index, query, library, budget))
}

fn export_drafts(
    settings: &Settings,
    cwd: &Path,
    file: &str,
) -> Result<GlobalOutcome, WiringError> {
    let store = GlobalStore::new(&settings.dejafix_home);
    let libraries = store.libraries()?;
    let names = project_names(settings);
    let projects: Vec<&str> = names.iter().map(String::as_str).collect();
    let records = parse_store(store.load_gotchas()?);
    let lines = upgrade_drafts(&records, |entry| {
        let input = normalize_input(legacy_input(entry), &libraries);
        let id = entry.get("id").and_then(serde_json::Value::as_str);
        export_notes(&check_gotcha(&input, &libraries, &projects, &records, id))
    });
    let text = lines.iter().fold(String::new(), |mut text, line| {
        text.push_str(line);
        text.push('\n');
        text
    });
    Ok(if GlobalStore::create_new(&cwd.join(file), &text)? {
        GlobalOutcome::UpgradeExported {
            file: file.to_owned(),
            drafts: lines.len().saturating_sub(1),
        }
    } else {
        GlobalOutcome::UpgradeExists(file.to_owned())
    })
}

fn apply_drafts(settings: &Settings, cwd: &Path, file: &str) -> Result<GlobalOutcome, WiringError> {
    let Some(text) = read_json_text(&cwd.join(file))? else {
        return Ok(GlobalOutcome::UpgradeMissing(file.to_owned()));
    };
    let drafts = match parse_drafts(&text) {
        Ok(drafts) => drafts,
        Err(errors) => {
            return Ok(GlobalOutcome::UpgradeInvalid(
                errors.iter().map(ToString::to_string).collect(),
            ));
        }
    };
    let store = GlobalStore::new(&settings.dejafix_home);
    let mut libraries = store.libraries()?;
    if touches_version_one(&parse_store(store.load_gotchas()?), &drafts) {
        store.backup_v1()?;
    }
    let names = project_names(settings);
    let projects: Vec<&str> = names.iter().map(String::as_str).collect();
    let today = today(settings);
    let mut outcome = store.edit_gotchas(|entries| {
        let records = parse_store(entries);
        let mut outcome = apply_upgrade(&records, &drafts, &today, &libraries, |id, input| {
            check_gotcha(input, &libraries, &projects, &records, Some(id))
        });
        if outcome.replaced + outcome.dropped == 0 {
            return (GotchaWrite::Keep, outcome);
        }
        let lines = std::mem::take(&mut outcome.lines);
        (GotchaWrite::Replace(lines), outcome)
    })?;
    let mut added = false;
    for library in &outcome.libraries {
        added |= libraries.add_library(library);
    }
    if added {
        store.save_libraries(&libraries)?;
    }
    outcome.kept.sort_unstable();
    Ok(GlobalOutcome::UpgradeApplied {
        replaced: outcome.replaced,
        dropped: outcome.dropped,
        kept: outcome.kept,
    })
}

pub(super) fn upgrade_action(
    settings: &Settings,
    cwd: &Path,
    export: Option<String>,
    apply: Option<String>,
) -> Result<GlobalOutcome, WiringError> {
    match (non_empty(export), non_empty(apply)) {
        (Some(file), None) => export_drafts(settings, cwd, &file),
        (None, Some(file)) => apply_drafts(settings, cwd, &file),
        _ => Ok(GlobalOutcome::Usage(Usage::Upgrade)),
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.is_empty())
}

pub(super) fn gotcha_action(
    settings: &Settings,
    project: &str,
    action: GlobalAction,
    ids: &mut impl Ids,
) -> Result<GlobalOutcome, WiringError> {
    match action {
        GlobalAction::AddGotcha {
            library,
            text,
            keywords,
            versions,
        } => {
            let (Some(library), Some(gotcha)) = (non_empty(library), non_empty(text)) else {
                return Ok(GlobalOutcome::Usage(Usage::AddGotcha));
            };
            if keywords.is_empty() {
                return Ok(GlobalOutcome::Usage(Usage::AddGotcha));
            }
            let input = GotchaInput {
                library,
                keywords,
                gotcha: gotcha.clone(),
                versions,
            };
            Ok(match add_gotcha(settings, input, Some(project), ids)? {
                AddOutcome::Added {
                    id,
                    library,
                    warnings,
                } => GlobalOutcome::GotchaAdded {
                    id,
                    library,
                    text: gotcha,
                    warnings: warnings.iter().map(ToString::to_string).collect(),
                },
                AddOutcome::Rejected(report) => GlobalOutcome::GotchaRejected {
                    failures: report.failures.iter().map(ToString::to_string).collect(),
                    warnings: report.warnings.iter().map(ToString::to_string).collect(),
                },
            })
        }
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
        _ => Ok(GlobalOutcome::Usage(Usage::AddGotcha)),
    }
}

fn add_aliases(libraries: &mut Libraries, aliases: &[String]) -> Result<Vec<String>, Vec<String>> {
    let mut added = Vec::new();
    let mut refused = Vec::new();
    for asked in aliases {
        let Some((library, alias)) = asked.split_once('=') else {
            refused.push(format!("'{asked}' is not <library>=<alias>"));
            continue;
        };
        let library = libraries.canonical(library);
        let alias = alias.trim().to_lowercase();
        if !is_package_name(&library) || !is_package_name(&alias) {
            refused.push(format!("'{asked}' does not name two packages"));
            continue;
        }
        let owner = libraries.canonical(&alias);
        if owner != alias && owner != library {
            refused.push(format!("'{alias}' is already an alias of {owner}"));
            continue;
        }
        if libraries.has_library(&alias) {
            refused.push(format!("'{alias}' is a library, not an alias"));
            continue;
        }
        if libraries.add_alias(&library, &alias) {
            added.push(format!("{library}={alias}"));
        }
    }
    if refused.is_empty() {
        Ok(added)
    } else {
        Err(refused)
    }
}

fn edited(
    records: &[StoredGotcha],
    id: &str,
    changes: &GotchaChanges,
    libraries: &Libraries,
    projects: &[&str],
    today: &str,
) -> Result<(Vec<String>, String, Vec<String>), Vec<String>> {
    let Some(position) = records.iter().position(|record| record.id() == id) else {
        return Err(vec![format!("no record has the id {id}")]);
    };
    let Some(StoredGotcha::V2(record)) = records.get(position) else {
        return Err(vec![format!(
            "{id} is not upgraded yet; run `djf global upgrade` first"
        )]);
    };
    let input = normalize_input(
        GotchaInput {
            library: changes
                .library
                .clone()
                .unwrap_or_else(|| record.library.clone()),
            keywords: if changes.keywords.is_empty() {
                record.keywords.clone()
            } else {
                changes.keywords.clone()
            },
            gotcha: record.gotcha.clone(),
            versions: changes.versions.clone().or_else(|| record.versions.clone()),
        },
        libraries,
    );
    let report = check_gotcha(&input, libraries, projects, records, Some(id));
    if !report.failures.is_empty() {
        return Err(report.failures.iter().map(ToString::to_string).collect());
    }
    let revised = GotchaV2 {
        library: input.library.clone(),
        keywords: input.keywords,
        versions: input.versions,
        reviewed: Some(today.to_owned()),
        ..record.clone()
    };
    let lines = records
        .iter()
        .enumerate()
        .map(|(index, stored)| {
            if index == position {
                line(&revised)
            } else {
                stored_line(stored)
            }
        })
        .collect();
    Ok((
        lines,
        input.library,
        report.warnings.iter().map(ToString::to_string).collect(),
    ))
}

fn edit_gotcha(
    settings: &Settings,
    id: Option<String>,
    changes: &GotchaChanges,
    aliases: &[String],
) -> Result<GlobalOutcome, WiringError> {
    let id = non_empty(id);
    if id.is_none() && aliases.is_empty() {
        return Ok(GlobalOutcome::Usage(Usage::Edit));
    }
    let store = GlobalStore::new(&settings.dejafix_home);
    let mut libraries = store.libraries()?;
    let added = match add_aliases(&mut libraries, aliases) {
        Ok(added) => added,
        Err(refused) => return Ok(GlobalOutcome::EditRefused(refused)),
    };
    let mut warnings = Vec::new();
    let mut new_library = false;
    if let Some(id) = &id {
        let names = project_names(settings);
        let projects: Vec<&str> = names.iter().map(String::as_str).collect();
        let today = today(settings);
        let result = store.edit_gotchas(|entries| {
            match edited(
                &parse_store(entries),
                id,
                changes,
                &libraries,
                &projects,
                &today,
            ) {
                Ok((lines, library, warnings)) => {
                    (GotchaWrite::Replace(lines), Ok((library, warnings)))
                }
                Err(refused) => (GotchaWrite::Keep, Err(refused)),
            }
        })?;
        match result {
            Ok((library, found)) => {
                warnings = found;
                new_library = libraries.add_library(&library);
            }
            Err(refused) => return Ok(GlobalOutcome::EditRefused(refused)),
        }
    }
    if new_library || !added.is_empty() {
        store.save_libraries(&libraries)?;
    }
    Ok(GlobalOutcome::Edited {
        id,
        aliases: added,
        warnings,
    })
}

pub(super) fn edit_action(
    settings: &Settings,
    action: GlobalAction,
) -> Result<GlobalOutcome, WiringError> {
    let GlobalAction::Edit {
        id,
        library,
        keywords,
        versions,
        aliases,
    } = action
    else {
        return Ok(GlobalOutcome::Usage(Usage::Edit));
    };
    let changes = GotchaChanges {
        library: non_empty(library),
        keywords,
        versions,
    };
    edit_gotcha(settings, id, &changes, &aliases)
}
