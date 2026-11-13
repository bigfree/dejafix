use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::types::{ClientConfig, ClientPaths, ConfigState};

const CLAUDE_DESKTOP: &str = "Claude Desktop";
const DESKTOP_CONFIG: &str = "Claude/claude_desktop_config.json";
const STORE_PREFIX: &str = "Claude_";
const PIN_MARKERS: [&str; 4] = ["--root", "\"cwd\"", "cwd =", "DEJAFIX_ROOT"];

fn nested(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn store_packages(local: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(local.join("Packages")) else {
        return Vec::new();
    };
    let mut packages: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if cfg!(windows) {
                name.to_lowercase()
                    .starts_with(&STORE_PREFIX.to_lowercase())
            } else {
                name.starts_with(STORE_PREFIX)
            }
        })
        .map(|entry| entry.path())
        .collect();
    packages.sort();
    packages
}

#[must_use]
pub fn client_configs(
    paths: &ClientPaths,
    project_root: Option<&Path>,
) -> Vec<(&'static str, PathBuf)> {
    let home = &paths.home;
    let xdg = paths
        .xdg_config_home
        .clone()
        .unwrap_or_else(|| home.join(".config"));
    let appdata = paths
        .appdata
        .clone()
        .unwrap_or_else(|| nested(home, "AppData/Roaming"));
    let local = paths
        .local_appdata
        .clone()
        .unwrap_or_else(|| nested(home, "AppData/Local"));
    let mut entries = vec![
        (
            CLAUDE_DESKTOP,
            nested(&nested(home, "Library/Application Support"), DESKTOP_CONFIG),
        ),
        (CLAUDE_DESKTOP, nested(&appdata, DESKTOP_CONFIG)),
        (CLAUDE_DESKTOP, nested(&xdg, DESKTOP_CONFIG)),
        ("Cursor", nested(home, ".cursor/mcp.json")),
        (
            "Antigravity",
            nested(home, ".gemini/antigravity/mcp_config.json"),
        ),
        ("Codex", nested(home, ".codex/config.toml")),
        ("Claude Code", home.join(".claude.json")),
    ];
    entries.extend(store_packages(&local).into_iter().map(|package| {
        (
            "Claude Desktop (Store)",
            nested(&nested(&package, "LocalCache/Roaming"), DESKTOP_CONFIG),
        )
    }));
    if let Some(root) = project_root {
        entries.push(("This project (.mcp.json)", root.join(".mcp.json")));
        entries.push((
            "This project (.cursor/mcp.json)",
            nested(root, ".cursor/mcp.json"),
        ));
    }
    entries
}

fn state(path: &Path) -> Option<ConfigState> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(ConfigState::Unreadable),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Some(ConfigState::Unreadable);
    };
    if !text.contains("dejafix") {
        return None;
    }
    if PIN_MARKERS.iter().any(|marker| text.contains(marker)) {
        Some(ConfigState::Pinned)
    } else {
        Some(ConfigState::Clean)
    }
}

#[must_use]
pub fn scan_client_configs(paths: &ClientPaths, project_root: Option<&Path>) -> Vec<ClientConfig> {
    client_configs(paths, project_root)
        .into_iter()
        .filter_map(|(client, path)| {
            let state = state(&path)?;
            Some(ClientConfig {
                client,
                path,
                state,
            })
        })
        .collect()
}

#[must_use]
pub fn pinned_client_configs(
    paths: &ClientPaths,
    project_root: Option<&Path>,
) -> Vec<ClientConfig> {
    scan_client_configs(paths, project_root)
        .into_iter()
        .filter(|config| config.state == ConfigState::Pinned)
        .collect()
}
