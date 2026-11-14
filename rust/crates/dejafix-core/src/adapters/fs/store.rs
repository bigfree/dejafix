use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::registry::canonical_path;
use super::types::{MemoryDir, StoreError};
use crate::core::text::universal_newlines;

pub(crate) const MEMORY_DIR: &str = ".dejafix";
const CONFIG_FILE: &str = "config.toml";

fn is_project_memory(candidate: &Path) -> bool {
    candidate.is_dir() && candidate.join(CONFIG_FILE).exists()
}

impl MemoryDir {
    pub fn at(root: &Path) -> Result<Self, StoreError> {
        let dir = root.join(MEMORY_DIR);
        if points_outside(&dir, root) {
            return Err(StoreError::OutsideProject { path: dir });
        }
        Ok(Self {
            root: root.to_path_buf(),
            dir,
        })
    }
}

pub fn find_memory_dir(cwd: &Path) -> Result<MemoryDir, StoreError> {
    if is_project_memory(&cwd.join(MEMORY_DIR)) {
        return MemoryDir::at(cwd);
    }
    let resolved = canonical_path(cwd).unwrap_or_else(|| cwd.to_path_buf());
    resolved
        .ancestors()
        .find(|ancestor| is_project_memory(&ancestor.join(MEMORY_DIR)))
        .ok_or_else(|| StoreError::NotFound {
            cwd: cwd.to_path_buf(),
        })
        .and_then(MemoryDir::at)
}

pub(crate) fn read_python_text(path: &Path, strip_bom: bool) -> io::Result<String> {
    let mut bytes = fs::read(path)?;
    if strip_bom && bytes.starts_with(b"\xef\xbb\xbf") {
        bytes.drain(..3);
    }
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(match universal_newlines(&text) {
        std::borrow::Cow::Borrowed(_) => text,
        std::borrow::Cow::Owned(changed) => changed,
    })
}

pub fn read_text(path: &Path) -> Result<String, StoreError> {
    read_python_text(path, false).map_err(|source| StoreError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_optional_text(path: &Path) -> Result<Option<String>, StoreError> {
    match read_text(path) {
        Ok(text) => Ok(Some(text)),
        Err(StoreError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn existing_target(path: &Path) -> Option<PathBuf> {
    for candidate in path.ancestors() {
        match fs::canonicalize(candidate) {
            Ok(target) => return Some(target),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && fs::symlink_metadata(candidate).is_err() => {}
            Err(_) => return None,
        }
    }
    None
}

pub(crate) fn points_outside(path: &Path, root: &Path) -> bool {
    match (existing_target(path), fs::canonicalize(root)) {
        (Some(target), Ok(root)) => !target.starts_with(root),
        _ => true,
    }
}

pub fn read_text_within(path: &Path, root: &Path) -> Result<Option<String>, StoreError> {
    if points_outside(path, root) {
        return Ok(None);
    }
    read_optional_text(path)
}

#[must_use]
pub fn project_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[must_use]
pub fn issues_dir(memory: &MemoryDir) -> PathBuf {
    memory.dir.join("issues")
}
