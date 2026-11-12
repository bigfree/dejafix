use std::path::{Path, PathBuf};

use super::load::DEFAULT_BRAIN;
use super::slug::{is_slug, slugify, unique_id};
use super::types::{ProjectRecord, RegisterResult, Registry, RegistryError};
use crate::core::text::python_strip;

#[must_use]
pub fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
    } else {
        left == right
    }
}

impl ProjectRecord {
    #[must_use]
    pub fn name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.id)
    }

    fn names_match(&self, lowered: &str) -> bool {
        lowered == self.id
            || self
                .alias
                .as_ref()
                .is_some_and(|alias| alias.to_lowercase() == lowered)
    }
}

impl Registry {
    pub fn find(
        &self,
        identifier: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Option<usize> {
        if identifier.is_empty() {
            return None;
        }
        let stripped = python_strip(identifier);
        let lowered = stripped.to_lowercase();
        let mut resolved: Option<Option<PathBuf>> = None;
        self.projects.iter().position(|record| {
            record.names_match(&lowered)
                || resolved
                    .get_or_insert_with(|| resolve(stripped))
                    .as_deref()
                    .is_some_and(|path| same_path(path, &record.path))
        })
    }

    #[must_use]
    pub fn known_names(&self) -> String {
        if self.projects.is_empty() {
            return "none registered".to_owned();
        }
        self.projects
            .iter()
            .map(ProjectRecord::name)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn require(
        &self,
        identifier: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<usize, RegistryError> {
        self.find(identifier, resolve)
            .ok_or_else(|| RegistryError::NotFound(identifier.to_owned()))
    }

    fn require_free_alias(&self, alias: &str, skip_id: Option<&str>) -> Result<(), RegistryError> {
        if !is_slug(alias) {
            return Err(RegistryError::BadAlias(alias.to_owned()));
        }
        match self.projects.iter().find(|record| {
            Some(record.id.as_str()) != skip_id
                && (record.id == alias || record.alias.as_deref() == Some(alias))
        }) {
            Some(record) => Err(RegistryError::AliasTaken {
                alias: alias.to_owned(),
                path: record.path.clone(),
            }),
            None => Ok(()),
        }
    }

    fn touched(
        &mut self,
        index: usize,
        identifier: &str,
        now: &str,
    ) -> Result<&mut ProjectRecord, RegistryError> {
        let record = self
            .projects
            .get_mut(index)
            .ok_or_else(|| RegistryError::NotFound(identifier.to_owned()))?;
        now.clone_into(&mut record.updated_at);
        Ok(record)
    }

    fn realias(
        &mut self,
        index: usize,
        identifier: &str,
        alias: &str,
        now: &str,
    ) -> Result<&ProjectRecord, RegistryError> {
        let skip = self.projects.get(index).map(|record| record.id.clone());
        self.require_free_alias(alias, skip.as_deref())?;
        let record = self.touched(index, identifier, now)?;
        record.alias = Some(alias.to_owned());
        Ok(record)
    }

    pub fn register(
        &mut self,
        resolved: PathBuf,
        alias: Option<&str>,
        now: &str,
    ) -> Result<(RegisterResult, &ProjectRecord), RegistryError> {
        let shown = resolved.to_string_lossy().into_owned();
        if let Some(index) = self
            .projects
            .iter()
            .position(|record| same_path(&record.path, &resolved))
        {
            let current = self
                .projects
                .get(index)
                .and_then(|record| record.alias.as_deref());
            if let Some(alias) = alias
                && !alias.is_empty()
                && Some(alias) != current
            {
                let record = self.realias(index, &shown, alias, now)?;
                return Ok((RegisterResult::Realiased, record));
            }
            let record = self
                .projects
                .get(index)
                .ok_or(RegistryError::NotFound(shown))?;
            return Ok((RegisterResult::Existing, record));
        }
        if let Some(alias) = alias {
            self.require_free_alias(alias, None)?;
        }
        let name = resolved
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id = unique_id(&slugify(&name), |id| {
            self.projects.iter().any(|record| record.id == id)
        });
        self.projects.push(ProjectRecord {
            id,
            path: resolved,
            alias: alias.map(str::to_owned),
            default_brain: DEFAULT_BRAIN.to_owned(),
            tags: Vec::new(),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        });
        let record = self.projects.last().ok_or(RegistryError::NotFound(shown))?;
        Ok((RegisterResult::Added, record))
    }

    pub fn set_alias(
        &mut self,
        identifier: &str,
        alias: &str,
        now: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<&ProjectRecord, RegistryError> {
        let index = self.require(identifier, resolve)?;
        self.realias(index, identifier, alias, now)
    }

    pub fn add_tag(
        &mut self,
        identifier: &str,
        tag: &str,
        now: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<(&ProjectRecord, bool), RegistryError> {
        let index = self.require(identifier, resolve)?;
        let slug = slugify(tag);
        let missing = || RegistryError::NotFound(identifier.to_owned());
        if self
            .projects
            .get(index)
            .ok_or_else(missing)?
            .tags
            .contains(&slug)
        {
            let record = self.projects.get(index).ok_or_else(missing)?;
            return Ok((record, false));
        }
        let record = self.touched(index, identifier, now)?;
        record.tags.push(slug);
        Ok((record, true))
    }

    pub fn remove_tag(
        &mut self,
        identifier: &str,
        tag: &str,
        now: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<&ProjectRecord, RegistryError> {
        let index = self.require(identifier, resolve)?;
        let slug = slugify(tag);
        let record = self.touched(index, identifier, now)?;
        record.tags.retain(|existing| *existing != slug);
        Ok(record)
    }

    pub fn set_active(
        &mut self,
        identifier: Option<&str>,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<Option<&ProjectRecord>, RegistryError> {
        let Some(identifier) = identifier else {
            self.active_project = None;
            return Ok(None);
        };
        let index =
            self.find(identifier, resolve)
                .ok_or_else(|| RegistryError::NotFoundListing {
                    identifier: identifier.to_owned(),
                    known: self.known_names(),
                })?;
        let record = self
            .projects
            .get(index)
            .ok_or_else(|| RegistryError::NotFound(identifier.to_owned()))?;
        self.active_project = Some(record.id.clone());
        Ok(Some(record))
    }

    pub fn unregister(
        &mut self,
        identifier: &str,
        resolve: impl Fn(&str) -> Option<PathBuf>,
    ) -> Result<ProjectRecord, RegistryError> {
        let index = self.require(identifier, resolve)?;
        let id = self
            .projects
            .get(index)
            .map(|record| record.id.clone())
            .ok_or_else(|| RegistryError::NotFound(identifier.to_owned()))?;
        self.remove_id(&id)
            .ok_or_else(|| RegistryError::NotFound(identifier.to_owned()))
    }

    pub fn remove_id(&mut self, id: &str) -> Option<ProjectRecord> {
        let index = self.projects.iter().position(|record| record.id == id)?;
        if self.active_project.as_deref() == Some(id) {
            self.active_project = None;
        }
        Some(self.projects.remove(index))
    }
}
