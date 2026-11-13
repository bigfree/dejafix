use std::fs;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use super::types::{Halt, LegacyProject, MigrateEvent, MigrateOptions, Plan, ProjectState};
use crate::adapters::fs::{
    RegistryFiles, StoreError, clear_destination, client_configs, files_with_legacy_paths,
    holds_data, is_symlink, live_watcher, move_entry, points_outside, project_name, read_universal,
    resolve_path, write_atomic,
};
use crate::adapters::git::hooks_dir;
use crate::core::hooks::HookKind;
use crate::core::migrate::{
    BRIDGE_BLOCK, Block, HOOK_BLOCK, LEGACY_DIR, MigrateStep, convert_block, has_legacy_path,
    is_stale_config, rewrite_legacy_paths,
};
use crate::core::registry::{Loaded, Registry, load, slugify};
use crate::core::scaffold::{INSTRUCTIONS_VERSION, instructions_version};
use crate::wiring::hooks::install;
use crate::wiring::init::{MEMORY_DIR, RULE_FILES, refresh_instructions, write_bridges};
use crate::wiring::registry::now_text;
use crate::wiring::types::{Settings, WiringError};

const LIST_FILE: &str = "projects.json";
const STAMP_ONLY: &str = "projects.meta.json";
const INSTRUCTIONS_FILE: &str = "AI_INSTRUCTIONS.md";
const GITIGNORE: &str = ".gitignore";

fn emit(
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
    event: MigrateEvent,
) -> Result<(), Halt> {
    match sink(event) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(Halt::Stopped),
    }
}

fn failed(path: &Path) -> impl Fn(io::Error) -> StoreError + '_ {
    move |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

fn read_registry(home: &Path, apply: bool, now: &str) -> Result<Registry, StoreError> {
    let files = RegistryFiles::new(home);
    if apply {
        return files.load(now);
    }
    let (list, meta) = files.read();
    Ok(
        match load(list.as_ref(), meta.as_ref(), now, |raw| {
            Path::new(raw).is_absolute()
        }) {
            Loaded::Clean(registry) | Loaded::Dirty(registry) | Loaded::Legacy(registry) => {
                registry
            }
        },
    )
}

fn legacy_projects(
    options: &MigrateOptions,
    new_home: &Path,
    now: &str,
) -> Result<Vec<LegacyProject>, StoreError> {
    let home = if options.old_home.join(LIST_FILE).exists() {
        options.old_home.as_path()
    } else {
        new_home
    };
    let registry = read_registry(home, options.apply, now)?;
    let mut projects: Vec<LegacyProject> = registry
        .projects
        .iter()
        .map(|record| LegacyProject {
            name: record.name().to_owned(),
            root: record.path.clone(),
        })
        .collect();
    let resolve =
        |raw: &str| resolve_path(raw, None, &options.cwd).unwrap_or_else(|| options.cwd.join(raw));
    let known: Vec<PathBuf> = projects
        .iter()
        .map(|project| resolve(&project.root.to_string_lossy()))
        .collect();
    for raw in &options.extra_roots {
        let resolved = resolve(raw);
        if !known.contains(&resolved) {
            projects.push(LegacyProject {
                name: slugify(&project_name(&resolved)),
                root: resolved,
            });
        }
    }
    Ok(projects)
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let mut entries = fs::read_dir(directory)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<io::Result<Vec<_>>>()
        })
        .map_err(|source| StoreError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort();
    Ok(entries)
}

