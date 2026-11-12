use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use super::hooks::install;
use super::registry::{load_registry, now_text};
use super::types::InitHooks;
use super::types::{BridgeOutcome, Inherited, InitOptions, InitReport, Settings, WiringError};
use crate::adapters::fs::{
    GlobalStore, RegistryFiles, StoreError, detect_stack, manifest_info, project_name,
    read_optional_text, read_text_within, resolve_path, write_text,
};
use crate::adapters::git::hooks_dir;
use crate::core::global::{Stack, inherited_block, promotable_libraries, relevant_entries};
use crate::core::registry::{RegisterResult, Registry, same_path, slugify};
use crate::core::scaffold::{
    BridgeChange, INITIAL_CONFIG, INSTRUCTIONS_BACKUP_FILE, INSTRUCTIONS_VERSION, ai_instructions,
    apply_bridge, extract_global_block, gitignore_with, initial_plan, initial_project_map,
    initial_summary, instructions_version, project_map_from_stack, splice_global_block,
    stack_summary_suffix,
};

pub(super) const MEMORY_DIR: &str = ".dejafix";
const INSTRUCTIONS_FILE: &str = "AI_INSTRUCTIONS.md";
const PROJECT_MAP_FILE: &str = "PROJECT_MAP.md";
const MAP_PLACEHOLDER: &str = "Status: not created yet";
const RULE_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];
const INHERITED_TAGS: usize = 5;

fn write_failed(path: &Path) -> impl Fn(io::Error) -> StoreError + '_ {
    move |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

fn ensure_dir(path: &Path) -> Result<(), StoreError> {
    match fs::create_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        other => other.map_err(write_failed(path)),
    }
}

fn write_missing(path: &Path, text: &str) -> Result<(), StoreError> {
    if path.exists() {
        return Ok(());
    }
    write_text(path, text)
}

fn ensure_gitignore(root: &Path, extra: &[&str]) -> Result<(), StoreError> {
    let path = root.join(".gitignore");
    let existing = read_text_within(&path, root)?.unwrap_or_default();
    match gitignore_with(&existing, extra) {
        Some(updated) => write_text(&path, &updated),
        None => Ok(()),
    }
}

fn register_quietly(root: &Path, settings: &Settings, home: Option<&Path>) -> Option<Registry> {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let now = now_text(settings);
    let mut registry = files.load(&now).ok()?;
    if let Some(resolved) = resolve_path(&root.to_string_lossy(), home, root)
        && let Ok((RegisterResult::Added | RegisterResult::Realiased, _)) =
            registry.register(resolved, None, &now)
        && files.save(&registry).is_err()
    {
        return files.load(&now).ok();
    }
    Some(registry)
}

fn initialize(
    root: &Path,
    settings: &Settings,
    home: Option<&Path>,
) -> Result<(PathBuf, Option<Registry>), StoreError> {
    let memory = root.join(MEMORY_DIR);
    ensure_dir(&memory)?;
    ensure_dir(&memory.join("issues"))?;
    let events = memory.join("events.jsonl");
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events)
        .map_err(write_failed(&events))?;
    let name = project_name(root);
    write_missing(&memory.join("config.toml"), INITIAL_CONFIG)?;
    write_missing(&memory.join("summary.md"), &initial_summary(&name))?;
    write_missing(&memory.join(INSTRUCTIONS_FILE), ai_instructions())?;
    write_missing(&memory.join(PROJECT_MAP_FILE), &initial_project_map(&name))?;
    write_missing(&memory.join("plan.md"), &initial_plan(&name))?;
    ensure_gitignore(root, &[])?;
    let registry = register_quietly(root, settings, home);
    Ok((memory, registry))
}

fn refresh_instructions(root: &Path) -> Result<bool, StoreError> {
    let memory = root.join(MEMORY_DIR);
    let path = memory.join(INSTRUCTIONS_FILE);
    let Some(current) = read_text_within(&path, root)? else {
        return Ok(false);
    };
    if instructions_version(&current) >= INSTRUCTIONS_VERSION {
        return Ok(false);
    }
    write_text(&memory.join(INSTRUCTIONS_BACKUP_FILE), &current)?;
    let fresh = match extract_global_block(&current) {
        Some(block) => splice_global_block(ai_instructions(), &block),
        None => ai_instructions().to_owned(),
    };
    write_text(&path, &fresh)?;
    ensure_gitignore(root, &[&format!("{MEMORY_DIR}/{INSTRUCTIONS_BACKUP_FILE}")])?;
    Ok(true)
}

fn bridge_name(root: &Path, registry: Option<&Registry>, home: Option<&Path>) -> String {
    let folder = project_name(root);
    let Some(registry) = registry else {
        return folder;
    };
    registry
        .find(&root.to_string_lossy(), |path| {
            resolve_path(path, home, root)
        })
        .and_then(|index| registry.projects.get(index))
        .map_or_else(|| slugify(&folder), |record| record.name().to_owned())
}

