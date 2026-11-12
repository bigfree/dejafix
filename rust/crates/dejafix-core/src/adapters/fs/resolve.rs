use std::fs;
use std::path::Path;

use super::types::{FsResolver, StoreError};
use crate::core::path::{PathContext, PathResolver};

impl PathResolver for FsResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        if cfg!(windows) && is_network_path(path) {
            return None;
        }
        let path = Path::new(path);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(path)
        };
        let ancestors: Vec<&Path> = absolute.ancestors().collect();
        let deepest = ancestors.partition_point(|ancestor| fs::canonicalize(ancestor).is_err());
        let ancestor = ancestors.get(deepest)?;
        let existing = fs::canonicalize(ancestor).ok()?;
        let tail = absolute.strip_prefix(ancestor).ok()?;
        Some(posix(&existing.join(tail)))
    }
}

pub(super) fn is_network_path(path: &str) -> bool {
    matches!(path.as_bytes(), [b'/' | b'\\', b'/' | b'\\', ..])
}

pub fn path_context(root: &Path) -> Result<PathContext, StoreError> {
    let resolved = fs::canonicalize(root).map_err(|source| StoreError::Root {
        path: root.to_path_buf(),
        source,
    })?;
    let cwd = std::env::current_dir()
        .and_then(fs::canonicalize)
        .map_or_else(|_| "/".to_owned(), |cwd| posix(&cwd));
    Ok(PathContext {
        root: posix(&resolved),
        cwd,
        fold_case: cfg!(windows),
    })
}

fn posix(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| text.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or_else(|| text.into_owned());
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}