fn migrate_home(
    old_home: &Path,
    new_home: &Path,
    apply: bool,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), Halt> {
    if !old_home.is_dir() {
        emit(sink, MigrateEvent::HomeMissing(old_home.to_path_buf()))?;
        return Ok(());
    }
    emit(
        sink,
        MigrateEvent::Home {
            from: old_home.to_path_buf(),
            to: new_home.to_path_buf(),
        },
    )?;
    if apply {
        fs::create_dir_all(new_home).map_err(failed(new_home))?;
    }
    let in_use = new_home.join(LIST_FILE).exists();
    for entry in sorted_entries(old_home)? {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest = new_home.join(&name);
        let stamp = !in_use && name == STAMP_ONLY;
        if holds_data(&dest) && !stamp {
            emit(sink, MigrateEvent::HomeSkip { name, dest })?;
            continue;
        }
        emit(sink, MigrateEvent::HomeMove(name))?;
        if apply {
            clear_destination(&dest).map_err(failed(&dest))?;
            move_entry(&entry, &dest).map_err(failed(&dest))?;
        }
    }
    if apply && !is_symlink(old_home) && sorted_entries(old_home)?.is_empty() {
        fs::remove_dir(old_home).map_err(failed(old_home))?;
    }
    Ok(())
}

fn read_inside(path: &Path, root: &Path) -> Option<String> {
    if points_outside(path, root) {
        return None;
    }
    read_universal(path)
}

fn has_old_block(path: &Path, root: &Path, block: &Block) -> bool {
    read_inside(path, root).is_some_and(|text| text.contains(block.old_start))
}

fn convert_file(path: &Path, root: &Path, block: &Block) -> Result<(), StoreError> {
    let Some(text) = read_inside(path, root) else {
        return Ok(());
    };
    match convert_block(&text, block) {
        Some(converted) => write_atomic(path, converted.as_bytes()),
        None => Ok(()),
    }
}

fn rewrite_paths(path: &Path, root: &Path) -> Result<(), StoreError> {
    match read_inside(path, root).and_then(|text| rewrite_legacy_paths(&text)) {
        Some(updated) => write_atomic(path, updated.as_bytes()),
        None => Ok(()),
    }
}

impl Plan {
    fn steps(&self) -> Vec<MigrateStep> {
        let mut steps = Vec::new();
        if self.rename {
            steps.push(MigrateStep::Rename);
        }
        if !self.stale.is_empty() {
            steps.push(MigrateStep::Paths(self.stale.len()));
        }
        if self.instructions {
            steps.push(MigrateStep::Instructions);
        }
        if self.gitignore {
            steps.push(MigrateStep::Gitignore);
        }
        if !self.bridges.is_empty() {
            steps.push(MigrateStep::Bridges(self.bridges.clone()));
        }
        if !self.hooks.is_empty() {
            steps.push(MigrateStep::Hooks(self.hooks.clone()));
        }
        steps
    }
}

fn blocked(legacy: &Path, target: &Path) -> Option<ProjectState> {
    if is_symlink(legacy) || is_symlink(target) {
        return Some(ProjectState::Symlink);
    }
    let rename = legacy.is_dir();
    if rename && target.exists() {
        return Some(ProjectState::BothExist);
    }
    if !rename && !target.is_dir() {
        return Some(ProjectState::NoLegacy);
    }
    rename
        .then(|| live_watcher(legacy))
        .flatten()
        .map(ProjectState::Watcher)
}

fn plan(root: &Path, legacy: &Path, target: &Path) -> Plan {
    let rename = legacy.is_dir();
    let memory = if rename { legacy } else { target };
    let hooks = hooks_dir(root);
    Plan {
        rename,
        stale: files_with_legacy_paths(memory),
        instructions: read_universal(&memory.join(INSTRUCTIONS_FILE))
            .is_some_and(|text| instructions_version(&text) < INSTRUCTIONS_VERSION),
        gitignore: read_inside(&root.join(GITIGNORE), root)
            .is_some_and(|text| has_legacy_path(&text)),
        bridges: RULE_FILES
            .into_iter()
            .filter(|file| has_old_block(&root.join(file), root, &BRIDGE_BLOCK))
            .collect(),
        hooks: HookKind::ALL
            .into_iter()
            .map(HookKind::file_name)
            .filter(|name| has_old_block(&hooks.join(name), root, &HOOK_BLOCK))
            .collect(),
    }
}

