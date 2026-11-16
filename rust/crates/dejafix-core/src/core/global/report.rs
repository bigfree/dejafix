use std::collections::BTreeSet;
use std::fmt::Write;

use serde_json::{Map, Value, json};

use super::record::is_upgraded;
use super::relevant::is_truthy;
use super::types::{AddOutcome, Entry, GlobalOutcome, Palette, Stack, Usage};
use crate::core::json::{python_str, write_python_json_indented};

const RULE: &str = "─";
const LIST_RULE_WIDTH: usize = 60;
const DETECT_RULE_WIDTH: usize = 50;
const STATUS_BAR_WIDTH: usize = 48;
const SHOWN_LIBRARIES: usize = 15;
const META_SEPARATOR: &str = " · ";
const AVAILABLE: &str =
    "status, list, add, add-gotcha, query, edit, upgrade, remove, export, import, prune, detect";

fn items(entry: &Entry, key: &str) -> Vec<String> {
    match entry.get(key) {
        Some(Value::Array(values)) => values.iter().map(python_str).collect(),
        Some(Value::String(text)) => text.chars().map(String::from).collect(),
        _ => Vec::new(),
    }
}

fn has_tag(entry: &Entry, tag: &str) -> bool {
    let wanted = tag.to_lowercase();
    items(entry, "tags")
        .iter()
        .any(|candidate| candidate.to_lowercase() == wanted)
}

fn text_of(entry: &Entry, key: &str, default: &str) -> String {
    entry
        .get(key)
        .map_or_else(|| default.to_owned(), python_str)
}

fn truthy_text(entry: &Entry, key: &str) -> Option<String> {
    entry
        .get(key)
        .filter(|value| is_truthy(value))
        .map(python_str)
}

#[must_use]
pub fn filtered(gotchas: Vec<Entry>, tag: Option<&str>, library: Option<&str>) -> Vec<Entry> {
    let tag = tag.filter(|tag| !tag.is_empty());
    let library = library
        .filter(|library| !library.is_empty())
        .map(str::to_lowercase);
    let mut gotchas = gotchas;
    if let Some(tag) = tag {
        gotchas.retain(|entry| has_tag(entry, tag));
    }
    if let Some(library) = library {
        gotchas.retain(|entry| {
            text_of(entry, "library", "")
                .to_lowercase()
                .contains(&library)
        });
    }
    gotchas
}

fn json_text(value: &Value) -> String {
    let mut out = Vec::new();
    let _ = write_python_json_indented(&mut out, value, 2, false);
    format!("{}\n", String::from_utf8_lossy(&out))
}

