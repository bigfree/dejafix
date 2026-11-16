use std::path::Path;

use crate::adapters::fs::{RegistryFiles, posix, project_name, read_origin, read_project_links};
use crate::core::lessons::main_checkout;
use crate::core::links::{
    ConfigError, LinkResolution, ProjectLinks, RELATION_CHARS, Registered, project_identity,
    resolve_links, sanitize,
};
use crate::core::registry::{Loaded, PathLookup, ProjectRecord, Registry, load, same_path};
use crate::wiring::registry::has_memory;
use crate::wiring::types::FsProbe;

#[derive(Debug, Clone)]
pub struct LinkedProjects {
    pub own: Registered,
    pub config_error: Option<ConfigError>,
    pub resolution: LinkResolution,
}

pub(super) fn read_registry(dejafix_home: &Path) -> Registry {
    let (list, meta) = RegistryFiles::new(dejafix_home).read();
    match load(list.as_ref(), meta.as_ref(), "", |raw| {
        Path::new(raw).is_absolute()
    }) {
        Loaded::Clean(registry) | Loaded::Dirty(registry) | Loaded::Legacy(registry) => registry,
    }
}

fn registered_at(
    path: &Path,
    id: &str,
    alias: Option<&str>,
    config: Result<ProjectLinks, ConfigError>,
) -> (Registered, Option<ConfigError>) {
    let (config, error) = match config {
        Ok(config) => (config, None),
        Err(error) => (ProjectLinks::default(), Some(error)),
    };
    let spelled = format!("{}/", posix(path));
    let main = main_checkout(&spelled);
    let worktree = main != spelled;
    let folder = if worktree {
        project_name(Path::new(main.trim_end_matches('/')))
    } else {
        project_name(path)
    };
    let project = Registered {
        id: id.to_owned(),
        alias: alias.map(str::to_owned),
        identity: sanitize(
            &project_identity(read_origin(path).as_deref(), &folder),
            RELATION_CHARS,
        ),
        folder,
        path: path.to_path_buf(),
        worktree,
        config,
    };
    (project, error)
}

fn registered(record: &ProjectRecord) -> (Registered, Option<ConfigError>) {
    registered_at(
        &record.path,
        &record.id,
        record.alias.as_deref(),
        read_project_links(&record.path),
    )
}

fn resolved(
    own: Registered,
    config_error: Option<ConfigError>,
    others: &[Registered],
) -> LinkedProjects {
    let resolution = if config_error.is_some() {
        LinkResolution::default()
    } else {
        resolve_links(&own, others)
    };
    LinkedProjects {
        own,
        config_error,
        resolution,
    }
}

fn with_memory(registry: &Registry) -> impl Iterator<Item = &ProjectRecord> {
    registry
        .projects
        .iter()
        .filter(|record| has_memory(&record.path))
}

#[must_use]
pub fn project_links(dejafix_home: &Path, root: &Path) -> LinkedProjects {
    links_with(dejafix_home, root, read_project_links(root))
}

pub(super) fn links_with(
    dejafix_home: &Path,
    root: &Path,
    config: Result<ProjectLinks, ConfigError>,
) -> LinkedProjects {
    let registry = read_registry(dejafix_home);
    let probe = FsProbe {
        cwd: root,
        home: None,
    };
    let own_record = registry
        .projects
        .iter()
        .find(|record| probe.same_folder(&record.path, root));
    let id = own_record.map_or_else(|| project_name(root), |record| record.id.clone());
    let (own, config_error) = registered_at(
        root,
        &id,
        own_record.and_then(|record| record.alias.as_deref()),
        config,
    );
    let others: Vec<Registered> = if config_error.is_some() {
        Vec::new()
    } else {
        with_memory(&registry)
            .filter(|record| {
                own_record.is_none_or(|own| own.path != record.path)
                    && !same_path(&record.path, root)
            })
            .map(|record| registered(record).0)
            .collect()
    };
    resolved(own, config_error, &others)
}

pub(super) fn every_project_links(dejafix_home: &Path) -> Vec<LinkedProjects> {
    let registry = read_registry(dejafix_home);
    let (projects, errors): (Vec<Registered>, Vec<Option<ConfigError>>) =
        with_memory(&registry).map(registered).unzip();
    projects
        .iter()
        .zip(errors)
        .map(|(own, error)| resolved(own.clone(), error, &projects))
        .collect()
}
