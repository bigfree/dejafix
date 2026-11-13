use std::fs;
use std::path::{Path, PathBuf};

use super::types::FsProbe;
use crate::adapters::fs::{MEMORY_DIR, find_memory_dir, resolve_path};
use crate::core::resolve::Probe;

impl FsProbe<'_> {
    fn absolute(&self, path: &Path) -> PathBuf {
        self.cwd.join(path)
    }
}

impl Probe for FsProbe<'_> {
    fn has_memory(&self, path: &Path) -> bool {
        self.absolute(path).join(MEMORY_DIR).is_dir()
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.absolute(path).is_dir()
    }

    fn expand_user(&self, text: &str) -> PathBuf {
        match (self.home, text.strip_prefix('~')) {
            (Some(home), Some("")) => home.to_path_buf(),
            (Some(home), Some(rest)) if rest.starts_with(['/', std::path::MAIN_SEPARATOR]) => {
                home.join(rest.trim_start_matches(['/', std::path::MAIN_SEPARATOR]))
            }
            _ => PathBuf::from(text),
        }
    }

    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let absolute = self.absolute(path);
        Some(fs::canonicalize(&absolute).unwrap_or(absolute))
    }

    fn lookup(&self, text: &str) -> Option<PathBuf> {
        resolve_path(text, self.home, self.cwd)
    }

    fn discover(&self) -> Option<PathBuf> {
        find_memory_dir(self.cwd).ok().map(|memory| memory.root)
    }
}
