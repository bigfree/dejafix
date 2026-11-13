use std::io::{self, Write};
use std::ops::ControlFlow;

use dejafix_core::{MigrateEvent, MigrateOptions, ProjectState, Settings, migrate};

use crate::cli::hooks::print_report;
use crate::cli::init::print_bridge;
use crate::cli::types::Outcome;

const APPLY_BANNER: &str = "Close running agent sessions (Claude Code, Codex, Cursor, Claude Desktop) first: a projectmem server still running writes to the old folders.\n";
const DRY_BANNER: &str = "Dry run — nothing is changed. Re-run with --apply to migrate.\n";
const LEFTOVER_HINT: &str = "  MCP server: `djf-mcp` or `python -m dejafix.mcp_server` (`djf init` prints a block to copy). Edit hook: `djf-edit-hook`, log variable DJF_EDIT_HOOK_LOG. Variables PROJECTMEM_* become DEJAFIX_*.";
const DONE: &str = "\nDone. Commit the renamed .dejafix/ folder in each project, and uninstall projectmem once your clients point at dejafix.";

fn write_project(label: &str, state: &ProjectState, out: &mut impl Write) -> io::Result<()> {
    match state {
        ProjectState::Symlink => writeln!(
            out,
            "{label}: the memory folder is a symlink, migrate it by hand."
        ),
        ProjectState::BothExist => writeln!(
            out,
            "{label}: both .projectmem/ and .dejafix/ exist, merge them by hand."
        ),
        ProjectState::NoLegacy => writeln!(out, "{label}: no .projectmem/ found, skipped."),
        ProjectState::Watcher(pid) => writeln!(
            out,
            "{label}: the old watcher (pid {pid}) is still running. Stop it with `pjm watch --stop` in that project, then re-run."
        ),
        ProjectState::Migrated => writeln!(out, "{label}: already migrated."),
        ProjectState::Steps(steps) => {
            writeln!(out, "{label}:")?;
            for step in steps {
                writeln!(out, "  {step}")?;
            }
            Ok(())
        }
    }
}

pub(super) fn write_event(
    event: &MigrateEvent,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<()> {
    match event {
        MigrateEvent::Banner { apply } => {
            writeln!(out, "{}", if *apply { APPLY_BANNER } else { DRY_BANNER })
        }
        MigrateEvent::HomeMissing(home) => {
            writeln!(out, "Home: {} not found, nothing to move.", home.display())
        }
        MigrateEvent::Home { from, to } => {
            writeln!(out, "Home: {} -> {}", from.display(), to.display())
        }
        MigrateEvent::HomeSkip { name, dest } => writeln!(
            out,
            "  skip {name}: {} already has data, merge it by hand",
            dest.display()
        ),
        MigrateEvent::HomeMove(name) => writeln!(out, "  move {name}"),
        MigrateEvent::Project { label, state } => write_project(label, state, out),
        MigrateEvent::Bridge(file, outcome) => print_bridge(file, *outcome, out),
        MigrateEvent::Hooks(report) => print_report(report, out, err),
        MigrateEvent::Leftovers { configs, env } => {
            writeln!(
                out,
                "\nStill pointing at projectmem — update these by hand:"
            )?;
            for (client, path) in configs {
                writeln!(out, "  {client}: {}", path.display())?;
            }
            for name in env {
                writeln!(out, "  environment variable {name}")?;
            }
            writeln!(out, "{LEFTOVER_HINT}")
        }
        MigrateEvent::Done => writeln!(out, "{DONE}"),
    }
}

pub fn run(
    options: &MigrateOptions,
    settings: &Settings,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    let mut written = Ok(());
    let result = migrate(settings, options, &mut |event| {
        written = write_event(&event, out, err);
        if written.is_ok() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    });
    written?;
    Ok(Outcome::from(result))
}
