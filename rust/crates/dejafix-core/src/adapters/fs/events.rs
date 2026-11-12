use std::fs;
use std::path::Path;

use super::types::StoreError;

const EVENTS_FILE: &str = "events.jsonl";

pub fn read_events_file(mem_dir: &Path) -> Result<Vec<u8>, StoreError> {
    let path = mem_dir.join(EVENTS_FILE);
    fs::read(&path).map_err(|source| StoreError::Read { path, source })
}
