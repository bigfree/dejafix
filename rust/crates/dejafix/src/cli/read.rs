use std::io::{self, Write};
use std::path::Path;

use dejafix_core::{
    MemoryFile, Project, SearchOutput, SearchQuery, WiringError, add_plan_idea, read_memory_file,
    regenerate, search_project, show,
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

pub fn memory_file(
    cwd: &Path,
    file: MemoryFile,
    out: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    match read_memory_file(cwd, file) {
        Ok((_, text)) => writeln!(out, "{text}").map(Ok),
        Err(error) => Ok(Err(error)),
    }
}

pub fn plan(
    cwd: &Path,
    idea: Option<&str>,
    out: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    let Some(idea) = idea.filter(|idea| !idea.is_empty()) else {
        return memory_file(cwd, MemoryFile::Plan, out);
    };
    match add_plan_idea(cwd, idea) {
        Ok(path) => writeln!(out, "Added to Ideas in {}", path.display()).map(Ok),
        Err(error) => Ok(Err(error)),
    }
}
