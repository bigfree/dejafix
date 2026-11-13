use std::io::{self, Write};
use std::path::PathBuf;

use dejafix_core::{
    ClientPaths, DoctorOptions, DoctorReport, FoundProject, ScanReport, Settings, UpdateStatus,
    doctor, scan_projects, set_auto_check, update_status,
};

use super::types::DoctorRun;
use crate::cli::USER_ERROR;
use crate::cli::output::{FAIL, OK, WARN, secho};
use crate::cli::types::{Outcome, Style};
use crate::config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const LISTED: usize = 10;
const CORRUPT_LISTED: usize = 5;

fn depth(value: i64) -> usize {
    usize::try_from(value).unwrap_or(0)
}

fn write_missing(
    report: &DoctorReport,
    fix: bool,
    out: &mut impl Write,
    color: bool,
) -> io::Result<usize> {
    if report.missing.is_empty() {
        secho(
            out,
            &format!(
                "  {OK} All {} project(s) found are registered",
                report.found
            ),
            Style::Green,
            color,
        )?;
        return Ok(0);
    }
    secho(
        out,
        &format!(
            "  {WARN} {} project(s) have memory but are not registered",
            report.missing.len()
        ),
        Style::Yellow,
        color,
    )?;
    for project in report.missing.iter().take(LISTED) {
        writeln!(out, "      {}", project.display())?;
    }
    if report.missing.len() > LISTED {
        writeln!(out, "      … and {} more", report.missing.len() - LISTED)?;
    }
    if fix {
        for (project, error) in &report.register_failures {
            secho(
                out,
                &format!("      {FAIL} {}: {error}", project.display()),
                Style::Red,
                color,
            )?;
        }
        secho(
            out,
            &format!(
                "  {OK} Registered {}",
                report
                    .missing
                    .len()
                    .saturating_sub(report.register_failures.len())
            ),
            Style::Green,
            color,
        )?;
    }
    Ok(1)
}

fn write_stale(
    report: &DoctorReport,
    fix: bool,
    out: &mut impl Write,
    color: bool,
) -> io::Result<usize> {
    if report.stale.is_empty() {
        secho(
            out,
            &format!("\n  {OK} Every registered project still has its memory"),
            Style::Green,
            color,
        )?;
        return Ok(0);
    }
    secho(
        out,
        &format!(
            "\n  {WARN} {} registered project(s) no longer have memory",
            report.stale.len()
        ),
        Style::Yellow,
        color,
    )?;
    for record in &report.stale {
        writeln!(out, "      {}  {}", record.name(), record.path.display())?;
    }
    if fix {
        match &report.prune_failure {
            Some(error) => secho(out, &format!("  {FAIL} {error}"), Style::Red, color)?,
            None => secho(
                out,
                &format!(
                    "  {OK} Removed {} from the registry (repos untouched)",
                    report.stale.len()
                ),
                Style::Green,
                color,
            )?,
        }
    }
    Ok(1)
}

fn write_configs(report: &DoctorReport, out: &mut impl Write, color: bool) -> io::Result<usize> {
    let mut problems = 0;
    if !report.unreadable.is_empty() {
        problems += 1;
        secho(
            out,
            &format!("\n  {WARN} Could not read some client config(s) — not checked"),
            Style::Yellow,
            color,
        )?;
        for config in &report.unreadable {
            writeln!(out, "      {}  {}", config.client, config.path.display())?;
        }
        writeln!(
            out,
            "      A config that cannot be read is not a config that is fine."
        )?;
    }
    if report.pinned.is_empty() {
        secho(
            out,
            &format!("\n  {OK} No MCP client config is pinned to a single repo"),
            Style::Green,
            color,
        )?;
        return Ok(problems);
    }
    secho(
        out,
        &format!("\n  {WARN} MCP client config(s) still pinned to a single repo"),
        Style::Yellow,
        color,
    )?;
    for config in &report.pinned {
        writeln!(out, "      {}  {}", config.client, config.path.display())?;
    }
    writeln!(
        out,
        "      Remove --root / cwd / DEJAFIX_ROOT from the dejafix entry."
    )?;
    writeln!(
        out,
        "      dejafix never edits client settings — that one is yours to change."
    )?;
    secho(
        out,
        "      Quit the client completely before editing, then re-run `djf doctor`.",
        Style::Yellow,
        color,
    )?;
    for reverted in &report.reverted {
        secho(
            out,
            &format!(
                "      ! {} was clean on {} and is pinned again — the app most likely rewrote this file on exit.",
                reverted.client, reverted.clean_since
            ),
            Style::Red,
            color,
        )?;
    }
    Ok(problems + 1)
}

fn write_corrupt(report: &DoctorReport, out: &mut impl Write, color: bool) -> io::Result<usize> {
    for log in &report.corrupt {
        secho(
            out,
            &format!(
                "\n  {WARN} {} line(s) of {} could not be read",
                log.lines.len(),
                log.path.display()
            ),
            Style::Yellow,
            color,
        )?;
        for skipped in log.lines.iter().take(CORRUPT_LISTED) {
            writeln!(out, "      line {}: {}", skipped.line, skipped.error)?;
        }
        if log.lines.len() > CORRUPT_LISTED {
            writeln!(out, "      … and {} more", log.lines.len() - CORRUPT_LISTED)?;
        }
        writeln!(
            out,
            "      The log is never rewritten; fix or remove those lines by hand."
        )?;
    }
    Ok(usize::from(!report.corrupt.is_empty()))
}

