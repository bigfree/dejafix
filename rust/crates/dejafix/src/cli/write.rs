use std::io::{self, Write};

use dejafix_core::{Command, OsIds, Outcome, Project, WiringError, WriteOutcome, record};

use super::args::Commands;

pub fn command(arguments: Commands) -> Option<Command> {
    Some(match arguments {
        Commands::Log { text, at } => Command::Log { text, location: at },
        Commands::Attempt {
            text,
            worked,
            failed,
            partial,
            at,
            issue,
            auto_issue,
        } => Command::Attempt {
            text,
            outcomes: [
                (worked, Outcome::Worked),
                (failed, Outcome::Failed),
                (partial, Outcome::Partial),
            ]
            .into_iter()
            .filter_map(|(set, outcome)| set.then_some(outcome))
            .collect(),
            location: at,
            issue,
            auto_issue,
        },
        Commands::Fix { text, at, issue } => Command::Fix {
            text,
            location: at,
            issue,
        },
        Commands::Decision {
            text,
            at,
            supersedes,
        } => Command::Decision {
            text,
            location: at,
            supersedes,
        },
        Commands::Note { text, at } => Command::Note { text, location: at },
        Commands::Show | Commands::Search { .. } | Commands::Regenerate => return None,
    })
}

pub fn run(
    project: &Project,
    command: Command,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<Result<(), WiringError>, io::Error> {
    let recorded = match record(project, command, &mut OsIds) {
        Ok(recorded) => recorded,
        Err(error) => return Ok(Err(error)),
    };
    for notice in &recorded.notices {
        writeln!(err, "{notice}")?;
    }
    match recorded.outcome {
        WriteOutcome::Logged(issue) => writeln!(out, "Logged issue #{}", issue.as_str())?,
        WriteOutcome::Attempted { outcome, issue } => writeln!(
            out,
            "Recorded {} attempt on #{}",
            outcome.as_str(),
            issue.as_str()
        )?,
        WriteOutcome::Fixed(issue) => writeln!(out, "Fixed issue #{}", issue.as_str())?,
        WriteOutcome::Decided {
            superseded: Some((id, summary)),
        } => writeln!(
            out,
            "Recorded decision (supersedes {}: \"{summary}\")",
            id.as_str()
        )?,
        WriteOutcome::Decided { superseded: None } => writeln!(out, "Recorded decision")?,
        WriteOutcome::Noted => writeln!(out, "Recorded note")?,
    }
    Ok(recorded.incomplete.map_or(Ok(()), Err))
}
