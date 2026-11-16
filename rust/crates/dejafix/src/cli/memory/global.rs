use std::io::{self, Write};

use dejafix_core::{GlobalAction, GlobalOutcome, OsIds, Settings, Usage, global, global_report};

use crate::cli::USER_ERROR;
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
        keyword,
        versions,
        budget,
        export_file,
        apply_file,
        alias,
    } = args;
    let json = format == "json";
    match action.as_str() {
        "status" => GlobalAction::Status,
        "list" => GlobalAction::List { tag, library, json },
        "add" | "add-gotcha" => GlobalAction::AddGotcha {
            library,
            text,
            keywords: keyword,
            versions,
        },
        "query" => GlobalAction::Query {
            text,
            library,
            budget,
        },
        "edit" => GlobalAction::Edit {
            id: text,
            library,
            keywords: keyword,
            versions,
            aliases: alias,
        },
        "upgrade" => GlobalAction::Upgrade {
            export: export_file,
            apply: apply_file,
        },
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
    err: &mut impl Write,
) -> io::Result<Outcome> {
    match global(&config.cwd, settings, action_of(args), &mut OsIds) {
        Ok(
            outcome @ (GlobalOutcome::Usage(Usage::AddGotcha | Usage::Edit | Usage::Upgrade)
            | GlobalOutcome::EditRefused(_)
            | GlobalOutcome::GotchaRejected { .. }
            | GlobalOutcome::UpgradeExists(_)
            | GlobalOutcome::UpgradeMissing(_)
            | GlobalOutcome::UpgradeInvalid(_)),
        ) => {
            err.write_all(global_report(&outcome, false).as_bytes())?;
            Ok(Outcome::Exit(USER_ERROR))
        }
        Ok(outcome) => {
            out.write_all(global_report(&outcome, config.color).as_bytes())?;
            Ok(Outcome::Done)
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}
