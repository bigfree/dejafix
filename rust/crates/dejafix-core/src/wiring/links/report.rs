use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use super::project::{every_project_links, links_with, project_links};
use crate::adapters::fs::{
    FsResolver, MEMORY_DIR, find_memory_dir, posix, read_events_unlocked, read_project_links,
};
use crate::core::lessons::{LessonKind, Memory, records};
use crate::core::links::{
    Glob, Link, LinkedBlock, RELATION_CHARS, boundary_globs, direction_text, linked_blocks,
    linked_report, links_section, matching, path_query, sanitize,
};
use crate::wiring::types::{Settings, WiringError};

fn read_log(link: &Link) -> Option<String> {
    let bytes = read_events_unlocked(&link.path.join(MEMORY_DIR)).ok()?;
    Some(
        String::from_utf8(bytes)
            .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned()),
    )
}

#[must_use]
pub fn boundary_reports(
    dejafix_home: &Path,
    root: &Path,
    relatives: &[String],
) -> Vec<(String, String)> {
    let Ok(own) = read_project_links(root) else {
        return Vec::new();
    };
    let own_globs = boundary_globs(&own.boundary);
    let wanted: Vec<(&String, String)> = relatives
        .iter()
        .filter_map(|relative| {
            matching(&own_globs, relative).map(|glob| (relative, path_query(relative, Some(glob))))
        })
        .collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let linked = links_with(dejafix_home, root, Ok(own));
    let mut blocks: HashMap<&str, Vec<LinkedBlock>> = HashMap::new();
    for link in linked
        .resolution
        .links
        .iter()
        .filter(|link| !link.boundary.is_empty())
    {
        let Some(text) = read_log(link) else {
            continue;
        };
        let found = records(&text);
        let memory = Memory::new(&found, &posix(&link.path), &FsResolver);
        let globs: Vec<Glob> = boundary_globs(&link.boundary);
        let lessons = memory.lessons_where(|file| matching(&globs, file).is_some());
        let queries: Vec<&str> = wanted.iter().map(|(_, query)| query.as_str()).collect();
        for ((relative, _), block) in wanted
            .iter()
            .zip(linked_blocks(link, &lessons, &globs, &queries))
        {
            if let Some(block) = block {
                blocks.entry(relative.as_str()).or_default().push(block);
            }
        }
    }
    wanted
        .iter()
        .filter_map(|(relative, _)| {
            let report = linked_report(relative, blocks.remove(relative.as_str())?);
            (!report.is_empty()).then(|| ((*relative).clone(), report))
        })
        .collect()
}

fn open_on_boundary(link: &Link) -> Option<usize> {
    if link.boundary.is_empty() {
        return None;
    }
    let text = read_log(link)?;
    let found = records(&text);
    let memory = Memory::new(&found, &posix(&link.path), &FsResolver);
    let globs = boundary_globs(&link.boundary);
    Some(
        memory
            .lessons_where(|file| matching(&globs, file).is_some())
            .iter()
            .filter(|lesson| lesson.kind == LessonKind::Issue)
            .count(),
    )
}

#[must_use]
pub fn links_text(dejafix_home: &Path, root: &Path) -> String {
    let linked = project_links(dejafix_home, root);
    if let Some(error) = linked.config_error {
        return format!("could not read links: {error}");
    }
    let counted: Vec<(&Link, Option<usize>)> = linked
        .resolution
        .links
        .iter()
        .map(|link| (link, open_on_boundary(link)))
        .collect();
    links_section(&counted)
}

pub fn links_listing(cwd: &Path, settings: &Settings) -> Result<String, WiringError> {
    let memory = find_memory_dir(cwd)?;
    let linked = project_links(&settings.dejafix_home, &memory.root);
    let mut out = format!(
        "dejafix links — {} ({})\n",
        linked.own.id, linked.own.identity
    );
    if let Some(error) = &linked.config_error {
        let _ = writeln!(out, "  {error}");
        return Ok(out);
    }
    if !linked.own.config.boundary.is_empty() {
        let _ = writeln!(
            out,
            "  Boundary: {}",
            sanitize(&linked.own.config.boundary.join(", "), usize::MAX)
        );
    }
    if linked.resolution.links.is_empty() {
        out.push_str(
            "  No linked projects. Declare one under [[links]] in .dejafix/config.toml.\n",
        );
    }
    for link in &linked.resolution.links {
        let _ = writeln!(
            out,
            "  - {}: {}, {}, {}",
            link.id,
            direction_text(link.direction),
            link.identity,
            link.path.display()
        );
        if !link.relation.is_empty() {
            let _ = writeln!(out, "      \"{}\"", link.relation);
        }
    }
    for value in &linked.resolution.unresolved {
        let _ = writeln!(
            out,
            "  Not registered on this machine: {}",
            sanitize(value, RELATION_CHARS)
        );
    }
    for identity in &linked.resolution.duplicates {
        let _ = writeln!(
            out,
            "  Registered more than once: {identity} (the main checkout is used)"
        );
    }
    Ok(out)
}

#[must_use]
pub fn link_problems(dejafix_home: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for linked in every_project_links(dejafix_home) {
        let id = &linked.own.id;
        if let Some(error) = linked.config_error {
            problems.push(format!("{id}: {error}"));
            continue;
        }
        for value in &linked.resolution.unresolved {
            problems.push(format!(
                "{id}: {} is not registered on this machine",
                sanitize(value, RELATION_CHARS)
            ));
        }
        for identity in &linked.resolution.duplicates {
            let line =
                format!("{identity} is registered more than once; the main checkout is used");
            if !problems.contains(&line) {
                problems.push(line);
            }
        }
    }
    problems
}
