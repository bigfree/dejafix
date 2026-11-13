use std::io::{self, Write};

use dejafix_core::{
    CaptureOutcome, EventKind, HookKind, HooksAction, HooksOutcome, HooksReport, OsIds,
    PrecheckOutcome, PrecheckRequest, Settings, auto_capture, hooks_action, precheck,
    render_warnings, resolve_djf_binary,
};

use super::USER_ERROR;
use super::args::PrecheckArgs;
use super::types::Outcome;
use crate::config::Config;

const RESET: &str = "\x1b[0m";

pub fn djf_binary(config: &Config) -> String {
    resolve_djf_binary(
        config.exe_dir.as_deref(),
        config.path_var.as_deref(),
        cfg!(windows),
    )
}

fn names(kinds: &[HookKind]) -> String {
    kinds
        .iter()
        .map(|kind| kind.file_name())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn print_report(
    report: &HooksReport,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<()> {
    for kind in &report.skipped {
        writeln!(
            err,
            "Warning: .git/hooks/{} has a dejafix start marker but no end marker, so it was left alone. Fix or delete that block, then re-run `djf hooks install`.",
            kind.file_name()
        )?;
    }
    for kind in &report.unreadable {
        writeln!(
            err,
            "Warning: .git/hooks/{} could not be read as text, so it was left alone.",
            kind.file_name()
        )?;
    }
    if !report.installed.is_empty() {
        writeln!(
            out,
            "dejafix git hooks installed: {}\n  Auto-captures: commits, reverts, merges, fixes, features, breaking changes.\n  Pre-commit: warns about repeating failed approaches and high-churn files.",
            names(&report.installed)
        )?;
    }
    if !report.refreshed.is_empty() {
        writeln!(
            out,
            "dejafix git hooks refreshed: {}",
            names(&report.refreshed)
        )?;
    }
    if report.installed.is_empty()
        && report.refreshed.is_empty()
        && report.skipped.is_empty()
        && report.unreadable.is_empty()
    {
        writeln!(out, "dejafix git hooks already installed.")?;
    }
    Ok(())
}

pub fn run_hooks(
    action: String,
    config: &Config,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    let action = match action.as_str() {
        "install" => HooksAction::Install {
            djf: djf_binary(config),
        },
        "uninstall" => HooksAction::Uninstall,
        _ => HooksAction::Unknown(action),
    };
    match hooks_action(&config.cwd, action) {
        Ok(HooksOutcome::NoHooksDir) => writeln!(
            err,
            "Error: .git/hooks directory not found. Is this a git repository?"
        )?,
        Ok(HooksOutcome::Installed(report)) => print_report(&report, out, err)?,
        Ok(HooksOutcome::Removed(removed)) if removed.is_empty() => {
            writeln!(out, "No dejafix hooks found to remove.")?;
        }
        Ok(HooksOutcome::Removed(removed)) => {
            writeln!(out, "dejafix hooks removed from: {}", names(&removed))?;
        }
        Ok(HooksOutcome::Unknown(action)) => writeln!(out, "Unknown action: {action}")?,
        Err(error) => return Ok(Outcome::Failed(error.into())),
    }
    Ok(Outcome::Done)
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("{code}{text}{RESET}")
    } else {
        text.to_owned()
    }
}

pub fn run_capture(
    trigger: &str,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    match auto_capture(&config.cwd, settings, trigger, &mut OsIds) {
        CaptureOutcome::Skipped => {}
        CaptureOutcome::Captured {
            kind,
            summary,
            notices,
        } => {
            for notice in notices {
                writeln!(err, "{notice}")?;
            }
            let code = match kind {
                EventKind::Attempt => "\x1b[0;33m",
                EventKind::Fix => "\x1b[0;32m",
                EventKind::Decision => "\x1b[0;31m",
                EventKind::Note => "\x1b[0;36m",
                EventKind::Issue | EventKind::Hypothesis => "\x1b[0;37m",
            };
            let line = format!("[dejafix] Auto-captured: {summary}");
            writeln!(out, "{}", paint(&line, code, config.color))?;
        }
        CaptureOutcome::Merged {
            first_line,
            notices,
        } => {
            for notice in notices {
                writeln!(err, "{notice}")?;
            }
            let line = format!("[dejafix] Auto-captured merge: {first_line}");
            writeln!(out, "{}", paint(&line, "\x1b[0;35m", config.color))?;
        }
    }
    Ok(Outcome::Done)
}

pub fn run_precheck(
    args: PrecheckArgs,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    let request = PrecheckRequest {
        level: args.level,
        working: args.working,
        files: args.files,
        quiet: args.quiet,
        snooze: args.snooze,
        unsnooze: args.unsnooze,
    };
    let outcome = match precheck(&config.cwd, settings, &request, &mut OsIds) {
        Ok(outcome) => outcome,
        Err(error) => return Ok(Outcome::Failed(error)),
    };
    let color = config.color;
    match outcome {
        PrecheckOutcome::NoProject | PrecheckOutcome::Unreadable => {}
        PrecheckOutcome::Unsnoozed(true) => {
            writeln!(out, "dejafix: precheck warnings re-enabled.")?;
        }
        PrecheckOutcome::Unsnoozed(false) => writeln!(out, "dejafix: no active snooze.")?,
        PrecheckOutcome::Snoozed { expiry, notices } => {
            for notice in notices {
                writeln!(err, "{notice}")?;
            }
            let until = jiff::tz::TimeZone::UTC
                .to_datetime(expiry)
                .strftime("%H:%M UTC");
            writeln!(
                out,
                "dejafix: precheck warnings snoozed until {until} (logged to memory). Re-enable early with `djf precheck --unsnooze`."
            )?;
        }
        PrecheckOutcome::BadSnooze(error) => {
            writeln!(err, "Error: {error}")?;
            return Ok(Outcome::Exit(USER_ERROR));
        }
        PrecheckOutcome::Snoozing(left) => {
            let line = format!(
                "dejafix: warnings snoozed ({left} left) — `djf precheck --unsnooze` to re-enable."
            );
            writeln!(out, "{}", paint(&line, "\x1b[2m", color))?;
        }
        PrecheckOutcome::NoFiles => {
            if !request.quiet {
                writeln!(out, "dejafix: No files to check.")?;
            }
        }
        PrecheckOutcome::Clean => {
            if !request.quiet {
                writeln!(
                    out,
                    "{} no warnings — looking good!",
                    paint("dejafix:", "\x1b[32m", color)
                )?;
            }
        }
        PrecheckOutcome::Warnings { warnings, blocking } => {
            if let Some(text) = render_warnings(&warnings, &request.level, color) {
                out.write_all(text.as_bytes())?;
            }
            if blocking && request.level == "block" {
                return Ok(Outcome::Exit(USER_ERROR));
            }
        }
    }
    Ok(Outcome::Done)
}
