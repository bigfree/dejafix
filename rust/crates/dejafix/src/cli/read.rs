use std::io::{self, Write};

use dejafix_core::{
    Project, SearchOutput, SearchQuery, WiringError, regenerate, search_project, show,
};

pub fn show_summary(
    project: &Project,
    out: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    match show(project) {
        Ok(text) => writeln!(out, "{text}").map(Ok),
        Err(error) => Ok(Err(error)),
    }
}

pub fn search(
    project: &Project,
    query: &SearchQuery<'_>,
    out: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    match search_project(project, query) {
        Ok(SearchOutput::Matches(lines)) => {
            for line in lines {
                writeln!(out, "{line}")?;
            }
            Ok(Ok(()))
        }
        Ok(SearchOutput::Nothing(message)) => writeln!(out, "{message}").map(Ok),
        Err(error) => Ok(Err(error)),
    }
}

pub fn rebuild(
    project: &Project,
    out: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    match regenerate(project) {
        Ok(path) => writeln!(out, "Regenerated {}", path.display()).map(Ok),
        Err(error) => Ok(Err(error)),
    }
}