fn write_version(status: &UpdateStatus, out: &mut impl Write, color: bool) -> io::Result<()> {
    match status {
        UpdateStatus::Offline => {
            secho(
                out,
                &format!("\n  {OK} Running {VERSION}"),
                Style::Green,
                color,
            )?;
            writeln!(
                out,
                "      Check PyPI for a newer release:  djf doctor --online"
            )?;
            writeln!(
                out,
                "      Check daily from now on:         djf doctor --auto"
            )?;
            writeln!(
                out,
                "      (dejafix makes no network calls unless you ask.)"
            )
        }
        UpdateStatus::Unreachable => {
            secho(
                out,
                &format!("\n  {OK} Running {VERSION}"),
                Style::Green,
                color,
            )?;
            writeln!(
                out,
                "      Could not reach PyPI — nothing else was affected."
            )
        }
        UpdateStatus::Newer(newest) => {
            secho(
                out,
                &format!("\n  {WARN} {newest} is available (you have {VERSION})"),
                Style::Yellow,
                color,
            )?;
            writeln!(out, "      pip install -U dejafix")
        }
        UpdateStatus::Latest => secho(
            out,
            &format!("\n  {OK} Running {VERSION} — the latest"),
            Style::Green,
            color,
        ),
    }
}

fn write_report(
    report: &DoctorReport,
    fix: bool,
    status: &UpdateStatus,
    out: &mut impl Write,
    color: bool,
) -> io::Result<()> {
    writeln!(out, "dejafix {VERSION} — checking your setup\n")?;
    writeln!(out, "Scanning {} location(s) for projects…", report.roots)?;
    let problems = write_missing(report, fix, out, color)?
        + write_stale(report, fix, out, color)?
        + write_configs(report, out, color)?
        + write_corrupt(report, out, color)?;
    write_version(status, out, color)?;
    writeln!(out)?;
    if problems == 0 {
        secho(out, "Everything checks out.", Style::GreenBold, color)
    } else if fix {
        secho(
            out,
            "Fixed what could be fixed automatically.",
            Style::GreenBold,
            color,
        )
    } else {
        secho(
            out,
            "Run `djf doctor --fix` to apply the fixes above.",
            Style::Bold,
            color,
        )
    }
}

pub fn run(
    run: DoctorRun,
    paths: &ClientPaths,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
    fetch: impl FnOnce() -> Option<String>,
) -> io::Result<Outcome> {
    if let Some(enabled) = run.auto {
        let _ = set_auto_check(settings, enabled);
        let state = if enabled { "on (once a day)" } else { "off" };
        secho(
            out,
            &format!("{OK} Automatic update checks {state}\n"),
            Style::Green,
            config.color,
        )?;
    }
    let options = DoctorOptions {
        fix: run.fix,
        depth: depth(run.depth),
        roots: (!run.roots.is_empty()).then_some(run.roots),
    };
    match doctor(settings, paths, &config.cwd, &options) {
        Ok(report) => {
            let status = update_status(settings, run.online, VERSION, fetch);
            write_report(&report, run.fix, &status, out, config.color).map(|()| Outcome::Done)
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}

fn write_scan(
    roots: &[PathBuf],
    found: &[FoundProject],
    failures: &[(PathBuf, String)],
    depth_limit: i64,
    dry_run: bool,
    out: &mut impl Write,
    color: bool,
) -> io::Result<()> {
    for root in roots {
        writeln!(out, "Scanning {} (depth {depth_limit})…", root.display())?;
    }
    if found.is_empty() {
        return writeln!(out, "No projects with memory found here.");
    }
    for project in found {
        let (mark, note) = if project.known {
            ("  ", "  (already registered)")
        } else {
            ("+ ", "")
        };
        writeln!(out, "  {mark}{}{note}", project.path.display())?;
    }
    let fresh = found.iter().filter(|project| !project.known).count();
    if fresh == 0 {
        return secho(
            out,
            &format!("\n{OK} All {} already registered.", found.len()),
            Style::Green,
            color,
        );
    }
    if dry_run {
        return writeln!(
            out,
            "\n{fresh} would be registered. Re-run without --dry-run."
        );
    }
    for (project, error) in failures {
        secho(
            out,
            &format!("  {FAIL} {}: {error}", project.display()),
            Style::Red,
            color,
        )?;
    }
    secho(
        out,
        &format!(
            "\n{OK} Registered {} project(s).",
            fresh.saturating_sub(failures.len())
        ),
        Style::Green,
        color,
    )?;
    writeln!(
        out,
        "  One MCP server now reaches them all — `djf project list`."
    )
}

pub fn scan(
    paths: &[String],
    depth_limit: i64,
    dry_run: bool,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
) -> io::Result<Outcome> {
    let color = config.color;
    let report = scan_projects(
        settings,
        &config.cwd,
        config.home.as_deref(),
        paths,
        depth(depth_limit),
        dry_run,
    );
    match report {
        Ok(ScanReport::NotADirectory(root)) => {
            secho(
                out,
                &format!("{FAIL} {} is not a directory.", root.display()),
                Style::Red,
                color,
            )?;
            Ok(Outcome::Exit(USER_ERROR))
        }
        Ok(ScanReport::Scanned {
            roots,
            found,
            failures,
        }) => write_scan(&roots, &found, &failures, depth_limit, dry_run, out, color)
            .map(|()| Outcome::Done),
        Err(error) => Ok(Outcome::Failed(error)),
    }
}
