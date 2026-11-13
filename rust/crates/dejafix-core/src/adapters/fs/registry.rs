use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

use super::types::{RegistryFiles, StoreError};
use super::write::write_text;
use crate::core::json::write_python_json_indented;
use crate::core::registry::{Loaded, Registry, list_payload, load, meta_payload};
use crate::core::text::platform_newlines;

const LIST_FILE: &str = "projects.json";
const META_FILE: &str = "projects.meta.json";
const BACKUP_FILE: &str = "projects.json.bak";
const LAST_VERSION: &str = "last_version";
const DOCTOR_STATE: &str = "doctor-state.json";

fn read_json(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    serde_json::from_slice(body).ok()
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), StoreError> {
    let failed = |source: io::Error| StoreError::Write {
        path: path.to_path_buf(),
        source,
    };
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).map_err(failed)?;
    let mut text = Vec::new();
    write_python_json_indented(&mut text, value, 1, false).map_err(|error| failed(error.into()))?;
    text.push(b'\n');
    let text = String::from_utf8_lossy(&text);
    let mut temporary = tempfile::Builder::new()
        .prefix(".pm-")
        .suffix(".json")
        .tempfile_in(directory)
        .map_err(failed)?;
    temporary
        .write_all(platform_newlines(&text).as_bytes())
        .map_err(failed)?;
    temporary
        .persist(path)
        .map(drop)
        .map_err(|error| failed(error.error))
}

impl RegistryFiles {
    #[must_use]
    pub fn new(dejafix_home: &Path) -> Self {
        Self {
            list: dejafix_home.join(LIST_FILE),
            meta: dejafix_home.join(META_FILE),
        }
    }

    #[must_use]
    pub fn read(&self) -> (Option<Value>, Option<Value>) {
        (read_json(&self.list), read_json(&self.meta))
    }

    pub fn load(&self, now: &str) -> Result<Registry, StoreError> {
        let (list, meta) = self.read();
        let loaded = load(list.as_ref(), meta.as_ref(), now, |raw| {
            Path::new(raw).is_absolute()
        });
        match loaded {
            Loaded::Clean(registry) => Ok(registry),
            Loaded::Dirty(registry) => {
                self.save(&registry)?;
                Ok(registry)
            }
            Loaded::Legacy(registry) => {
                self.backup_once();
                self.save(&registry)?;
                Ok(registry)
            }
        }
    }

    pub fn save(&self, registry: &Registry) -> Result<(), StoreError> {
        write_json_atomic(&self.list, &list_payload(registry))?;
        write_json_atomic(&self.meta, &meta_payload(registry))
    }

    fn backup_once(&self) {
        let backup = self.list.with_file_name(BACKUP_FILE);
        if self.list.exists()
            && !backup.exists()
            && let Ok(bytes) = fs::read(&self.list)
        {
            let _ = fs::write(backup, bytes);
        }
    }

    #[must_use]
    pub fn read_meta(&self) -> Map<String, Value> {
        match read_json(&self.meta) {
            Some(Value::Object(meta)) => meta,
            _ => Map::new(),
        }
    }

    #[must_use]
    pub fn seen_version(&self) -> Option<Value> {
        self.read_meta().remove(LAST_VERSION)
    }

    #[must_use]
    pub fn doctor_state(&self) -> Map<String, Value> {
        match read_json(&self.list.with_file_name(DOCTOR_STATE)) {
            Some(Value::Object(state)) => state,
            _ => Map::new(),
        }
    }

    pub fn save_doctor_state(&self, state: Map<String, Value>) -> Result<(), StoreError> {
        let path = self.list.with_file_name(DOCTOR_STATE);
        let failed = |source: io::Error| StoreError::Write {
            path: path.clone(),
            source,
        };
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory).map_err(failed)?;
        }
        let mut text = Vec::new();
        write_python_json_indented(&mut text, &Value::Object(state), 2, true)
            .map_err(|error| failed(error.into()))?;
        write_text(&path, &String::from_utf8_lossy(&text))
    }

    pub fn save_meta(&self, meta: Map<String, Value>) -> Result<(), StoreError> {
        write_json_atomic(&self.meta, &Value::Object(meta))
    }

    pub fn record_version(&self, version: &str) {
        let mut meta = self.read_meta();
        meta.insert(LAST_VERSION.to_owned(), Value::String(version.to_owned()));
        let _ = self.save_meta(meta);
    }
}

fn expand_user(path: &str, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) if rest.starts_with(['/', std::path::MAIN_SEPARATOR]) => {
            home.join(rest.trim_start_matches(['/', std::path::MAIN_SEPARATOR]))
        }
        _ => PathBuf::from(path),
    }
}

fn without_verbatim(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

#[must_use]
pub fn canonical_path(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok().map(without_verbatim)
}

#[must_use]
pub fn resolve_path(path: &str, home: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    let expanded = expand_user(path, home);
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    let (existing, ancestor) = absolute
        .ancestors()
        .find_map(|ancestor| Some((fs::canonicalize(ancestor).ok()?, ancestor)))?;
    let tail = absolute.strip_prefix(ancestor).ok()?;
    let mut resolved = without_verbatim(existing);
    for component in tail.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Some(resolved)
}
