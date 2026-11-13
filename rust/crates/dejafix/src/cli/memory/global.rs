use std::io::{self, Write};

use dejafix_core::{GlobalAction, OsIds, Settings, global, global_report};

use crate::cli::args::GlobalArgs;
use crate::cli::types::Outcome;
use crate::config::Config;

pub fn action_of(args: GlobalArgs) -> GlobalAction {
    let GlobalArgs {
        action,
        text,
        tag,
        library,
        older_than,
        confidence,
        format,
    } = args;
    let json = format == "json";
    let action = match action.as_str() {
        "add"
            if library
                .as_deref()
                .is_some_and(|library| !library.is_empty()) =>
        {
            "add-gotcha"
        }
        other => other,
    };
    match action {
        "status" => GlobalAction::Status,
        "list" => GlobalAction::List { tag, library, json },
        "add" => GlobalAction::Add { text, tag },
        "add-gotcha" => GlobalAction::AddGotcha { library, text, tag },
        "remove" => GlobalAction::Remove(text),
        "export" => GlobalAction::Export,
        "import" => GlobalAction::Import(text),
        "prune" => GlobalAction::Prune {
            older_than,
            confidence,
        },
        "detect" => GlobalAction::Detect { json },
        other => GlobalAction::Unknown(other.to_owned()),
    }
}

pub fn run(
    args: GlobalArgs,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
) -> io::Result<Outcome> {
    match global(&config.cwd, settings, action_of(args), &mut OsIds) {
        Ok(outcome) => {
            out.write_all(global_report(&outcome, config.color).as_bytes())?;
            Ok(Outcome::Done)
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}
