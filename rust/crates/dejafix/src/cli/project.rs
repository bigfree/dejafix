use std::io::{self, Write};

use dejafix_core::{
    ListedProject, ProjectRecord, RegistryOp, RegistryReply, Settings, WiringError,
    register_project, registry_op,
};

use super::USER_ERROR;
use super::args::ProjectAction;
use super::output::{FAIL, OK, RUNNING, WARN, printable, printable_text, secho};
use super::types::{Outcome, Style};
use crate::config::Config;

fn listing(project: &ListedProject, active: Option<&str>) -> String {
    let record = &project.record;
    let mark = if active == Some(record.id.as_str()) {
        RUNNING
    } else {
        " "
    };
    let alias = record
        .alias
        .as_ref()
        .map(|alias| format!("  ({alias})"))
        .unwrap_or_default();
    let tags = if record.tags.is_empty() {
        String::new()
    } else {
        format!("  #{}", record.tags.join(" #"))
    };
    let missing = if project.has_memory {
        String::new()
    } else {
        format!("   {WARN} no memory here")
    };
    format!(
        " {mark} {}{alias}{tags}\n     {}{missing}",
        record.id,
        printable(&record.path)
    )
}

fn print_empty(out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "No projects registered yet.")?;
    writeln!(out)?;
    writeln!(out, "  Already have projects with memory? Find them:")?;
    writeln!(out, "    djf project scan ~/code --dry-run")?;
    writeln!(out)?;
    writeln!(
        out,
        "  Otherwise run `djf init` in a repo, or add one directly:"
    )?;
    writeln!(out, "    djf project register <path>")
}

fn print_reply(reply: RegistryReply, out: &mut impl Write, color: bool) -> io::Result<()> {
    let ok = |out: &mut _, text: String| secho(out, &format!("{OK} {text}"), Style::Green, color);
    match reply {
        RegistryReply::Listed { projects, .. } if projects.is_empty() => print_empty(out),
        RegistryReply::Listed { active, projects } => {
            writeln!(out, "{} project(s)   {RUNNING} = active\n", projects.len())?;
            for project in &projects {
                writeln!(out, "{}", listing(project, active.as_deref()))?;
            }
            Ok(())
        }
        RegistryReply::Activated(None) => ok(out, "Active project cleared".to_owned()),
        RegistryReply::Activated(Some(record)) => {
            ok(out, format!("Active project: {}", record.name()))?;
            writeln!(out, "  {}", printable(&record.path))
        }
        RegistryReply::Removed(record) => {
            ok(out, format!("Removed {} from the registry", record.name()))?;
            writeln!(
                out,
                "  {} and its .dejafix/ are untouched.",
                printable(&record.path)
            )
        }
        RegistryReply::Aliased(record) => ok(
            out,
            format!(
                "{} is now also '{}'",
                record.id,
                record.alias.as_deref().unwrap_or_default()
            ),
        ),
        RegistryReply::Tagged(record) => {
            let tags = if record.tags.is_empty() {
                "(none)".to_owned()
            } else {
                record
                    .tags
                    .iter()
                    .map(|tag| format!("#{tag}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            ok(out, format!("{}: {tags}", record.name()))
        }
    }
}

fn print_registered(record: &ProjectRecord, out: &mut impl Write, color: bool) -> io::Result<()> {
    secho(
        out,
        &format!("{OK} Registered {}", record.name()),
        Style::Green,
        color,
    )?;
    writeln!(out, "  {}", printable(&record.path))?;
    writeln!(
        out,
        "  Use it from any folder:  djf project use {}",
        record.name()
    )
}

fn finish(
    result: Result<io::Result<()>, WiringError>,
    out: &mut impl Write,
    color: bool,
) -> io::Result<Outcome> {
    match result {
        Ok(printed) => printed.map(|()| Outcome::Done),
        Err(WiringError::Registry(error)) => {
            secho(
                out,
                &format!("{FAIL} {}", printable_text(&error.to_string())),
                Style::Red,
                color,
            )?;
            Ok(Outcome::Exit(USER_ERROR))
        }
        Err(error) => Ok(Outcome::Failed(error)),
    }
}

pub fn run(
    action: ProjectAction,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
) -> io::Result<Outcome> {
    let home = config.home.as_deref();
    let color = config.color;
    let op = match action {
        ProjectAction::Register { path, alias } => {
            let path = path.unwrap_or_else(|| ".".to_owned());
            let result = register_project(settings, &config.cwd, home, &path, alias.as_deref())
                .map(|record| print_registered(&record, out, color));
            return finish(result, out, color);
        }
        ProjectAction::Scan {
            paths,
            depth,
            dry_run,
        } => return super::setup::scan(&paths, depth, dry_run, config, settings, out),
        ProjectAction::List => RegistryOp::List,
        ProjectAction::Use { identifier } => RegistryOp::Use(identifier),
        ProjectAction::Remove { identifier } => RegistryOp::Remove(identifier),
        ProjectAction::Alias { identifier, alias } => RegistryOp::Alias { identifier, alias },
        ProjectAction::Tag {
            identifier,
            tag,
            remove,
        } => RegistryOp::Tag {
            identifier,
            tag,
            remove,
        },
    };
    let result =
        registry_op(settings, &config.cwd, home, op).map(|reply| print_reply(reply, out, color));
    finish(result, out, color)
}
