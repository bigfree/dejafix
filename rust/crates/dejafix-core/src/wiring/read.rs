use super::types::{Project, SearchOutput, WiringError};
use crate::adapters::fs::{FsResolver, read_events_file, read_text_inside};
use crate::core::log::EventLog;
use crate::core::search::{SearchQuery, match_line, no_match_message, search};

pub fn show(project: &Project) -> Result<String, WiringError> {
    Ok(read_text_inside(
        &project.summary_path(),
        &project.memory.root,
    )?)
}

pub fn search_project(
    project: &Project,
    query: &SearchQuery<'_>,
) -> Result<SearchOutput, WiringError> {
    let bytes = read_events_file(&project.memory.dir)?;
    let log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    let found = search(&log, query);
    if found.is_empty() {
        return Ok(SearchOutput::Nothing(no_match_message(query)));
    }
    Ok(SearchOutput::Matches(
        found
            .into_iter()
            .map(|event| match_line(event, log.is_retired(&event.id)))
            .collect(),
    ))
}
