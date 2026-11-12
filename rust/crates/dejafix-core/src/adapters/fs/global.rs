use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::store::read_python_text;
use super::types::{GlobalStore, StoreError};
use super::write::write_text;
use crate::core::global::{Entry, PROMOTABLE_SEED};
use crate::core::json::{write_canonical, write_python_json_indented};
use crate::core::text::{platform_newlines, python_lines, python_strip, universal_newlines};

const PATTERNS_FILE: &str = "patterns.jsonl";
const GOTCHAS_FILE: &str = "library_gotchas.jsonl";
const PROMOTABLE_FILE: &str = ".promotable.json";

fn write_error(path: &Path) -> impl Fn(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

fn parse_entries(text: &str) -> Option<Vec<Entry>> {
    python_lines(text.as_bytes())
        .filter_map(|(_, line)| std::str::from_utf8(line).ok())
        .filter(|line| !python_strip(line).is_empty())
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(entry)) => Some(entry),
            _ => None,
        })
        .collect()
}

fn library_names(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_lowercase)
                .collect(),
        ),
        Some(Value::Object(map)) => Some(
            map.keys()
                .filter(|name| !name.is_empty())
                .map(|name| name.to_lowercase())
                .collect(),
        ),
        Some(Value::String(text)) => {
            Some(text.chars().map(|c| c.to_lowercase().collect()).collect())
        }
        Some(Value::Null | Value::Bool(_) | Value::Number(_)) => None,
    }
}

impl GlobalStore {
    #[must_use]
    pub fn new(dejafix_home: &Path) -> Self {
        Self {
            dir: dejafix_home.join("global"),
        }
    }

    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn ensure(&self) -> Result<(), StoreError> {
        fs::create_dir_all(&self.dir).map_err(write_error(&self.dir))
    }

    fn read(&self, name: &str, strip_bom: bool) -> Result<Option<String>, ()> {
        match read_python_text(&self.path(name), strip_bom) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(()),
        }
    }

    #[must_use]
    pub fn promotable(&self) -> Option<Vec<String>> {
        let _ = self.ensure();
        let text = match self.read(PROMOTABLE_FILE, true) {
            Ok(Some(text)) => text,
            Ok(None) => return Some(Vec::new()),
            Err(()) => return None,
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(data)) => library_names(data.get("libraries")),
            Ok(_) => None,
            Err(_) => Some(Vec::new()),
        }
    }

    pub fn record_promotable(&self, libraries: &[String]) -> Result<(), StoreError> {
        let fresh: BTreeSet<String> = libraries
            .iter()
            .filter(|library| !library.is_empty())
            .map(|library| library.to_lowercase())
            .collect();
        if fresh.is_empty() {
            return Ok(());
        }
        let Some(cached) = self.promotable() else {
            return Ok(());
        };
        let mut known: BTreeSet<String> = PROMOTABLE_SEED
            .iter()
            .map(|library| (*library).to_owned())
            .chain(cached)
            .collect();
        if fresh.is_subset(&known) {
            return Ok(());
        }
        known.extend(fresh);
        let path = self.path(PROMOTABLE_FILE);
        let value = serde_json::json!({"libraries": known.into_iter().collect::<Vec<_>>()});
        let mut out = Vec::new();
        write_python_json_indented(&mut out, &value, 2, true)
            .map_err(|error| write_error(&path)(error.into()))?;
        self.ensure()?;
        write_text(&path, &String::from_utf8_lossy(&out))
    }

    #[must_use]
    pub fn has_entries(&self) -> bool {
        let _ = self.ensure();
        self.path(PATTERNS_FILE).exists() || self.path(GOTCHAS_FILE).exists()
    }

    #[must_use]
    pub fn gotchas(&self) -> Option<Vec<Entry>> {
        self.entries(GOTCHAS_FILE)
    }

    #[must_use]
    pub fn patterns(&self) -> Option<Vec<Entry>> {
        self.entries(PATTERNS_FILE)
    }

    fn entries(&self, name: &str) -> Option<Vec<Entry>> {
        match self.read(name, false) {
            Ok(Some(text)) => parse_entries(&text),
            Ok(None) => Some(Vec::new()),
            Err(()) => None,
        }
    }

    pub fn append_gotcha_if_new(
        &self,
        decide: impl FnOnce(&[Entry]) -> Option<Entry>,
    ) -> Result<bool, StoreError> {
        self.ensure()?;
        let path = self.path(GOTCHAS_FILE);
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(write_error(&path))?;
        file.lock().map_err(|source| StoreError::Lock {
            path: path.clone(),
            source,
        })?;
        let mut text = String::new();
        if file.read_to_string(&mut text).is_err() {
            return Ok(false);
        }
        let Some(existing) = parse_entries(&universal_newlines(&text)) else {
            return Ok(false);
        };
        let Some(entry) = decide(&existing) else {
            return Ok(false);
        };
        let mut line = Vec::new();
        write_canonical(&mut line, &Value::Object(entry))
            .map_err(|error| write_error(&path)(error.into()))?;
        line.push(b'\n');
        let line = String::from_utf8_lossy(&line).into_owned();
        file.write_all(platform_newlines(&line).as_bytes())
            .map_err(write_error(&path))?;
        Ok(true)
    }
}
