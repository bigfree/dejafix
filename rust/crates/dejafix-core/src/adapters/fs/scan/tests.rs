use std::fs;
use std::path::{Path, PathBuf};

use super::{ClientPaths, ConfigState, client_configs, pinned_client_configs, scan_client_configs};

const PINNED: &str = r#"{"mcpServers":{"dejafix":{"command":"py","args":["-m","dejafix.mcp_server","--root","D:/repo/Thing"]}}}"#;
const CLEAN: &str =
    r#"{"mcpServers":{"dejafix":{"command":"py","args":["-m","dejafix.mcp_server"]}}}"#;

struct Windows {
    _dir: tempfile::TempDir,
    paths: ClientPaths,
    local: PathBuf,
    appdata: PathBuf,
}

fn windows() -> Windows {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let appdata = dir.path().join("Roaming");
    let local = dir.path().join("Local");
    for path in [&home, &appdata, &local] {
        fs::create_dir_all(path).unwrap();
    }
    Windows {
        paths: ClientPaths {
            home,
            xdg_config_home: None,
            appdata: Some(appdata.clone()),
            local_appdata: Some(local.clone()),
        },
        _dir: dir,
        local,
        appdata,
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn store_config(local: &Path, package: &str) -> PathBuf {
    local
        .join("Packages")
        .join(package)
        .join("LocalCache/Roaming/Claude/claude_desktop_config.json")
}

fn pinned_paths(paths: &ClientPaths, root: Option<&Path>) -> Vec<PathBuf> {
    pinned_client_configs(paths, root)
        .into_iter()
        .map(|config| config.path)
        .collect()
}

#[test]
fn a_store_installed_claude_desktop_is_discovered() {
    let env = windows();
    let config = store_config(&env.local, "Claude_pzs8sxrjxfjjc");
    write(&config, PINNED);
    assert!(
        client_configs(&env.paths, None)
            .iter()
            .any(|(_, path)| *path == config)
    );
    assert_eq!(pinned_paths(&env.paths, None), [config]);
}

#[test]
fn the_store_entry_is_labelled_distinctly() {
    let env = windows();
    write(&store_config(&env.local, "Claude_abc123"), PINNED);
    let clients: Vec<&str> = pinned_client_configs(&env.paths, None)
        .iter()
        .map(|config| config.client)
        .collect();
    assert_eq!(clients, ["Claude Desktop (Store)"]);
}

#[test]
fn appdata_is_honoured_over_the_default_under_home() {
    let env = windows();
    let config = env.appdata.join("Claude/claude_desktop_config.json");
    write(&config, PINNED);
    assert!(!env.paths.home.join("AppData/Roaming/Claude").exists());
    assert_eq!(pinned_paths(&env.paths, None), [config]);
}

#[test]
fn several_store_packages_are_all_found_in_a_stable_order() {
    let env = windows();
    let made: Vec<PathBuf> = ["Claude_zzz999", "Claude_aaa111", "Other_x"]
        .iter()
        .map(|package| {
            let config = store_config(&env.local, package);
            write(&config, PINNED);
            config
        })
        .collect();
    assert_eq!(
        pinned_paths(&env.paths, None),
        [made[1].clone(), made[0].clone()]
    );
}

#[test]
fn no_packages_directory_is_not_an_error() {
    let env = windows();
    assert!(!env.local.join("Packages").exists());
    assert_eq!(pinned_client_configs(&env.paths, None).len(), 0);
}

#[test]
fn an_unreadable_config_is_reported_rather_than_skipped() {
    let env = windows();
    let config = env.appdata.join("Claude/claude_desktop_config.json");
    fs::create_dir_all(&config).unwrap();
    let invalid = env.paths.home.join(".cursor/mcp.json");
    write(&invalid, "");
    fs::write(&invalid, b"dejafix \xff").unwrap();
    let states: Vec<(&str, ConfigState)> = scan_client_configs(&env.paths, None)
        .iter()
        .map(|config| (config.client, config.state))
        .collect();
    assert_eq!(
        states,
        [
            ("Claude Desktop", ConfigState::Unreadable),
            ("Cursor", ConfigState::Unreadable)
        ]
    );
    assert_eq!(pinned_client_configs(&env.paths, None).len(), 0);
}

#[test]
fn a_missing_config_says_nothing_at_all() {
    let env = windows();
    assert_eq!(scan_client_configs(&env.paths, None).len(), 0);
}

#[test]
fn a_project_scoped_mcp_json_is_checked() {
    let env = windows();
    let project = env.paths.home.join("repo");
    write(&project.join(".mcp.json"), PINNED);
    assert_eq!(pinned_client_configs(&env.paths, None).len(), 0);
    assert_eq!(
        pinned_paths(&env.paths, Some(&project)),
        [project.join(".mcp.json")]
    );
}

#[test]
fn a_clean_project_config_is_not_flagged() {
    let env = windows();
    let project = env.paths.home.join("repo");
    write(&project.join(".cursor/mcp.json"), CLEAN);
    assert_eq!(pinned_client_configs(&env.paths, Some(&project)).len(), 0);
    let states: Vec<ConfigState> = scan_client_configs(&env.paths, Some(&project))
        .iter()
        .map(|config| config.state)
        .collect();
    assert_eq!(states, [ConfigState::Clean]);
}

#[test]
fn every_pin_mechanism_counts_and_unrelated_configs_are_ignored() {
    let env = windows();
    let xdg = env.paths.home.join("xdg");
    let paths = ClientPaths {
        xdg_config_home: Some(xdg.clone()),
        ..env.paths.clone()
    };
    write(
        &xdg.join("Claude/claude_desktop_config.json"),
        r#"{"dejafix": {"cwd": "/x"}}"#,
    );
    write(
        &paths.home.join(".codex/config.toml"),
        "[mcp_servers.dejafix]\ncwd = \"/x\"\n",
    );
    write(
        &paths.home.join(".claude.json"),
        r#"{"env": {"DEJAFIX_ROOT": "/x"}, "dejafix": 1}"#,
    );
    write(
        &paths.home.join(".gemini/antigravity/mcp_config.json"),
        r#"{"other": "--root"}"#,
    );
    let clients: Vec<&str> = pinned_client_configs(&paths, None)
        .iter()
        .map(|config| config.client)
        .collect();
    assert_eq!(clients, ["Claude Desktop", "Codex", "Claude Code"]);
    let defaults = client_configs(
        &ClientPaths {
            home: PathBuf::from("/h"),
            xdg_config_home: None,
            appdata: None,
            local_appdata: None,
        },
        None,
    );
    let defaults: Vec<PathBuf> = defaults.into_iter().map(|(_, path)| path).collect();
    assert!(defaults.contains(&PathBuf::from(
        "/h/.config/Claude/claude_desktop_config.json"
    )));
    assert!(defaults.contains(&PathBuf::from(
        "/h/AppData/Roaming/Claude/claude_desktop_config.json"
    )));
}
