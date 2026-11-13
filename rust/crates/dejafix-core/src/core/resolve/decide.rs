use std::path::{Path, PathBuf};

use super::types::{Probe, Resolution, ResolveError, Source};
use crate::core::registry::{ProjectRecord, Registry};

impl Source {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Explicit => "explicit",
            Self::Roots => "roots",
            Self::Active => "active",
            Self::Cwd => "cwd",
            Self::Only => "only",
        }
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn find<'r>(registry: &'r Registry, text: &str, probe: &impl Probe) -> Option<&'r ProjectRecord> {
    registry
        .find(text, |path| probe.lookup(path))
        .and_then(|index| registry.projects.get(index))
}

fn named(registry: &Registry, root: &Path, probe: &impl Probe) -> String {
    find(registry, &root.to_string_lossy(), probe)
        .map_or_else(|| folder_name(root), |record| record.name().to_owned())
}

fn from_record(
    record: &ProjectRecord,
    source: Source,
    probe: &impl Probe,
) -> Result<Resolution, ResolveError> {
    if !probe.has_memory(&record.path) {
        return Err(ResolveError::RegisteredWithoutMemory {
            name: record.name().to_owned(),
            path: record.path.clone(),
        });
    }
    Ok(Resolution {
        root: record.path.clone(),
        source,
        name: record.name().to_owned(),
    })
}

fn pinned_to(
    registry: &Registry,
    explicit: Option<&str>,
    pinned: &Path,
    probe: &impl Probe,
) -> Result<Resolution, ResolveError> {
    if !probe.has_memory(pinned) {
        return Err(ResolveError::PinnedWithoutMemory(pinned.to_path_buf()));
    }
    let name = named(registry, pinned, probe);
    if let Some(explicit) = explicit.filter(|explicit| !explicit.is_empty()) {
        let wanted = find(registry, explicit, probe);
        let same = wanted.is_some_and(|record| record.path == pinned)
            || explicit == name
            || explicit == pinned.to_string_lossy();
        if !same {
            return Err(ResolveError::PinnedMismatch {
                name,
                pinned: pinned.to_path_buf(),
                explicit: explicit.to_owned(),
            });
        }
    }
    Ok(Resolution {
        root: pinned.to_path_buf(),
        source: Source::Pinned,
        name,
    })
}

fn explicitly(
    registry: &Registry,
    explicit: &str,
    probe: &impl Probe,
) -> Result<Resolution, ResolveError> {
    if let Some(record) = find(registry, explicit, probe) {
        return from_record(record, Source::Explicit, probe);
    }
    let candidate = probe.expand_user(explicit);
    if probe.is_dir(&candidate) {
        if !probe.has_memory(&candidate) {
            return Err(ResolveError::NoMemory(candidate));
        }
        let root = probe
            .resolve(&candidate)
            .unwrap_or_else(|| candidate.clone());
        return Ok(Resolution {
            root,
            source: Source::Explicit,
            name: folder_name(&candidate),
        });
    }
    Err(ResolveError::Unknown {
        explicit: explicit.to_owned(),
        known: registry.known_names(),
    })
}

pub fn resolve(
    registry: &Registry,
    explicit: Option<&str>,
    pinned: Option<&Path>,
    roots: &[PathBuf],
    probe: &impl Probe,
) -> Result<Resolution, ResolveError> {
    if let Some(pinned) = pinned {
        return pinned_to(registry, explicit, pinned, probe);
    }
    if let Some(explicit) = explicit.filter(|explicit| !explicit.is_empty()) {
        return explicitly(registry, explicit, probe);
    }
    let usable: Vec<PathBuf> = roots
        .iter()
        .filter_map(|root| probe.resolve(&probe.expand_user(&root.to_string_lossy())))
        .filter(|root| probe.has_memory(root))
        .collect();
    match usable.as_slice() {
        [root] => {
            return Ok(Resolution {
                root: root.clone(),
                source: Source::Roots,
                name: named(registry, root, probe),
            });
        }
        [] => {}
        several => {
            let names: Vec<String> = several.iter().map(|root| folder_name(root)).collect();
            return Err(ResolveError::SeveralRoots(names.join(", ")));
        }
    }
    if let Some(active) = registry
        .active_project
        .as_deref()
        .and_then(|active| find(registry, active, probe))
    {
        return from_record(active, Source::Active, probe);
    }
    if let Some(root) = probe.discover() {
        let name = named(registry, &root, probe);
        return Ok(Resolution {
            root,
            source: Source::Cwd,
            name,
        });
    }
    if let [only] = registry.projects.as_slice() {
        return from_record(only, Source::Only, probe);
    }
    Err(ResolveError::NothingSelected(registry.known_names()))
}
