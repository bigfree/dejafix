use std::error::Error;
use std::fs;
use std::path::Path;

use crate::m3::{args, assert_same, compare, repository, write};
use crate::types::Action;

type TestResult = Result<(), Box<dyn Error>>;
type Prepared = Result<Vec<String>, Box<dyn Error>>;

const OLD_HOOK: &str = "#!/bin/sh\n\
echo user hook\n\
# >>> projectmem auto-capture >>>\n\
pjm auto-capture\n\
# <<< projectmem auto-capture <<<\n";
const OLD_BRIDGE: &str = "# CLAUDE.md\n\nMy own rules.\n\n\
<!-- >>> projectmem bridge >>> -->\n\
## projectmem (MANDATORY)\nEdit .projectmem/summary.md never.\n\
<!-- <<< projectmem bridge <<< -->\n";
const EVENTS: &str = "{\"id\": \"evt_1\", \"location\": \".projectmem/summary.md\", \"summary\": \"keep me\", \"timestamp\": \"2026-01-01T09:00:00Z\", \"type\": \"note\"}\r\n";

fn home(project: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    Ok(project.parent().ok_or("no parent")?.join("home"))
}

fn legacy_project(root: &Path) -> Result<(), Box<dyn Error>> {
    repository(root)?;
    write(root, ".projectmem/events.jsonl", EVENTS)?;
    write(
        root,
        ".projectmem/issues/0001-keep.md",
        "# Keep\n\nSee .projectmem/summary.md.\n",
    )?;
    write(
        root,
        ".projectmem/AI_INSTRUCTIONS.md",
        "# projectmem AI Instructions\n\n<!-- projectmem-instructions: 2 -->\n\n## Global Memory\n\nkept\n",
    )?;
    write(root, ".projectmem/summary.md", "# Summary\n")?;
    write(root, ".projectmem/notes.txt", ".projectmem stays\n")?;
    write(
        root,
        ".gitignore",
        "node_modules/\n.projectmem/events.jsonl\n",
    )?;
    write(root, "CLAUDE.md", OLD_BRIDGE)?;
    write(root, ".git/hooks/pre-commit", OLD_HOOK)?;
    Ok(())
}

fn legacy_home(project: &Path) -> Prepared {
    let old = home(project)?.join(".projectmem");
    write(&old, "global/library_gotchas.jsonl", "{}\n")?;
    write(
        &old,
        "projects.json",
        &serde_json::json!([project.display().to_string()]).to_string(),
    )?;
    write(
        &old,
        "projects.meta.json",
        "{\"active_project\": \"project\"}\n",
    )?;
    Ok(args(&["migrate"]))
}

fn step(list: &'static [&'static str]) -> Action<'static> {
    Action::Djf(Box::new(move |_| Ok(args(list))))
}

#[test]
fn migrate_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let project = fixture.path().join("project");
    legacy_project(&project)?;
    let actions = vec![
        Action::Djf(Box::new(legacy_home)),
        step(&["migrate", "--apply"]),
        step(&["migrate", "--apply"]),
        step(&["migrate", "--apply", "--path", "."]),
    ];
    let (python, differences) = compare(&project, &actions, Ok)?;
    assert_same("migrate a legacy project and home", &differences);
    let dry = &python.steps.first().ok_or("no step")?.stdout;
    assert!(dry.starts_with("Dry run"), "{dry}");
    assert!(python.files.contains_key("project/.dejafix/events.jsonl"));
    assert!(python.files.contains_key("home/.dejafix/projects.json"));
    Ok(())
}

fn edge_home(project: &Path) -> Prepared {
    let home = home(project)?;
    let old = home.join(".projectmem");
    write(&old, "global/library_gotchas.jsonl", "old\n")?;
    write(&old, "projects.meta.json", "{\"active_project\": \"x\"}\n")?;
    write(&old, "projects.json", "[]")?;
    write(&home, ".dejafix/global/library_gotchas.jsonl", "new\n")?;
    write(
        &home,
        ".dejafix/projects.meta.json",
        "{\"seen_version\": \"0.4.0\"}\n",
    )?;
    write(
        &home,
        ".codex/config.toml",
        "[mcp_servers.projectmem]\ncommand = \"pjm-mcp\"\n",
    )?;
    write(
        &home,
        ".cursor/mcp.json",
        "{\"projects\": {\"/src/projectmem\": {}}}\n",
    )?;
    let both = project.join("both");
    fs::create_dir_all(both.join(".projectmem"))?;
    fs::create_dir_all(both.join(".dejafix"))?;
    let done = project.join("done");
    write(&done, ".dejafix/summary.md", "# Summary\n")?;
    fs::create_dir_all(project.join("empty"))?;
    Ok(args(&[
        "migrate", "--apply", "-p", "both", "-p", "done", "-p", "empty", "-p", "missing",
    ]))
}

#[test]
fn migrate_edge_cases_match_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let project = fixture.path().join("project");
    fs::create_dir_all(&project)?;
    let actions = vec![Action::Djf(Box::new(edge_home))];
    let (python, differences) = compare(&project, &actions, Ok)?;
    assert_same("migrate edge cases", &differences);
    let out = &python.steps.first().ok_or("no step")?.stdout;
    assert!(out.contains("merge it by hand"), "{out}");
    assert!(out.contains("Still pointing at projectmem"), "{out}");
    Ok(())
}

#[test]
fn doctor_auto_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let project = fixture.path().join("project");
    fs::create_dir_all(&project)?;
    let actions = vec![
        step(&["doctor", "--no-auto", "--path", ".", "--depth", "1"]),
        step(&["doctor", "--auto", "--no-auto", "-p", ".", "-d", "1"]),
    ];
    let (_, differences) = compare(&project, &actions, Ok)?;
    assert_same("doctor --no-auto", &differences);
    Ok(())
}
