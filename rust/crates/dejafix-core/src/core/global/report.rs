use std::collections::BTreeSet;
use std::fmt::Write;

use serde_json::{Map, Value, json};

use super::inherit::is_truthy;
use super::types::{Entry, GlobalOutcome, Palette, Stack, Usage};
use crate::core::json::{python_str, write_python_json_indented};

const RULE: &str = "─";
const LIST_RULE_WIDTH: usize = 60;
const DETECT_RULE_WIDTH: usize = 50;
const STATUS_BAR_WIDTH: usize = 48;
const SHOWN_LIBRARIES: usize = 15;
const META_SEPARATOR: &str = " · ";
const AVAILABLE: &str = "status, list, add, add-gotcha, remove, export, import, prune, detect";

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
pub fn filtered(
    patterns: Vec<Entry>,
    gotchas: Vec<Entry>,
    tag: Option<&str>,
    library: Option<&str>,
) -> (Vec<Entry>, Vec<Entry>) {
    let tag = tag.filter(|tag| !tag.is_empty());
    let library = library
        .filter(|library| !library.is_empty())
        .map(str::to_lowercase);
    let mut patterns = patterns;
    let mut gotchas = gotchas;
    if let Some(tag) = tag {
        patterns.retain(|entry| has_tag(entry, tag));
        gotchas.retain(|entry| has_tag(entry, tag));
    }
    if let Some(library) = library {
        gotchas.retain(|entry| {
            text_of(entry, "library", "")
                .to_lowercase()
                .contains(&library)
        });
    }
    (patterns, gotchas)
}

fn json_text(value: &Value) -> String {
    let mut out = Vec::new();
    let _ = write_python_json_indented(&mut out, value, 2, false);
    format!("{}\n", String::from_utf8_lossy(&out))
}

fn status(
    location: &str,
    patterns: &[Entry],
    gotchas: &[Entry],
    preferences: bool,
    palette: Palette,
) -> String {
    let Palette {
        bold, dim, reset, ..
    } = palette;
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut libraries: BTreeSet<String> = BTreeSet::new();
    let mut projects: BTreeSet<String> = BTreeSet::new();
    for entry in patterns.iter().chain(gotchas) {
        tags.extend(items(entry, "tags"));
        projects.extend(truthy_text(entry, "source_project"));
    }
    for entry in gotchas {
        libraries.insert(text_of(entry, "library", ""));
    }
    libraries.remove("");
    let bar = "=".repeat(STATUS_BAR_WIDTH);
    let mut out = format!(
        "\n  {dim}{bar}{reset}\n  {bold}  dejafix Global Memory{reset}\n  {dim}{bar}{reset}\n  {dim}Location: {location}{reset}\n\n"
    );
    let _ = write!(
        out,
        "    Patterns:           {bold}{}{reset}\n    Library gotchas:     {bold}{}{reset}\n    Stack preferences:   {bold}{}{reset}\n    Unique tags:         {bold}{}{reset}\n    Libraries tracked:   {bold}{}{reset}\n    Source projects:     {bold}{}{reset}\n",
        patterns.len(),
        gotchas.len(),
        if preferences { "yes" } else { "not set" },
        tags.len(),
        libraries.len(),
        projects.len(),
    );
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
    patterns: &[Entry],
    gotchas: &[Entry],
    tag: Option<&str>,
    library: Option<&str>,
    palette: Palette,
) -> String {
    let Palette {
        bold,
        dim,
        cyan,
        yellow,
        reset,
        ..
    } = palette;
    let mut out = String::new();
    if patterns.is_empty() && gotchas.is_empty() {
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
            "\n  Add patterns:  djf global add \"your pattern here\"\n  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library>\n",
        );
        return out;
    }
    let rule = RULE.repeat(LIST_RULE_WIDTH);
    if !patterns.is_empty() {
        let _ = write!(
            out,
            "\n{bold}Patterns ({}){reset}\n{dim}{rule}{reset}\n",
            patterns.len()
        );
        for entry in patterns {
            let _ = writeln!(
                out,
                "  {cyan}{}{reset}  {}",
                text_of(entry, "id", "?"),
                text_of(entry, "pattern", "")
            );
            let mut parts = Vec::new();
            let tags = items(entry, "tags").join(", ");
            if !tags.is_empty() {
                parts.push(format!("tags: {tags}"));
            }
            if let Some(source) = truthy_text(entry, "source_project") {
                parts.push(format!("from: {source}"));
            }
            if let Some(confidence) = truthy_text(entry, "confidence") {
                parts.push(format!("confidence: {confidence}"));
            }
            meta(&mut out, &parts, palette);
        }
    }
    if !gotchas.is_empty() {
        let _ = write!(
            out,
            "\n{bold}Library Gotchas ({}){reset}\n{dim}{rule}{reset}\n",
            gotchas.len()
        );
        for entry in gotchas {
            let _ = writeln!(
                out,
                "  {yellow}{}{reset}  [{bold}{}{reset}] {}",
                text_of(entry, "id", "?"),
                text_of(entry, "library", "unknown"),
                text_of(entry, "gotcha", "")
            );
            let mut parts = Vec::new();
            if let Some(version) = truthy_text(entry, "version_range") {
                parts.push(format!("version: {version}"));
            }
            if let Some(source) = truthy_text(entry, "source_project") {
                parts.push(format!("from: {source}"));
            }
            meta(&mut out, &parts, palette);
        }
    }
    out.push('\n');
    out
}