fn write_bridges(
    root: &Path,
    name: &str,
) -> Result<Vec<(&'static str, BridgeOutcome)>, StoreError> {
    let mut written = Vec::new();
    for file in RULE_FILES {
        let path = root.join(file);
        let existing = read_text_within(&path, root)?;
        let (outcome, text) = match apply_bridge(existing.as_deref(), file, Some(name)) {
            BridgeChange::Created(text) => (BridgeOutcome::Created, text),
            BridgeChange::Appended(text) => (BridgeOutcome::Appended, text),
            BridgeChange::Refreshed(text) => (BridgeOutcome::Refreshed, text),
            BridgeChange::Unchanged => continue,
        };
        write_text(&path, &text)?;
        written.push((file, outcome));
    }
    Ok(written)
}

fn detect_and_record(root: &Path, settings: &Settings) -> Stack {
    let stack = detect_stack(root);
    let _ =
        GlobalStore::new(&settings.dejafix_home).record_promotable(&promotable_libraries(&stack));
    stack
}

fn cached<'s>(stack: &'s mut Option<Stack>, root: &Path, settings: &Settings) -> &'s Stack {
    stack.get_or_insert_with(|| detect_and_record(root, settings))
}

fn populate_map(
    root: &Path,
    settings: &Settings,
    stack: &mut Option<Stack>,
) -> Result<Option<String>, StoreError> {
    let path = root.join(MEMORY_DIR).join(PROJECT_MAP_FILE);
    let Some(current) = read_optional_text(&path)? else {
        return Ok(None);
    };
    if !current.contains(MAP_PLACEHOLDER) {
        return Ok(None);
    }
    let stack = cached(stack, root, settings);
    let Some(map) = project_map_from_stack(&project_name(root), stack, &manifest_info(root)) else {
        return Ok(None);
    };
    write_text(&path, &map)?;
    Ok(Some(stack_summary_suffix(stack)))
}

fn inherit(
    root: &Path,
    settings: &Settings,
    global_tags: Option<&str>,
    stack: &mut Option<Stack>,
) -> Result<Option<Inherited>, StoreError> {
    let store = GlobalStore::new(&settings.dejafix_home);
    if !store.has_entries() {
        return Ok(None);
    }
    let stack = cached(stack, root, settings);
    if stack.tags.is_empty() && stack.libraries.is_empty() {
        return Ok(None);
    }
    let filter: Option<Vec<String>> = global_tags.filter(|tags| !tags.is_empty()).map(|tags| {
        tags.split(',')
            .map(|tag| crate::core::text::python_strip(tag).to_owned())
            .collect()
    });
    let patterns = store.patterns().unwrap_or_default();
    let gotchas = store.gotchas().unwrap_or_default();
    let relevant = relevant_entries(stack, filter.as_deref(), &patterns, &gotchas);
    let Some(block) = inherited_block(&relevant) else {
        return Ok(None);
    };
    let path = root.join(MEMORY_DIR).join(INSTRUCTIONS_FILE);
    if let Some(content) = read_text_within(&path, root)? {
        write_text(&path, &splice_global_block(&content, &block))?;
    }
    Ok(Some(Inherited {
        tags: stack.tags.iter().take(INHERITED_TAGS).cloned().collect(),
        gotchas: relevant.gotchas.len(),
        patterns: relevant.patterns.len(),
    }))
}

pub fn init_project(
    root: &Path,
    home: Option<&Path>,
    settings: &Settings,
    options: &InitOptions,
) -> Result<InitReport, WiringError> {
    let (mem_dir, registry) = initialize(root, settings, home)?;
    let refreshed_instructions = refresh_instructions(root)?;
    let bridges = if options.claude_md {
        write_bridges(root, &bridge_name(root, registry.as_ref(), home))?
    } else {
        Vec::new()
    };
    let mut stack = None;
    let project_map = if options.stack_detect {
        populate_map(root, settings, &mut stack)?
    } else {
        None
    };
    let hooks = match &options.hooks {
        None => None,
        Some(djf) => {
            let directory = hooks_dir(root);
            Some(if directory.exists() {
                InitHooks::Installed(install(&directory, djf)?)
            } else {
                InitHooks::NoGit
            })
        }
    };
    let inherited = if options.global {
        inherit(root, settings, options.global_tags.as_deref(), &mut stack)?
    } else {
        None
    };
    Ok(InitReport {
        mem_dir,
        refreshed_instructions,
        bridges,
        project_map,
        inherited,
        hooks,
    })
}

#[must_use]
pub fn other_projects(settings: &Settings, root: &Path) -> usize {
    load_registry(settings).map_or(0, |registry| {
        registry
            .projects
            .iter()
            .filter(|record| !same_path(&record.path, root))
            .count()
    })
}
