use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use jiff::tz::TimeZone;
use serde_json::{Map, Value};

use super::store::read_python_text;
use super::types::{GlobalStore, GotchaWrite, StoreError};
use super::write::{write_atomic, write_text};
use crate::core::global::{Entry, Libraries, PROMOTABLE_SEED, is_prunable};
use crate::core::json::{write_canonical, write_python_json_indented};
use crate::core::text::{platform_newlines, python_lines, python_strip, universal_newlines};

const PATTERNS_FILE: &str = "patterns.jsonl";
const GOTCHAS_FILE: &str = "library_gotchas.jsonl";
const PREFERENCES_FILE: &str = "stack_preferences.md";
const PROMOTABLE_FILE: &str = ".promotable.json";
const LIBRARIES_FILE: &str = "libraries.json";
const BACKUP_FILE: &str = "library_gotchas.v1.jsonl";
const LOCK_ATTEMPTS: usize = 16;

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

fn invalid_store(path: &Path) -> StoreError {
    StoreError::Read {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "a line is not a JSON object",
        ),
    }
}

#[cfg(unix)]
fn same_file(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (file.metadata(), fs::metadata(path)) {
        (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_file(file: &File, path: &Path) -> bool {
    match (file.metadata(), fs::metadata(path)) {
        (Ok(open), Ok(named)) => {
            open.len() == named.len() && open.modified().ok() == named.modified().ok()
        }
        _ => false,
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

    fn change_locked<T>(
        &self,
        name: &str,
        create: bool,
        mut decide: impl FnMut(Vec<Entry>) -> (GotchaWrite, T),
    ) -> Result<Option<T>, StoreError> {
        self.ensure()?;
        let path = self.path(name);
        let read_error = |source| StoreError::Read {
            path: path.clone(),
            source,
        };
        let lock_error = |source| StoreError::Lock {
            path: path.clone(),
            source,
        };
        for _ in 0..LOCK_ATTEMPTS {
            let mut file = match OpenOptions::new().read(true).append(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                    return Ok(None);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let (write, value) = decide(Vec::new());
                    if matches!(write, GotchaWrite::Keep) {
                        return Ok(Some(value));
                    }
                    let mut file = match OpenOptions::new()
                        .read(true)
                        .append(true)
                        .create_new(true)
                        .open(&path)
                    {
                        Ok(file) => file,
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            continue;
                        }
                        Err(error) => return Err(write_error(&path)(error)),
                    };
                    file.lock().map_err(lock_error)?;
                    if !same_file(&file, &path) || file.metadata().map_err(read_error)?.len() > 0 {
                        continue;
                    }
                    self.apply(&path, &mut file, write)?;
                    return Ok(Some(value));
                }
                Err(error) => return Err(read_error(error)),
            };
            file.lock().map_err(lock_error)?;
            if !same_file(&file, &path) {
                continue;
            }
            let mut text = String::new();
            file.read_to_string(&mut text).map_err(read_error)?;
            let entries =
                parse_entries(&universal_newlines(&text)).ok_or_else(|| invalid_store(&path))?;
            let (write, value) = decide(entries);
            self.apply(&path, &mut file, write)?;
            return Ok(Some(value));
        }
        Err(lock_error(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "the file kept being replaced while waiting for its lock",
        )))
    }

    fn apply(&self, path: &Path, file: &mut File, write: GotchaWrite) -> Result<(), StoreError> {
        match write {
            GotchaWrite::Keep => Ok(()),
            GotchaWrite::Append(lines) => file
                .write_all(platform_newlines(&joined_lines(&lines)).as_bytes())
                .map_err(write_error(path)),
            GotchaWrite::Replace(lines) => {
                self.ensure()?;
                write_atomic(path, platform_newlines(&joined_lines(&lines)).as_bytes())
            }
        }
    }

    fn retain_locked(
        &self,
        name: &str,
        mut keep: impl FnMut(&Entry) -> bool,
    ) -> Result<usize, StoreError> {
        let path = self.path(name);
        let mut failure = None;
        let removed = self.change_locked(name, false, |entries| {
            let kept: Vec<&Entry> = entries.iter().filter(|entry| keep(entry)).collect();
            let removed = entries.len() - kept.len();
            if removed == 0 {
                return (GotchaWrite::Keep, 0);
            }
            match Self::canonical_lines(&path, &kept) {
                Ok(text) => (
                    GotchaWrite::Replace(text.lines().map(str::to_owned).collect()),
                    removed,
                ),
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
        let path = self.path(GOTCHAS_FILE);
        let mut decide = Some(decide);
        let mut failure = None;
        let appended = self.change_locked(GOTCHAS_FILE, true, |existing| {
            let Some(entry) = decide.take().and_then(|decide| decide(&existing)) else {
                return (GotchaWrite::Keep, false);
            };
            match Self::canonical_lines(&path, &[&entry]) {
                Ok(text) => (
                    GotchaWrite::Append(text.lines().map(str::to_owned).collect()),
                    true,
                ),
                Err(error) => {
                    failure = Some(error);
                    (GotchaWrite::Keep, false)
                }
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        match appended {
            Ok(appended) => Ok(appended.unwrap_or(false)),
            Err(StoreError::Read { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}
