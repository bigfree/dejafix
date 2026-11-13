use std::collections::{BTreeSet, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use jiff::tz::TimeZone;
use serde_json::{Map, Value};

use super::store::read_python_text;
use super::types::{GlobalStore, StoreError};
use super::write::write_text;
use crate::core::global::{Entry, PROMOTABLE_SEED, is_prunable};
use crate::core::json::{write_canonical, write_python_json_indented};
use crate::core::text::{platform_newlines, python_lines, python_strip, universal_newlines};

const PATTERNS_FILE: &str = "patterns.jsonl";
const GOTCHAS_FILE: &str = "library_gotchas.jsonl";
const PREFERENCES_FILE: &str = "stack_preferences.md";
const PROMOTABLE_FILE: &str = ".promotable.json";

fn write_error(path: &Path) -> impl Fn(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

pub fn read_json_text(path: &Path) -> Result<Option<String>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }
    read_python_text(path, true)
        .map(Some)
        .map_err(|source| StoreError::Read {
            path: path.to_path_buf(),
            source,
        })
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

    pub fn ensure(&self) -> Result<(), StoreError> {
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

    fn load(&self, name: &str) -> Result<Vec<Entry>, StoreError> {
        self.ensure()?;
        let path = self.path(name);
        match read_python_text(&path, false) {
            Ok(text) => parse_entries(&text).ok_or_else(|| StoreError::Read {
                path,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "a line is not a JSON object",
                ),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(source) => Err(StoreError::Read { path, source }),
        }
    }

    pub fn load_patterns(&self) -> Result<Vec<Entry>, StoreError> {
        self.load(PATTERNS_FILE)
    }

    pub fn load_gotchas(&self) -> Result<Vec<Entry>, StoreError> {
        self.load(GOTCHAS_FILE)
    }

    #[must_use]
    pub fn has_preferences(&self) -> bool {
        self.path(PREFERENCES_FILE).exists()
    }

    fn canonical_lines(path: &Path, entries: &[&Entry]) -> Result<String, StoreError> {
        let mut bytes = Vec::new();
        for entry in entries {
            write_canonical(&mut bytes, &Value::Object((*entry).clone()))
                .map_err(|error| write_error(path)(error.into()))?;
            bytes.push(b'\n');
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn append(&self, name: &str, entry: &Entry) -> Result<(), StoreError> {
        self.ensure()?;
        let path = self.path(name);
        let text = Self::canonical_lines(&path, &[entry])?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(platform_newlines(&text).as_bytes()))
            .map_err(write_error(&path))
    }

    pub fn append_pattern(&self, entry: &Entry) -> Result<(), StoreError> {
        self.append(PATTERNS_FILE, entry)
    }

    pub fn append_gotcha(&self, entry: &Entry) -> Result<(), StoreError> {
        self.append(GOTCHAS_FILE, entry)
    }

    fn retain_locked(
        &self,
        name: &str,
        mut keep: impl FnMut(&Entry) -> bool,
    ) -> Result<usize, StoreError> {
        self.ensure()?;
        let path = self.path(name);
        let read_error = |source| StoreError::Read {
            path: path.clone(),
            source,
        };
        let mut file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(read_error(error)),
        };
        file.lock().map_err(|source| StoreError::Lock {
            path: path.clone(),
            source,
        })?;
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(read_error)?;
        let entries = parse_entries(&universal_newlines(&text)).ok_or_else(|| {
            read_error(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a line is not a JSON object",
            ))
        })?;
        let kept: Vec<&Entry> = entries.iter().filter(|entry| keep(entry)).collect();
        let removed = entries.len() - kept.len();
        if removed == 0 {
            return Ok(0);
        }
        let lines = Self::canonical_lines(&path, &kept)?;
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)))
            .and_then(|_| file.write_all(platform_newlines(&lines).as_bytes()))
            .map_err(write_error(&path))?;
        Ok(removed)
    }

    pub fn remove(&self, id: &str) -> Result<bool, StoreError> {
        for name in [PATTERNS_FILE, GOTCHAS_FILE] {
            let removed = self.retain_locked(name, |entry| {
                entry.get("id").and_then(Value::as_str) != Some(id)
            })?;
            if removed > 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn prune(
        &self,
        now: jiff::Timestamp,
        zone: &TimeZone,
        older_than: i64,
        confidence: Option<&str>,
    ) -> Result<usize, StoreError> {
        let mut removed = 0;
        for name in [PATTERNS_FILE, GOTCHAS_FILE] {
            removed += self.retain_locked(name, |entry| {
                !is_prunable(entry, now, zone, older_than, confidence)
            })?;
        }
        Ok(removed)
    }

    pub fn export(&self, exported_at: &str) -> Result<Value, StoreError> {
        let path = self.path(PREFERENCES_FILE);
        let preferences = match read_python_text(&path, false) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => return Err(StoreError::Read { path, source }),
        };
        Ok(serde_json::json!({
            "version": "1.0",
            "exported_at": exported_at,
            "patterns": self.load_patterns()?,
            "gotchas": self.load_gotchas()?,
            "preferences": preferences,
        }))
    }

    pub fn import(&self, data: &Map<String, Value>) -> Result<(usize, usize), StoreError> {
        let known = |entries: Vec<Entry>| -> HashSet<String> {
            entries
                .iter()
                .filter_map(|entry| entry.get("id"))
                .map(Value::to_string)
                .collect()
        };
        let known_patterns = known(self.load_patterns()?);
        let known_gotchas = known(self.load_gotchas()?);
        let mut added = [0_usize; 2];
        for (slot, (key, name, known)) in added.iter_mut().zip([
            ("patterns", PATTERNS_FILE, &known_patterns),
            ("gotchas", GOTCHAS_FILE, &known_gotchas),
        ]) {
            let Some(Value::Array(items)) = data.get(key) else {
                continue;
            };
            for entry in items.iter().filter_map(Value::as_object) {
                if entry
                    .get("id")
                    .is_some_and(|id| known.contains(&id.to_string()))
                {
                    continue;
                }
                self.append(name, entry)?;
                *slot += 1;
            }
        }
        if let Some(Value::String(preferences)) = data.get("preferences")
            && !preferences.is_empty()
            && !self.has_preferences()
        {
            write_text(&self.path(PREFERENCES_FILE), preferences)?;
        }
        let [patterns, gotchas] = added;
        Ok((patterns, gotchas))
    }

    pub fn append_gotcha_if_new(
        &self,
        decide: impl FnOnce(&[Entry]) -> Option<Entry>,
    ) -> Result<bool, StoreError> {
        self.ensure()?;
        let path = self.path(GOTCHAS_FILE);
        let opened = OpenOptions::new().read(true).append(true).open(&path);
        let (mut file, entry) = match opened {
            Ok(mut file) => {
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
                (file, entry)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(entry) = decide(&[]) else {
                    return Ok(false);
                };
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
                if file.read_to_string(&mut text).is_err() || !text.is_empty() {
                    return Ok(false);
                }
                (file, entry)
            }
            Err(error) => return Err(write_error(&path)(error)),
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
