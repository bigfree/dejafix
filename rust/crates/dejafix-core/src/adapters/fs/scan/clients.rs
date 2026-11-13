use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::types::{ClientConfig, ClientPaths, ConfigState};

const CLAUDE_DESKTOP: &str = "Claude Desktop";
const DESKTOP_CONFIG: &str = "Claude/claude_desktop_config.json";
const STORE_PREFIX: &str = "Claude_";
const PIN_MARKERS: [&str; 4] = ["--root", "\"cwd\"", "cwd =", "DEJAFIX_ROOT"];

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
        .unwrap_or_else(|| home.join("AppData/Roaming"));
    let local = paths
        .local_appdata
        .clone()
        .unwrap_or_else(|| home.join("AppData/Local"));
    let mut entries = vec![
        (
            CLAUDE_DESKTOP,
            home.join("Library/Application Support")
                .join(DESKTOP_CONFIG),
        ),
        (CLAUDE_DESKTOP, appdata.join(DESKTOP_CONFIG)),
        (CLAUDE_DESKTOP, xdg.join(DESKTOP_CONFIG)),
        ("Cursor", home.join(".cursor/mcp.json")),
        (
            "Antigravity",
            home.join(".gemini/antigravity/mcp_config.json"),
        ),
        ("Codex", home.join(".codex/config.toml")),
        ("Claude Code", home.join(".claude.json")),
    ];
    entries.extend(store_packages(&local).into_iter().map(|package| {
        (
            "Claude Desktop (Store)",
            package.join("LocalCache/Roaming").join(DESKTOP_CONFIG),
        )
    }));
    if let Some(root) = project_root {
        entries.push(("This project (.mcp.json)", root.join(".mcp.json")));
        entries.push((
            "This project (.cursor/mcp.json)",
            root.join(".cursor/mcp.json"),
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
