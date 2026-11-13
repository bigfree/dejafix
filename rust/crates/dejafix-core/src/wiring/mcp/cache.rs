use std::fs;
use std::path::Path;
use std::sync::{Arc, MutexGuard, PoisonError};

use super::errors::io_error;
use super::types::{CacheEntry, CachedLog, LogCache, McpState, ToolError};
use crate::adapters::fs::{
    FsResolver, MEMORY_DIR, events_path, path_context, posix, read_events_file,
};
use crate::core::log::EventLog;
use crate::core::time::Clock;

const CACHED_PROJECTS: usize = 16;

impl LogCache {
    fn fresh(
        &mut self,
        root: &Path,
        len: u64,
        modified: Option<std::time::SystemTime>,
    ) -> Option<Arc<CachedLog>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.entries.get_mut(root)?;
        if entry.len != len || entry.modified != modified {
            return None;
        }
        entry.used = tick;
        Some(Arc::clone(&entry.log))
    }

    fn keep(&mut self, root: &Path, entry: CacheEntry) {
        if !self.entries.contains_key(root)
            && self.entries.len() >= CACHED_PROJECTS
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(root, _)| root.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(root.to_path_buf(), entry);
    }
}

impl McpState {
    fn cache(&self) -> MutexGuard<'_, LogCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn cached_log(
        &self,
        root: &Path,
        clock: &Clock,
    ) -> Result<Arc<CachedLog>, ToolError> {
        let dir = root.join(MEMORY_DIR);
        let path = events_path(&dir);
        let metadata = fs::metadata(&path).map_err(|error| io_error(&path, &error))?;
        let modified = metadata.modified().ok();
        if let Some(log) = self.cache().fresh(root, metadata.len(), modified) {
            return Ok(log);
        }
        let bytes = read_events_file(&dir)?;
        let mut context = path_context(root)?;
        context.cwd = fs::canonicalize(&self.settings.cwd)
            .map_or_else(|_| context.cwd.clone(), |cwd| posix(&cwd));
        let log = EventLog::parse(&bytes, clock, &context, &FsResolver).into_owned();
        let cached = Arc::new(CachedLog { log, context });
        let mut cache = self.cache();
        let used = cache.tick;
        cache.keep(
            root,
            CacheEntry {
                len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                modified,
                used,
                log: Arc::clone(&cached),
            },
        );
        Ok(cached)
    }

    #[must_use]
    pub fn cached_projects(&self) -> usize {
        self.cache().entries.len()
    }
}
