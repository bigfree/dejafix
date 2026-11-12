use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use jiff::tz::TimeZone;

use super::init::MEMORY_DIR;
use super::registry::{has_memory, now_text};
use super::types::{
    CorruptLog, DoctorOptions, DoctorReport, FoundProject, ScanReport, Settings, WiringError,
};
use crate::adapters::fs::{
    ClientPaths, ConfigState, RegistryFiles, canonical_path, dedupe_paths, default_roots,
    events_path, find_projects, identity, read_events_file, resolve_path, scan_client_configs,
};
use crate::core::doctor::{SeenConfig, track_pins};
use crate::core::log::skipped_lines;
use crate::core::registry::{RegisterResult, Registry, RegistryError, same_path};

const STATE_STAMP: &str = "%Y-%m-%d %H:%M";

fn describe(error: &dyn Error) -> String {
    match error.source() {
        Some(source) => format!("{error}: {source}"),
        None => error.to_string(),
    }
}

fn resolve_root(raw: &str, home: Option<&Path>, cwd: &Path) -> PathBuf {
    resolve_path(raw, home, cwd).unwrap_or_else(|| cwd.join(raw))
}

fn projects_under(roots: &[PathBuf], depth: usize) -> Vec<PathBuf> {
    dedupe_paths(roots.iter().flat_map(|root| find_projects(root, depth)))
}

fn register_all(
    files: &RegistryFiles,
    registry: &mut Registry,
    projects: &[PathBuf],
    now: &str,
) -> Vec<(PathBuf, String)> {
    let before = registry.clone();
    let mut failures = Vec::new();
    let mut added = false;
    for project in projects {
        let registered = canonical_path(project)
            .ok_or_else(|| RegistryError::Unresolvable(project.display().to_string()))
            .and_then(|resolved| {
                registry
                    .register(resolved, None, now)
                    .map(|(result, _)| result)
            });
        match registered {
            Ok(result) => added |= result == RegisterResult::Added,
            Err(error) => failures.push((project.clone(), error.to_string())),
        }
    }
    if added && let Err(error) = files.save(registry) {
        let message = describe(&error);
        failures = projects
            .iter()
            .map(|project| (project.clone(), message.clone()))
            .collect();
        *registry = before;
    }
    failures
}

fn corrupt_logs(settings: &Settings, registry: &Registry) -> Vec<CorruptLog> {
    registry
        .projects
        .iter()
        .filter_map(|record| {
            let memory = record.path.join(MEMORY_DIR);
            let bytes = read_events_file(&memory).ok()?;
            let lines = skipped_lines(&bytes, &settings.clock);
            (!lines.is_empty()).then(|| CorruptLog {
                path: events_path(&memory),
                lines,
            })
        })
        .collect()
}

fn state_stamp(settings: &Settings) -> String {
    settings
        .clock
        .now
        .to_zoned(TimeZone::UTC)
        .strftime(STATE_STAMP)
        .to_string()
}

fn check_configs(
    settings: &Settings,
    files: &RegistryFiles,
    paths: &ClientPaths,
    cwd: &Path,
    report: &mut DoctorReport,
) {
    let configs = scan_client_configs(paths, Some(cwd));
    let seen: Vec<SeenConfig<'_>> = configs
        .iter()
        .filter(|config| config.state != ConfigState::Unreadable)
        .map(|config| SeenConfig {
            client: config.client,
            key: config.path.display().to_string(),
            pinned: config.state == ConfigState::Pinned,
        })
        .collect();
    let mut state = files.doctor_state();
    report.reverted = track_pins(&mut state, &seen, &state_stamp(settings));
    let _ = files.save_doctor_state(state);
    let (unreadable, rest): (Vec<_>, Vec<_>) = configs
        .into_iter()
        .partition(|config| config.state == ConfigState::Unreadable);
    report.unreadable = unreadable;
    report.pinned = rest
        .into_iter()
        .filter(|config| config.state == ConfigState::Pinned)
        .collect();
}

pub fn doctor(
    settings: &Settings,
    paths: &ClientPaths,
    cwd: &Path,
    options: &DoctorOptions,
) -> Result<DoctorReport, WiringError> {
    let home = Some(paths.home.as_path());
    let roots: Vec<PathBuf> = match &options.roots {
        Some(raw) => raw
            .iter()
            .map(|root| resolve_root(root, home, cwd))
            .collect(),
        None => default_roots(&paths.home),
    };
    let found = projects_under(&roots, options.depth);
    let files = RegistryFiles::new(&settings.dejafix_home);
    let now = now_text(settings);
    let mut registry = files.load(&now)?;
    let known: HashSet<_> = registry
        .projects
        .iter()
        .filter_map(|record| identity(&record.path))
        .collect();
    let mut report = DoctorReport {
        roots: roots.len(),
        found: found.len(),
        missing: found
            .iter()
            .filter(|project| identity(project).is_some_and(|key| !known.contains(&key)))
            .cloned()
            .collect(),
        stale: registry
            .projects
            .iter()
            .filter(|record| !has_memory(&record.path))
            .cloned()
            .collect(),
        ..DoctorReport::default()
    };
    if options.fix {
        if !report.missing.is_empty() {
            report.register_failures = register_all(&files, &mut registry, &report.missing, &now);
        }
        if !report.stale.is_empty() {
            for record in &report.stale {
                registry.remove_id(&record.id);
            }
            if let Err(error) = files.save(&registry) {
                report.prune_failure = Some(describe(&error));
            }
        }
    }
    check_configs(settings, &files, paths, cwd, &mut report);
    report.corrupt = corrupt_logs(settings, &registry);
    Ok(report)
}

pub fn scan_projects(
    settings: &Settings,
    cwd: &Path,
    home: Option<&Path>,
    paths: &[String],
    depth: usize,
    dry_run: bool,
) -> Result<ScanReport, WiringError> {
    let here = [".".to_owned()];
    let raw = if paths.is_empty() { &here[..] } else { paths };
    let roots: Vec<PathBuf> = raw
        .iter()
        .map(|root| resolve_root(root, home, cwd))
        .collect();
    if let Some(root) = roots.iter().find(|root| !root.is_dir()) {
        return Ok(ScanReport::NotADirectory(root.clone()));
    }
    let projects = projects_under(&roots, depth);
    if projects.is_empty() {
        return Ok(ScanReport::Scanned {
            roots,
            found: Vec::new(),
            failures: Vec::new(),
        });
    }
    let files = RegistryFiles::new(&settings.dejafix_home);
    let now = now_text(settings);
    let mut registry = files.load(&now)?;
    let found: Vec<FoundProject> = projects
        .into_iter()
        .map(|path| FoundProject {
            known: registry
                .projects
                .iter()
                .any(|record| same_path(&record.path, &path)),
            path,
        })
        .collect();
    let fresh: Vec<PathBuf> = found
        .iter()
        .filter(|project| !project.known)
        .map(|project| project.path.clone())
        .collect();
    let failures = if dry_run || fresh.is_empty() {
        Vec::new()
    } else {
        register_all(&files, &mut registry, &fresh, &now)
    };
    Ok(ScanReport::Scanned {
        roots,
        found,
        failures,
    })
}
