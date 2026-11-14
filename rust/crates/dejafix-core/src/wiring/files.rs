use std::path::{Path, PathBuf};

use super::types::{MemoryFile, WiringError};
use crate::adapters::fs::{find_memory_dir, read_text_inside, read_text_within, write_text};
use crate::core::scaffold::plan_with_idea;

impl MemoryFile {
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Instructions => "AI_INSTRUCTIONS.md",
            Self::ProjectMap => "PROJECT_MAP.md",
            Self::Plan => "plan.md",
        }
    }
}

pub fn read_memory_file(cwd: &Path, file: MemoryFile) -> Result<(PathBuf, String), WiringError> {
    let memory = find_memory_dir(cwd)?;
    let path = memory.dir.join(file.file_name());
    let text = read_text_inside(&path, &memory.root)?;
    Ok((path, text))
}

pub fn add_plan_idea(cwd: &Path, idea: &str) -> Result<PathBuf, WiringError> {
    let memory = find_memory_dir(cwd)?;
    let path = memory.dir.join(MemoryFile::Plan.file_name());
    let existing = read_text_within(&path, &memory.root)?.unwrap_or_default();
    write_text(&path, &plan_with_idea(&existing, idea))?;
    Ok(path)
}