fn status(location: &str, gotchas: &[Entry], preferences: bool, palette: Palette) -> String {
    let Palette {
        bold, dim, reset, ..
    } = palette;
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut libraries: BTreeSet<String> = BTreeSet::new();
    let mut projects: BTreeSet<String> = BTreeSet::new();
    for entry in gotchas {
        tags.extend(items(entry, "tags"));
        projects.extend(truthy_text(entry, "source_project"));
        libraries.insert(text_of(entry, "library", ""));
    }
    libraries.remove("");
    let bar = "=".repeat(STATUS_BAR_WIDTH);
    let mut out = format!(
        "\n  {dim}{bar}{reset}\n  {bold}  dejafix Global Memory{reset}\n  {dim}{bar}{reset}\n  {dim}Location: {location}{reset}\n\n"
    );
    let _ = write!(
        out,
        "    Library gotchas:     {bold}{}{reset}\n    Stack preferences:   {bold}{}{reset}\n    Unique tags:         {bold}{}{reset}\n    Libraries tracked:   {bold}{}{reset}\n    Source projects:     {bold}{}{reset}\n",
        gotchas.len(),
        if preferences { "yes" } else { "not set" },
        tags.len(),
        libraries.len(),
        projects.len(),
    );
    let not_upgraded = gotchas.iter().filter(|entry| !is_upgraded(entry)).count();
    if not_upgraded > 0 {
        let _ = writeln!(out, "    Not upgraded:        {bold}{not_upgraded}{reset}");
    }
    let joined = |set: &BTreeSet<String>| {
        set.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !tags.is_empty() {
        let _ = write!(out, "\n  {dim}Tags:{reset} {}\n", joined(&tags));
    }
    if !libraries.is_empty() {
        let _ = writeln!(out, "  {dim}Libraries:{reset} {}", joined(&libraries));
    }
    if !projects.is_empty() {
        let _ = writeln!(out, "  {dim}Projects:{reset} {}", joined(&projects));
    }
    let _ = write!(out, "  {dim}{bar}{reset}\n\n");
    out
}

fn meta(out: &mut String, parts: &[String], palette: Palette) {
    if !parts.is_empty() {
        let _ = writeln!(
            out,
            "    {}{}{}",
            palette.dim,
            parts.join(META_SEPARATOR),
            palette.reset
        );
    }
}

fn list_text(
    gotchas: &[Entry],
    tag: Option<&str>,
    library: Option<&str>,
    palette: Palette,
) -> String {
    let Palette {
        bold,
        dim,
        yellow,
        reset,
        ..
    } = palette;
    let mut out = String::new();
    if gotchas.is_empty() {
        out.push_str("No entries found.");
        if tag.is_some_and(|tag| !tag.is_empty())
            || library.is_some_and(|library| !library.is_empty())
        {
            let _ = write!(
                out,
                " (filter: tag={}, library={})",
                tag.unwrap_or("None"),
                library.unwrap_or("None")
            );
        }
        out.push_str(
            "\n  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library> --keyword <phrase> --keyword <phrase>\n",
        );
        return out;
    }
    let rule = RULE.repeat(LIST_RULE_WIDTH);
    let _ = write!(
        out,
        "\n{bold}Library Gotchas ({}){reset}\n{dim}{rule}{reset}\n",
        gotchas.len()
    );
    for entry in gotchas {
        let upgraded = is_upgraded(entry);
        let _ = writeln!(
            out,
            "  {yellow}{}{reset}{}  [{bold}{}{reset}] {}",
            text_of(entry, "id", "?"),
            if upgraded { "" } else { " (not upgraded)" },
            text_of(entry, "library", "unknown"),
            text_of(entry, "gotcha", "")
        );
        let mut parts = Vec::new();
        if upgraded {
            let keywords = items(entry, "keywords").join(", ");
            if !keywords.is_empty() {
                parts.push(format!("keywords: {keywords}"));
            }
        }
        let version =
            truthy_text(entry, "versions").or_else(|| truthy_text(entry, "version_range"));
        if let Some(version) = version {
            parts.push(format!("version: {version}"));
        }
        if let Some(source) = truthy_text(entry, "source_project") {
            parts.push(format!("from: {source}"));
        }
        meta(&mut out, &parts, palette);
    }
    out.push('\n');
    out
}

fn list_json(gotchas: &[Entry]) -> String {
    json_text(&json!({ "gotchas": gotchas }))
}

fn detect_json(name: &str, stack: &Stack) -> String {
    let mut map = Map::new();
    map.insert("name".to_owned(), json!(name));
    map.insert("tags".to_owned(), json!(stack.tags));
    map.insert("libraries".to_owned(), json!(stack.libraries));
    map.insert("frameworks".to_owned(), json!(stack.frameworks));
    map.insert("manifest_files".to_owned(), json!(stack.manifest_files));
    json_text(&Value::Object(map))
}

fn detect_text(name: &str, stack: &Stack, inherited: usize, palette: Palette) -> String {
    let Palette {
        bold,
        dim,
        cyan,
        reset,
        ..
    } = palette;
    let rule = RULE.repeat(DETECT_RULE_WIDTH);
    let mut out = format!("\n{bold}Detected Stack: {name}{reset}\n{dim}{rule}{reset}\n");
    if stack.tags.is_empty() {
        let _ = writeln!(out, "  Tags:       {dim}(none detected){reset}");
    } else {
        let _ = writeln!(out, "  Tags:       {cyan}{}{reset}", stack.tags.join(", "));
    }
    if stack.libraries.is_empty() {
        let _ = writeln!(out, "  Libraries:  {dim}(none detected){reset}");
    } else {
        let shown: Vec<&str> = stack
            .libraries
            .iter()
            .take(SHOWN_LIBRARIES)
            .map(String::as_str)
            .collect();
        let _ = writeln!(out, "  Libraries:  {}", shown.join(", "));
        if stack.libraries.len() > SHOWN_LIBRARIES {
            let _ = writeln!(
                out,
                "              ... and {} more",
                stack.libraries.len() - SHOWN_LIBRARIES
            );
        }
    }
    if !stack.frameworks.is_empty() {
        let _ = writeln!(
            out,
            "  Frameworks: {cyan}{}{reset}",
            stack.frameworks.join(", ")
        );
    }
    let _ = writeln!(out, "  Manifests:  {}", stack.manifest_files.join(", "));
    if inherited > 0 {
        let _ = write!(
            out,
            "\n  {bold}Would inherit:{reset}\n    {inherited} library gotchas\n"
        );
    } else {
        let _ = write!(
            out,
            "\n  {dim}No matching global entries for this stack.{reset}\n"
        );
    }
    out.push('\n');
    out
}

fn usage_text(usage: Usage) -> &'static str {
    match usage {
        Usage::AddGotcha => {
            "Usage: djf global add-gotcha \"<text>\" --library <name> --keyword <phrase> --keyword <phrase> [--versions <range>]\nWrite one lesson in English, 30 to 600 characters, without project details; give 2 to 8 keywords.\n"
        }
        Usage::Upgrade => {
            "Usage: djf global upgrade --export <file> | djf global upgrade --apply <file>\n"
        }
        Usage::Edit => {
            "Usage: djf global edit <id> [--library <name>] [--keyword <phrase>...] [--versions <range>] [--alias <library>=<alias>]\n"
        }
        Usage::Remove => "Usage: djf global remove <id>\n",
        Usage::Import => "Usage: djf global import <file.json>\n",
    }
}