fn list_json(patterns: &[Entry], gotchas: &[Entry]) -> String {
    json_text(&json!({
        "patterns": patterns,
        "gotchas": gotchas,
    }))
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

fn detect_text(name: &str, stack: &Stack, inherited: (usize, usize), palette: Palette) -> String {
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
    if inherited.0 > 0 || inherited.1 > 0 {
        let _ = write!(
            out,
            "\n  {bold}Would inherit:{reset}\n    {} patterns, {} library gotchas\n",
            inherited.0, inherited.1
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
        Usage::Add => "Usage: djf global add \"your pattern here\" [--tag python]\n",
        Usage::AddGotcha => {
            "Usage: djf global add-gotcha --library sqlalchemy \"gotcha text\" [--tag python]\n"
        }
        Usage::Remove => "Usage: djf global remove <id>\n",
        Usage::Import => "Usage: djf global import <file.json>\n",
    }
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
            patterns,
            gotchas,
            preferences,
        } => status(location, patterns, gotchas, *preferences, palette),
        GlobalOutcome::List {
            patterns,
            gotchas,
            tag,
            library,
            json,
        } => {
            if *json {
                list_json(patterns, gotchas)
            } else {
                list_text(
                    patterns,
                    gotchas,
                    tag.as_deref(),
                    library.as_deref(),
                    palette,
                )
            }
        }
        GlobalOutcome::Usage(usage) => usage_text(*usage).to_owned(),
        GlobalOutcome::PatternAdded { id, text } => {
            format!("{tag} Pattern added: {id}\n  {text}\n")
        }
        GlobalOutcome::GotchaAdded { id, library, text } => {
            format!("{tag} Gotcha added: {id}\n  [{library}] {text}\n")
        }
        GlobalOutcome::Removed(id) => format!("{tag} Removed: {id}\n"),
        GlobalOutcome::NotFound(id) => format!("Entry not found: {id}\n"),
        GlobalOutcome::Exported(value) => json_text(value),
        GlobalOutcome::Imported { patterns, gotchas } => {
            format!("{tag} Imported: {patterns} patterns, {gotchas} gotchas\n")
        }
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
            inherited_patterns,
            inherited_gotchas,
            json,
        } => {
            if *json {
                detect_json(name, stack)
            } else {
                detect_text(
                    name,
                    stack,
                    (*inherited_patterns, *inherited_gotchas),
                    palette,
                )
            }
        }
        GlobalOutcome::Unknown(action) => {
            format!("Unknown action: {action}\nAvailable: {AVAILABLE}\n")
        }
    }
}
