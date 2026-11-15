use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use jiff::tz::TimeZone;
use serde_json::{Map, Value};

use super::store::read_python_text;
use super::types::{GlobalStore, GotchaWrite, StoreError};
use super::write::{write_atomic, write_text};
use crate::core::global::{Entry, Libraries, StoredGotcha, is_prunable, stored_line};
use crate::core::json::{write_canonical, write_python_json_indented};
use crate::core::text::{platform_newlines, python_lines, python_strip, universal_newlines};

const PATTERNS_FILE: &str = "patterns.jsonl";
const GOTCHAS_FILE: &str = "library_gotchas.jsonl";
const PREFERENCES_FILE: &str = "stack_preferences.md";
const LIBRARIES_FILE: &str = "libraries.json";
const BACKUP_FILE: &str = "library_gotchas.v1.jsonl";
const LOCK_SUFFIX: &str = ".lock";

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

fn invalid_store(path: &Path) -> StoreError {
    StoreError::Read {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "a line is not a JSON object",
        ),
    }
}

fn apply(path: &Path, write: GotchaWrite) -> Result<(), StoreError> {
    match write {
        GotchaWrite::Keep => Ok(()),
        GotchaWrite::Append(lines) => {
            let text = joined_lines(&lines);
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .and_then(|mut file| file.write_all(platform_newlines(&text).as_bytes()))
                .map_err(write_error(path))
        }
        GotchaWrite::Replace(lines) => {
            let text = joined_lines(&lines);
            write_atomic(path, platform_newlines(&text).as_bytes())
        }
    }
}

