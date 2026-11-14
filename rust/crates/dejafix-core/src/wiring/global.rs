use super::registry::load_registry;
use super::types::{Settings, WiringError};
use crate::adapters::fs::{GlobalStore, GotchaWrite};
use crate::core::global::{
    AddOutcome, GlobalAction, GlobalIndex, GlobalOutcome, GotchaInput, GotchaV2, Usage,
    check_gotcha, global_reply, line, normalize_input, parse_store,
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
        other => Ok(GlobalOutcome::Unknown(format!("{other:?}"))),
    }
}
