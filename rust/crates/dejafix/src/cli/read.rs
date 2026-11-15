use std::io::{self, Write};
use std::path::Path;

use dejafix_core::{
    MemoryFile, Project, SearchOutput, SearchQuery, Searched, WiringError, add_plan_idea,
    read_memory_file, regenerate, search_project, show,
};

use super::output::{printable_text, report_unreadable};

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
    err: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    let Searched { output, unreadable } = match search_project(project, query) {
        Ok(searched) => searched,
        Err(error) => return Ok(Err(error)),
    };
    match output {
        SearchOutput::Matches(lines) => {
            for line in lines {
                writeln!(out, "{line}")?;
            }
        }
        SearchOutput::Nothing(message) => writeln!(out, "{message}")?,
    }
    report_unreadable(unreadable, err).map(Ok)
}

pub fn rebuild(
    project: &Project,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    let report = match regenerate(project) {
        Ok(report) => report,
        Err(error) => return Ok(Err(error)),
    };
    writeln!(out, "Regenerated {}", report.path.display())?;
    if report.removed > 0 {
        writeln!(out, "Removed {} renamed issue file(s).", report.removed)?;
    }
    for line in &report.not_removed {
        writeln!(err, "{}", printable_text(line))?;
    }
    report_unreadable(report.unreadable, err).map(Ok)
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
