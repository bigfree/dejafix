use super::types::{Project, SearchOutput, Searched, WiringError};
use crate::adapters::fs::{FsResolver, read_events_file, read_text_inside};
use crate::core::log::EventLog;
use crate::core::search::{
    SearchMode, SearchQuery, event_index, match_line, no_match_message, ranked, search,
};

pub fn show(project: &Project) -> Result<String, WiringError> {
    Ok(read_text_inside(
        &project.summary_path(),
        &project.memory.root,
    )?)
}

pub fn search_project(project: &Project, query: &SearchQuery<'_>) -> Result<Searched, WiringError> {
    let bytes = read_events_file(&project.memory.dir)?;
    let log = EventLog::parse(
        &bytes,
        &project.settings.clock,
        &project.context,
        &FsResolver,
    );
    let found = if query.mode == SearchMode::Ranked {
        ranked(&log, &event_index(&log), query)
    } else {
        search(&log, query)
    };
    let output = if found.is_empty() {
        SearchOutput::Nothing(no_match_message(query))
    } else {
        SearchOutput::Matches(
            found
                .into_iter()
                .map(|event| match_line(event, log.is_retired(&event.id)))
                .collect(),
        )
    };
    Ok(Searched {
        output,
        unreadable: log.skipped().len(),
    })
}
