use std::path::{Path, PathBuf};

use serde_json::Value;

use super::init::MEMORY_DIR;
use super::types::{
    FsProbe, ListedProject, RegistryOp, RegistryReply, Settings, UpgradeNotice, WiringError,
};
use crate::adapters::fs::{RegistryFiles, StoreError, identity, resolve_path};
use crate::core::registry::{
    PathLookup, ProjectRecord, RegisterResult, Registry, RegistryError, same_path,
};
use crate::core::time::Timestamp;

pub(super) fn now_text(settings: &Settings) -> String {
    Timestamp::now(&settings.clock).as_str().to_owned()
}

pub(super) fn load_registry(settings: &Settings) -> Result<Registry, StoreError> {
    RegistryFiles::new(&settings.dejafix_home).load(&now_text(settings))
}

pub(super) fn has_memory(path: &Path) -> bool {
    path.join(MEMORY_DIR).is_dir()
}

impl PathLookup for FsProbe<'_> {
    fn locate(&self, text: &str) -> Option<PathBuf> {
        resolve_path(text, self.home, self.cwd)
    }

    fn same_folder(&self, stored: &Path, wanted: &Path) -> bool {
        same_path(stored, wanted)
            || identity(wanted).is_some_and(|wanted| identity(stored) == Some(wanted))
    }
}

pub fn register_project(
    settings: &Settings,
    cwd: &Path,
    home: Option<&Path>,
    path: &str,
    alias: Option<&str>,
) -> Result<ProjectRecord, WiringError> {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let now = now_text(settings);
    let root = resolve_path(path, home, cwd)
        .ok_or_else(|| RegistryError::Unresolvable(path.to_owned()))?;
    if !has_memory(&root) {
        return Err(RegistryError::NoMemory { root }.into());
    }
    let mut registry = files.load(&now)?;
    let (result, record) = registry.register(root, alias, &now, &FsProbe { cwd, home })?;
    let record = record.clone();
    if result != RegisterResult::Existing {
        files.save(&registry)?;
    }
    Ok(record)
}

pub fn registry_op(
    settings: &Settings,
    cwd: &Path,
    home: Option<&Path>,
    op: RegistryOp,
) -> Result<RegistryReply, WiringError> {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let now = now_text(settings);
    let lookup = FsProbe { cwd, home };
    let mut registry = files.load(&now)?;
    let (reply, changed) = match op {
        RegistryOp::List => {
            return Ok(RegistryReply::Listed {
                active: registry.active_project,
                projects: registry
                    .projects
                    .into_iter()
                    .map(|record| ListedProject {
                        has_memory: has_memory(&record.path),
                        record,
                    })
                    .collect(),
            });
        }
        RegistryOp::Use(identifier) => {
            let record = registry.set_active(identifier.as_deref(), &lookup)?;
            (RegistryReply::Activated(record.cloned()), true)
        }
        RegistryOp::Remove(identifier) => (
            RegistryReply::Removed(registry.unregister(&identifier, &lookup)?),
            true,
        ),
        RegistryOp::Alias { identifier, alias } => {
            let record = registry.set_alias(&identifier, &alias, &now, &lookup)?;
            (RegistryReply::Aliased(record.clone()), true)
        }
        RegistryOp::Tag {
            identifier,
            tag,
            remove: true,
        } => {
            let record = registry.remove_tag(&identifier, &tag, &now, &lookup)?;
            (RegistryReply::Tagged(record.clone()), true)
        }
        RegistryOp::Tag {
            identifier,
            tag,
            remove: false,
        } => {
            let (record, changed) = registry.add_tag(&identifier, &tag, &now, &lookup)?;
            (RegistryReply::Tagged(record.clone()), changed)
        }
    };
    if changed {
        files.save(&registry)?;
    }
    Ok(reply)
}

#[must_use]
pub fn upgrade_notice(settings: &Settings, version: &str) -> Option<UpgradeNotice> {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let previous = files.seen_version();
    if previous.as_ref().and_then(Value::as_str) == Some(version) {
        return None;
    }
    files.record_version(version);
    match previous {
        None | Some(Value::Null) => None,
        Some(_) => Some(UpgradeNotice {
            version: version.to_owned(),
        }),
    }
}