fn gotcha_added(tag: &str, id: &str, library: &str, text: &str, warnings: &[String]) -> String {
    let mut out = format!("{tag} Gotcha added: {id}\n  [{library}] {text}\n");
    for warning in warnings {
        let _ = writeln!(out, "  Warning: {warning}");
    }
    out
}

fn detected(name: &str, stack: &Stack, inherited: usize, json: bool, palette: Palette) -> String {
    if json {
        detect_json(name, stack)
    } else {
        detect_text(name, stack, inherited, palette)
    }
}

fn upgrade_invalid(problems: &[String]) -> String {
    let mut out = String::from("Draft file not applied, nothing was written:\n");
    for problem in problems {
        let _ = writeln!(out, "  {problem}");
    }
    out
}

fn upgrade_applied(
    tag: &str,
    replaced: usize,
    dropped: usize,
    kept: &[(String, Vec<String>)],
) -> String {
    let mut out = format!(
        "{tag} Upgrade: {replaced} replaced, {dropped} dropped, {} kept\n",
        kept.len()
    );
    for (id, reasons) in kept {
        let _ = writeln!(out, "  {id}: {}", reasons.join("; "));
    }
    out
}

#[must_use]
pub fn global_report(outcome: &GlobalOutcome, color: bool) -> String {
    let palette = if color {
        Palette::COLOR
    } else {
        Palette::PLAIN
    };
    let tag = format!("{}[global]{}", palette.green, palette.reset);
    match outcome {
        GlobalOutcome::Status {
            location,
            gotchas,
            preferences,
        } => status(location, gotchas, *preferences, palette),
        GlobalOutcome::List {
            gotchas,
            tag,
            library,
            json,
        } => {
            if *json {
                list_json(gotchas)
            } else {
                list_text(gotchas, tag.as_deref(), library.as_deref(), palette)
            }
        }
        GlobalOutcome::Usage(usage) => usage_text(*usage).to_owned(),
        GlobalOutcome::GotchaAdded {
            id,
            library,
            text,
            warnings,
        } => gotcha_added(&tag, id, library, text, warnings),
        GlobalOutcome::GotchaRejected { failures, warnings } => refused_text(failures, warnings),
        GlobalOutcome::Reply(text) => format!("{text}\n"),
        GlobalOutcome::Edited {
            id,
            aliases,
            warnings,
        } => edited_text(id.as_deref(), aliases, warnings, &tag),
        GlobalOutcome::EditRefused(reasons) => format!("Not changed: {}.\n", reasons.join("; ")),
        GlobalOutcome::UpgradeExported { file, drafts } => {
            format!(
                "{tag} Drafts written: {drafts} records to {file}\n  Let the agent fill them, review them, then: djf global upgrade --apply {file}\n"
            )
        }
        GlobalOutcome::UpgradeExists(file) => {
            format!("Not written: {file} already exists; move it away or choose another name.\n")
        }
        GlobalOutcome::UpgradeMissing(file) => format!("File not found: {file}\n"),
        GlobalOutcome::UpgradeInvalid(problems) => upgrade_invalid(problems),
        GlobalOutcome::UpgradeApplied {
            replaced,
            dropped,
            kept,
        } => upgrade_applied(&tag, *replaced, *dropped, kept),
        GlobalOutcome::Removed(id) => format!("{tag} Removed: {id}\n"),
        GlobalOutcome::NotFound(id) => format!("Entry not found: {id}\n"),
        GlobalOutcome::Exported(value) => json_text(value),
        GlobalOutcome::Imported {
            gotchas,
            skipped_patterns: 0,
        } => format!("{tag} Imported: {gotchas} gotchas\n"),
        GlobalOutcome::Imported {
            gotchas,
            skipped_patterns,
        } => format!(
            "{tag} Imported: {gotchas} gotchas\n  Skipped {skipped_patterns} cross-project pattern(s); dejafix no longer keeps them.\n"
        ),
        GlobalOutcome::FileNotFound(path) => format!("File not found: {path}\n"),
        GlobalOutcome::InvalidJson(message) => format!("Invalid JSON: {message}\n"),
        GlobalOutcome::Pruned { removed: 0, .. } => "No entries to prune.\n".to_owned(),
        GlobalOutcome::Pruned {
            removed,
            older_than,
        } => {
            format!("{tag} Pruned {removed} entries (older than {older_than} days)\n")
        }
        GlobalOutcome::Detected {
            name,
            stack,
            inherited_gotchas,
            json,
        } => detected(name, stack, *inherited_gotchas, *json, palette),
        GlobalOutcome::Unknown(action) => {
            format!("Unknown action: {action}\nAvailable: {AVAILABLE}\n")
        }
    }
}

