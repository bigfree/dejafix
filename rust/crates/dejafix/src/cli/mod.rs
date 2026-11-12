mod args;
mod doctor;
mod init;
mod notice;
mod output;
mod project;
mod read;
mod types;
mod write;

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;
use dejafix_core::{Clock, MemoryFile, Project, SearchQuery, Settings, StoreError, WiringError};

use crate::config::Config;
use args::{Cli, Commands};
use types::Outcome;

pub(super) const USER_ERROR: u8 = 1;
const IO_ERROR: u8 = 3;

pub fn run(
    arguments: impl IntoIterator<Item = OsString>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            return report(
                err,
                &format!("Error: cannot read the environment: {error}"),
                IO_ERROR,
            );
        }
    };
    let settings = settings(&config);
    if let Some(settings) = &settings {
        let _ = notice::print(settings, env!("CARGO_PKG_VERSION"), out, config.color);
    }
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let rendered = error.render().to_string();
            let written = if error.use_stderr() {
                err.write_all(rendered.as_bytes())
            } else {
                out.write_all(rendered.as_bytes())
            };
            return match (written, u8::try_from(error.exit_code())) {
                (Ok(()), Ok(code)) => ExitCode::from(code),
                _ => ExitCode::from(IO_ERROR),
            };
        }
    };
    let cwd = &config.cwd;
    let outcome = match cli.command {
        Commands::Instructions => {
            read::memory_file(cwd, MemoryFile::Instructions, out).map(Outcome::from)
        }
        Commands::Map => read::memory_file(cwd, MemoryFile::ProjectMap, out).map(Outcome::from),
        Commands::Plan { add } => read::plan(cwd, add.as_deref(), out).map(Outcome::from),
        command => {
            let Some(settings) = settings else {
                return report(err, "Error: cannot determine the home directory", IO_ERROR);
            };
            dispatch(command, &config, &settings, out, err)
        }
    };
    match outcome {
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Exit(code)) => ExitCode::from(code),
        Ok(Outcome::Failed(error)) => fail(err, &error),
        Err(_) => ExitCode::from(IO_ERROR),
    }
}

fn report(err: &mut impl Write, message: &str, code: u8) -> ExitCode {
    let _ = writeln!(err, "{message}");
    ExitCode::from(code)
}

fn fail(err: &mut impl Write, error: &WiringError) -> ExitCode {
    match error {
        WiringError::Validation(_)
        | WiringError::Registry(_)
        | WiringError::Store(StoreError::NotFound { .. }) => {
            report(err, &format!("Error: {error}"), USER_ERROR)
        }
        WiringError::Store(store) => {
            let detail = std::error::Error::source(store)
                .map(|source| format!(": {source}"))
                .unwrap_or_default();
            report(err, &format!("Error: {store}{detail}"), IO_ERROR)
        }
    }
}

fn settings(config: &Config) -> Option<Settings> {
    Some(Settings {
        dejafix_home: config.dejafix_home.clone()?,
        redact: config.redact,
        clock: Clock {
            now: jiff::Timestamp::now(),
            zone: config.zone.clone(),
        },
    })
}

fn dispatch(
    command: Commands,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    let cwd = &config.cwd;
    match command {
        Commands::Init(args) => return init::run(args, config, settings, out),
        Commands::Project { action } => return project::run(action, config, settings, out),
        Commands::Doctor { fix, depth, path } => {
            let Some(paths) = config.client_paths() else {
                writeln!(err, "Error: cannot determine the home directory")?;
                return Ok(Outcome::Exit(IO_ERROR));
            };
            return doctor::run(fix, depth, path, &paths, config, settings, out);
        }
        _ => {}
    }
    let project = match Project::open(cwd, settings.clone()) {
        Ok(project) => project,
        Err(error) => return Ok(Outcome::Failed(error)),
    };
    let result = match command {
        Commands::Show => read::show_summary(&project, out),
        Commands::Search {
            query,
            regex,
            failed_only,
        } => read::search(
            &project,
            &SearchQuery {
                text: &query,
                regex,
                failed_only,
            },
            out,
        ),
        Commands::Regenerate => read::rebuild(&project, out),
        other => match write::command(other) {
            Some(command) => write::run(&project, command, out, err),
            None => Ok(Ok(())),
        },
    };
    result.map(Outcome::from)
}
