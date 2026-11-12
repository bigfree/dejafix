mod args;
mod read;
mod write;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;
use dejafix_core::{Clock, Project, SearchQuery, Settings, StoreError, WiringError};

use crate::config::Config;
use args::{Cli, Commands};

const USER_ERROR: u8 = 1;
const IO_ERROR: u8 = 3;

pub fn run(
    arguments: impl IntoIterator<Item = OsString>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> ExitCode {
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
    match dispatch(cli.command, &config, out, err) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(error)) => fail(err, &error),
        Err(_) => ExitCode::from(IO_ERROR),
    }
}

fn report(err: &mut impl Write, message: &str, code: u8) -> ExitCode {
    let _ = writeln!(err, "{message}");
    ExitCode::from(code)
}

fn fail(err: &mut impl Write, error: &WiringError) -> ExitCode {
    match error {
        WiringError::Validation(_) | WiringError::Store(StoreError::NotFound { .. }) => {
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

fn settings(config: &Config) -> io::Result<Settings> {
    let dejafix_home = config.dejafix_home.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine the home directory",
        )
    })?;
    Ok(Settings {
        dejafix_home,
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
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Result<(), WiringError>> {
    let settings = match settings(config) {
        Ok(settings) => settings,
        Err(error) => {
            writeln!(err, "Error: {error}")?;
            return Err(error);
        }
    };
    let project = match Project::open(&config.cwd, settings) {
        Ok(project) => project,
        Err(error) => return Ok(Err(error)),
    };
    match command {
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
    }
}