#[must_use]
pub fn add_reply(outcome: &AddOutcome) -> String {
    match outcome {
        AddOutcome::Added {
            id,
            library,
            warnings,
        } => {
            let mut reply = format!("Added global gotcha {id} ({library}).");
            for warning in warnings {
                let _ = write!(reply, "\nWarning: {warning}.");
            }
            reply
        }
        AddOutcome::Rejected(report) => {
            let failures: Vec<String> = report.failures.iter().map(ToString::to_string).collect();
            let mut reply = format!("Not added: {}.", failures.join("; "));
            for warning in &report.warnings {
                let _ = write!(reply, "\nWarning: {warning}.");
            }
            reply
        }
    }
}

fn edited_text(id: Option<&str>, aliases: &[String], warnings: &[String], tag: &str) -> String {
    let mut out = String::new();
    if let Some(id) = id {
        let _ = writeln!(out, "{tag} Gotcha edited: {id}");
    }
    for alias in aliases {
        let _ = writeln!(out, "{tag} Alias added: {alias}");
    }
    if out.is_empty() {
        out.push_str("Nothing to change.\n");
    }
    for warning in warnings {
        let _ = writeln!(out, "  Warning: {warning}");
    }
    out
}

fn refused_text(failures: &[String], warnings: &[String]) -> String {
    let mut out = format!("Not added: {}.\n", failures.join("; "));
    for warning in warnings {
        let _ = writeln!(out, "  Warning: {warning}");
    }
    out
}
