use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::store::points_outside;
use super::types::{LockedLog, StoreError};

const EVENTS_FILE: &str = "events.jsonl";

#[must_use]
pub fn events_path(mem_dir: &Path) -> PathBuf {
    mem_dir.join(EVENTS_FILE)
}

fn checked_events_path(mem_dir: &Path) -> Result<PathBuf, StoreError> {
    let path = events_path(mem_dir);
    let root = mem_dir.parent().unwrap_or(mem_dir);
    if points_outside(&path, root) {
        return Err(StoreError::OutsideProject { path });
    }
    Ok(path)
}

pub fn read_events_unlocked(mem_dir: &Path) -> Result<Vec<u8>, StoreError> {
    let path = checked_events_path(mem_dir)?;
    let read = |source| StoreError::Read {
        path: path.clone(),
        source,
    };
    if fs::metadata(&path).is_ok_and(|meta| !meta.is_file()) {
        return Err(read(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        )));
    }
    fs::read(&path).map_err(read)
}

pub fn read_events_file(mem_dir: &Path) -> Result<Vec<u8>, StoreError> {
    let path = checked_events_path(mem_dir)?;
    let read = |source| StoreError::Read {
        path: path.clone(),
        source,
    };
    if fs::metadata(&path).is_ok_and(|meta| !meta.is_file()) {
        return Err(read(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        )));
    }
    let mut file = File::open(&path).map_err(read)?;
    file.lock_shared().map_err(|source| StoreError::Lock {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(read)?;
    Ok(bytes)
}

pub fn lock_events(mem_dir: &Path) -> Result<(LockedLog, Vec<u8>), StoreError> {
    let path = checked_events_path(mem_dir)?;
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|source| StoreError::Write {
            path: path.clone(),
            source,
        })?;
    file.lock().map_err(|source| StoreError::Lock {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| StoreError::Read {
            path: path.clone(),
            source,
        })?;
    let unterminated = bytes
        .last()
        .is_some_and(|last| !matches!(last, b'\n' | b'\r'));
    Ok((
        LockedLog {
            file,
            path,
            unterminated,
        },
        bytes,
    ))
}

impl LockedLog {
    pub fn append(&mut self, lines: &[u8]) -> Result<(), StoreError> {
        let mut buffer = Vec::with_capacity(lines.len() + 2);
        if self.unterminated {
            buffer.push(b'\n');
        }
        buffer.extend_from_slice(lines);
        let buffer = platform_line_ends(buffer);
        self.unterminated = false;
        self.file
            .write_all(&buffer)
            .and_then(|()| self.file.flush())
            .map_err(|source| StoreError::Write {
                path: self.path.clone(),
                source,
            })
    }
}

fn platform_line_ends(bytes: Vec<u8>) -> Vec<u8> {
    if !cfg!(windows) {
        return bytes;
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 64);
    for byte in bytes {
        if byte == b'\n' {
            out.push(b'\r');
        }
        out.push(byte);
    }
    out
}
