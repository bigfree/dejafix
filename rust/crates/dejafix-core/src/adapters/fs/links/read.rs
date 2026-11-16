use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::super::store::{CONFIG_FILE, MEMORY_DIR, points_outside};
use crate::core::links::{CONFIG_LIMIT, ConfigError, ProjectLinks, origin_url, parse_config};

fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let limit = u64::try_from(CONFIG_LIMIT)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    File::open(path)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_capped_text(path: &Path) -> io::Result<String> {
    String::from_utf8(read_capped(path)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn read_project_links(root: &Path) -> Result<ProjectLinks, ConfigError> {
    let path = root.join(MEMORY_DIR).join(CONFIG_FILE);
    if !path.exists() || points_outside(&path, root) {
        return Ok(ProjectLinks::default());
    }
    let bytes = read_capped(&path).map_err(|error| ConfigError::Unreadable(error.to_string()))?;
    if bytes.len() > CONFIG_LIMIT {
        return Err(ConfigError::TooLarge(CONFIG_LIMIT));
    }
    let text =
        String::from_utf8(bytes).map_err(|error| ConfigError::Unreadable(error.to_string()))?;
    parse_config(&text)
}

fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = read_capped_text(&dot_git).ok()?;
    let target = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))?
        .trim();
    let gitdir = root.join(target);
    match read_capped_text(&gitdir.join("commondir")) {
        Ok(common) => Some(gitdir.join(common.trim())),
        Err(_) => Some(gitdir),
    }
}

#[must_use]
pub fn read_origin(root: &Path) -> Option<String> {
    let config = read_capped_text(&git_dir(root)?.join("config")).ok()?;
    origin_url(&config)
}