fn apply_plan(
    project: &LegacyProject,
    plan: &Plan,
    djf: &str,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), Halt> {
    let root = &project.root;
    let target = root.join(MEMORY_DIR);
    if plan.rename {
        let legacy = root.join(LEGACY_DIR);
        fs::rename(&legacy, &target).map_err(failed(&target))?;
    }
    for relative in &plan.stale {
        rewrite_paths(&target.join(relative), root)?;
    }
    if plan.gitignore {
        rewrite_paths(&root.join(GITIGNORE), root)?;
    }
    if plan.instructions {
        refresh_instructions(root)?;
    }
    if !plan.bridges.is_empty() {
        for file in &plan.bridges {
            convert_file(&root.join(file), root, &BRIDGE_BLOCK)?;
        }
        for (file, outcome) in write_bridges(root, &project.name)? {
            emit(sink, MigrateEvent::Bridge(file, outcome))?;
        }
    }
    if !plan.hooks.is_empty() {
        let directory = hooks_dir(root);
        for name in &plan.hooks {
            convert_file(&directory.join(name), root, &HOOK_BLOCK)?;
        }
        emit(sink, MigrateEvent::Hooks(install(&directory, djf)?))?;
    }
    Ok(())
}

fn migrate_project(
    project: &LegacyProject,
    options: &MigrateOptions,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), Halt> {
    let root = &project.root;
    let label = format!("Project {} ({})", project.name, root.display());
    let legacy = root.join(LEGACY_DIR);
    let target = root.join(MEMORY_DIR);
    if let Some(state) = blocked(&legacy, &target) {
        emit(sink, MigrateEvent::Project { label, state })?;
        return Ok(());
    }
    let plan = plan(root, &legacy, &target);
    let steps = plan.steps();
    if steps.is_empty() {
        emit(
            sink,
            MigrateEvent::Project {
                label,
                state: ProjectState::Migrated,
            },
        )?;
        return Ok(());
    }
    emit(
        sink,
        MigrateEvent::Project {
            label,
            state: ProjectState::Steps(steps),
        },
    )?;
    if options.apply {
        apply_plan(project, &plan, &options.djf, sink)?;
    }
    Ok(())
}

fn leftovers(
    options: &MigrateOptions,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), Halt> {
    let home = &options.clients.home;
    let mut files = client_configs(&options.clients, None);
    files.push(("Claude Code hooks", home.join(".claude/settings.json")));
    files.push(("Codex hooks", home.join(".codex/hooks.json")));
    let configs: Vec<(&'static str, PathBuf)> = files
        .into_iter()
        .filter(|(_, path)| read_universal(path).is_some_and(|text| is_stale_config(&text)))
        .collect();
    if configs.is_empty() && options.legacy_env.is_empty() {
        return Ok(());
    }
    emit(
        sink,
        MigrateEvent::Leftovers {
            configs,
            env: options.legacy_env.clone(),
        },
    )
}

fn run(
    settings: &Settings,
    options: &MigrateOptions,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), Halt> {
    let new_home = &settings.dejafix_home;
    let projects = legacy_projects(options, new_home, &now_text(settings))?;
    emit(
        sink,
        MigrateEvent::Banner {
            apply: options.apply,
        },
    )?;
    migrate_home(&options.old_home, new_home, options.apply, sink)?;
    for project in &projects {
        migrate_project(project, options, sink)?;
    }
    leftovers(options, sink)?;
    if options.apply {
        emit(sink, MigrateEvent::Done)?;
    }
    Ok(())
}

pub fn migrate(
    settings: &Settings,
    options: &MigrateOptions,
    sink: &mut impl FnMut(MigrateEvent) -> ControlFlow<()>,
) -> Result<(), WiringError> {
    match run(settings, options, sink) {
        Ok(()) | Err(Halt::Stopped) => Ok(()),
        Err(Halt::Store(error)) => Err(error.into()),
    }
}
