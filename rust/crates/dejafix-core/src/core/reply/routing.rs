use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::core::registry::Registry;
use crate::core::resolve::Resolution;

#[must_use]
pub fn projects_reply(
    registry: &Registry,
    pinned: Option<&Path>,
    lookup: impl Fn(&str) -> Option<PathBuf>,
) -> String {
    if let Some(pinned) = pinned {
        let name = pinned
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        return format!(
            "This server is pinned to a single project: {name} ({}).\nThe `project` parameter is ignored.",
            pinned.display()
        );
    }
    if registry.projects.is_empty() {
        return "No projects registered. Run `djf init` in a repo (it registers automatically), or `djf project register <path>`.".to_owned();
    }
    let mut lines = Vec::new();
    let active = registry
        .active_project
        .as_deref()
        .and_then(|active| registry.find(active, &lookup))
        .and_then(|index| registry.projects.get(index));
    if let Some(active) = active {
        lines.push(format!(
            "ACTIVE: {} — {}",
            active.name(),
            active.path.display()
        ));
        lines.push("Calls that omit `project` go here. You do not need to pass it.".to_owned());
        lines.push(String::new());
    }
    lines.push(format!(
        "{} project(s) — pass one as `project`:",
        registry.projects.len()
    ));
    for record in &registry.projects {
        let mut line = format!("  {}", record.id);
        if let Some(alias) = record.alias.as_deref().filter(|alias| !alias.is_empty()) {
            let _ = write!(line, " / {alias}");
        }
        if registry.active_project.as_deref() == Some(record.id.as_str()) {
            line.push_str(" (active)");
        }
        let _ = write!(line, " — {}", record.path.display());
        lines.push(line);
    }
    if registry.active_project.is_none() {
        lines.push("\nNo active project: calls that omit `project` will fail rather than guess. Set one with `djf project use <name>`.".to_owned());
    }
    lines.join("\n")
}

#[must_use]
pub fn current_reply(resolution: &Resolution) -> String {
    format!(
        "{} — {}\nchosen by: {}",
        resolution.name,
        resolution.root.display(),
        resolution.source.as_str()
    )
}