fn joined_lines(lines: &[String]) -> String {
    let mut text = String::with_capacity(lines.iter().map(|line| line.len() + 1).sum());
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
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

    fn split_lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    pub fn append_pattern(&self, entry: &Entry) -> Result<(), StoreError> {
        let lines = Self::split_lines(&Self::canonical_lines(&self.path(PATTERNS_FILE), &[entry])?);
        self.change_locked(PATTERNS_FILE, true, |_| {
            (GotchaWrite::Append(lines.clone()), ())
        })
        .map(drop)
    }

    fn change_locked<T>(
        &self,
        name: &str,
        create: bool,
        mut decide: impl FnMut(Vec<Entry>) -> (GotchaWrite, T),
    ) -> Result<Option<T>, StoreError> {
        self.ensure()?;
        let path = self.path(name);
        let lock_path = self.path(&format!("{name}{LOCK_SUFFIX}"));
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(write_error(&lock_path))?;
        lock.lock().map_err(|source| StoreError::Lock {
            path: path.clone(),
            source,
        })?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                return Ok(None);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => {
                return Err(StoreError::Read {
                    path: path.clone(),
                    source,
                });
            }
        };
        let entries =
            parse_entries(&universal_newlines(&text)).ok_or_else(|| invalid_store(&path))?;
        let (write, value) = decide(entries);
        apply(&path, write)?;
        Ok(Some(value))
    }

    fn retain_locked(
        &self,
        name: &str,
        mut keep: impl FnMut(&Entry) -> bool,
    ) -> Result<usize, StoreError> {
        let path = self.path(name);
        let mut failure = None;
        let removed = self.change_locked(name, false, |entries| {
            let total = entries.len();
            let kept: Vec<Entry> = entries.into_iter().filter(|entry| keep(entry)).collect();
            let removed = total - kept.len();
            if removed == 0 {
                return (GotchaWrite::Keep, 0);
            }
            if name == GOTCHAS_FILE {
                let lines = kept
                    .into_iter()
                    .map(|entry| stored_line(&StoredGotcha::from_entry(entry)))
                    .collect();
                return (GotchaWrite::Replace(lines), removed);
            }
            let kept: Vec<&Entry> = kept.iter().collect();
            match Self::canonical_lines(&path, &kept) {
                Ok(text) => (GotchaWrite::Replace(Self::split_lines(&text)), removed),
                Err(error) => {
                    failure = Some(error);
                    (GotchaWrite::Keep, 0)
                }
            }
        })?;
        failure.map_or(Ok(removed.unwrap_or(0)), Err)
    }

    pub fn edit_gotchas<T>(
        &self,
        decide: impl FnMut(Vec<Entry>) -> (GotchaWrite, T),
    ) -> Result<T, StoreError> {
        let mut decide = decide;
        self.change_locked(GOTCHAS_FILE, true, &mut decide)?
            .ok_or_else(|| invalid_store(&self.path(GOTCHAS_FILE)))
    }

    pub fn replace_gotchas(&self, lines: &[String]) -> Result<(), StoreError> {
        self.edit_gotchas(|_| (GotchaWrite::Replace(lines.to_vec()), ()))
    }

    pub fn libraries(&self) -> Result<Libraries, StoreError> {
        let path = self.path(LIBRARIES_FILE);
        let Some(text) = read_json_text(&path)? else {
            return Ok(Libraries::default());
        };
        serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| Libraries::from_value(&value))
            .ok_or_else(|| StoreError::Read {
                path,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "libraries.json is not an object of library name to alias list",
                ),
            })
    }

    pub fn save_libraries(&self, libraries: &Libraries) -> Result<(), StoreError> {
        self.ensure()?;
        let path = self.path(LIBRARIES_FILE);
        let mut out = Vec::new();
        write_python_json_indented(&mut out, &libraries.to_value(), 2, true)
            .map_err(|error| write_error(&path)(error.into()))?;
        out.push(b'\n');
        write_text(&path, &String::from_utf8_lossy(&out))
    }

    pub fn create_new(path: &Path, text: &str) -> Result<bool, StoreError> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => file
                .write_all(platform_newlines(text).as_bytes())
                .map(|()| true)
                .map_err(write_error(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(write_error(path)(error)),
        }
    }

    pub fn backup_v1(&self) -> Result<PathBuf, StoreError> {
        self.ensure()?;
        let backup = self.path(BACKUP_FILE);
        let source = self.path(GOTCHAS_FILE);
        let bytes = match fs::read(&source) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(StoreError::Read {
                    path: source,
                    source: error,
                });
            }
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(mut file) => file.write_all(&bytes).map_err(write_error(&backup))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(write_error(&backup)(error)),
        }
        Ok(backup)
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
        let libraries = self.libraries()?;
        let mut export = serde_json::json!({
            "version": "1.0",
            "exported_at": exported_at,
            "patterns": self.load_patterns()?,
            "gotchas": self.load_gotchas()?,
            "preferences": preferences,
        });
        if let Value::Object(map) = &mut export
            && !libraries.is_empty()
        {
            map.insert("libraries".to_owned(), libraries.to_value());
        }
        Ok(export)
    }

    fn merge_libraries(&self, imported: &Libraries) -> Result<(), StoreError> {
        let mut failure = None;
        self.edit_gotchas(|_| {
            let saved = self.libraries().and_then(|mut libraries| {
                if libraries.merge(imported) {
                    self.save_libraries(&libraries)
                } else {
                    Ok(())
                }
            });
            failure = saved.err();
            (GotchaWrite::Keep, ())
        })?;
        failure.map_or(Ok(()), Err)
    }

    fn import_entries(&self, name: &str, items: &[Value]) -> Result<usize, StoreError> {
        let path = self.path(name);
        let mut failure = None;
        let added = self.change_locked(name, true, |existing| {
            let known: HashSet<String> = existing
                .iter()
                .filter_map(|entry| entry.get("id"))
                .map(Value::to_string)
                .collect();
            let fresh: Vec<&Entry> = items
                .iter()
                .filter_map(Value::as_object)
                .filter(|entry| {
                    entry
                        .get("id")
                        .is_none_or(|id| !known.contains(&id.to_string()))
                })
                .collect();
            if fresh.is_empty() {
                return (GotchaWrite::Keep, 0);
            }
            match Self::canonical_lines(&path, &fresh) {
                Ok(text) => (GotchaWrite::Append(Self::split_lines(&text)), fresh.len()),
                Err(error) => {
                    failure = Some(error);
                    (GotchaWrite::Keep, 0)
                }
            }
        })?;
        failure.map_or(Ok(added.unwrap_or(0)), Err)
    }

    pub fn import(&self, data: &Map<String, Value>) -> Result<(usize, usize), StoreError> {
        let mut added = [0_usize; 2];
        for (slot, (key, name)) in added
            .iter_mut()
            .zip([("patterns", PATTERNS_FILE), ("gotchas", GOTCHAS_FILE)])
        {
            if let Some(Value::Array(items)) = data.get(key) {
                *slot = self.import_entries(name, items)?;
            }
        }
        if let Some(Value::String(preferences)) = data.get("preferences")
            && !preferences.is_empty()
            && !self.has_preferences()
        {
            write_text(&self.path(PREFERENCES_FILE), preferences)?;
        }
        if let Some(imported) = data.get("libraries").and_then(Libraries::from_value) {
            self.merge_libraries(&imported)?;
        }
        let [patterns, gotchas] = added;
        Ok((patterns, gotchas))
    }
}
