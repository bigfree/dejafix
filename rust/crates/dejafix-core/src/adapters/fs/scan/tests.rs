use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ClientPaths, ConfigState, client_configs, dedupe_paths, default_roots, find_projects,
    pinned_client_configs, scan_client_configs,
};

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

fn project(root: &Path) {
    fs::create_dir_all(root.join(".dejafix")).unwrap();
}

#[test]
fn the_walk_finds_projects_and_skips_noise() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let work = base.as_path().join("work");
    project(&work.join("one"));
    project(&work.join("nested/two"));
    project(&work.join("one/inner"));
    project(&work.join("node_modules/pkg"));
    project(&work.join(".hidden/three"));
    fs::create_dir_all(work.join("plain")).unwrap();
    fs::create_dir_all(work.join("file")).unwrap();
    fs::write(work.join("file/.dejafix"), "").unwrap();
    let mut found = find_projects(&work, 4);
    found.sort();
    assert_eq!(found, [work.join("nested/two"), work.join("one")]);
}

#[test]
fn the_default_depth_reaches_a_nested_repo_folder() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let deep = base.as_path().join("drive/repo/Group/Project");
    project(&deep);
    let drive = base.as_path().join("drive");
    assert_eq!(find_projects(&drive, 4), std::slice::from_ref(&deep));
    assert_eq!(find_projects(&drive, 3), Vec::<PathBuf>::new());
    assert_eq!(find_projects(&deep, 0), Vec::<PathBuf>::new());
    assert_eq!(find_projects(&deep, 1), [deep]);
}

#[test]
fn a_missing_root_finds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    assert_eq!(find_projects(&base.as_path().join("missing"), 4).len(), 0);
}

#[cfg(unix)]
#[test]
fn symlinked_folders_are_listed_but_not_entered() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let outside = base.as_path().join("outside");
    project(&outside.join("deep"));
    let work = base.as_path().join("work");
    fs::create_dir_all(&work).unwrap();
    std::os::unix::fs::symlink(&outside, work.join("link")).unwrap();
    assert_eq!(find_projects(&work, 4).len(), 0);
    let linked = base.as_path().join("linked");
    fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(outside.join("deep/.dejafix"), linked.join(".dejafix")).unwrap();
    assert_eq!(find_projects(&linked, 4), [linked]);
}

#[cfg(unix)]
#[test]
fn dedupe_paths_uses_identity_not_text() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let real = base.as_path().join("dir");
    fs::create_dir_all(&real).unwrap();
    let link = base.as_path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(dedupe_paths([real.clone(), link, real.clone()]), [real]);
    assert_eq!(dedupe_paths([base.as_path().join("missing")]).len(), 0);
}

#[cfg(unix)]
#[test]
fn the_same_folder_reached_twice_is_found_once() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let work = base.as_path().join("work");
    project(&work.join("api"));
    let again = base.as_path().join("work-again");
    std::os::unix::fs::symlink(&work, &again).unwrap();
    let found = dedupe_paths(
        find_projects(&work, 4)
            .into_iter()
            .chain(find_projects(&again, 4)),
    );
    assert_eq!(found, [work.join("api")]);
}

#[test]
fn cloud_folders_are_included_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let home = base.as_path();
    for name in [
        "OneDrive - Some University",
        "Dropbox",
        "Nextcloud",
        "iCloudDrive",
        "Box",
        "MEGA",
        "code",
        "Library/CloudStorage/GoogleDrive-x",
        "Library/Mobile Documents/com~apple~CloudDocs",
    ] {
        fs::create_dir_all(home.join(name)).unwrap();
    }
    fs::write(home.join("OneDrive-file"), "").unwrap();
    let roots = default_roots(home);
    for name in [
        "OneDrive - Some University",
        "Dropbox",
        "Nextcloud",
        "iCloudDrive",
        "Box",
        "MEGA",
        "code",
        "Library/CloudStorage/GoogleDrive-x",
        "Library/Mobile Documents/com~apple~CloudDocs",
    ] {
        assert!(roots.contains(&home.join(name)), "{name}: {roots:?}");
    }
    assert!(!roots.contains(&home.join("OneDrive-file")));
    assert_eq!(roots.first(), Some(&home.join("code")));
}

#[cfg(unix)]
#[test]
fn cloud_roots_are_deduped_by_what_they_resolve_to() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    let home = base.as_path();
    let real = home.join("Library/CloudStorage/OneDrive-Uni");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, home.join("OneDrive - Uni")).unwrap();
    let roots = default_roots(home);
    let resolved: Vec<PathBuf> = roots
        .iter()
        .map(|root| fs::canonicalize(root).unwrap())
        .collect();
    let target = fs::canonicalize(&real).unwrap();
    assert_eq!(resolved.iter().filter(|root| **root == target).count(), 1);
    assert_eq!(roots, [home.join("OneDrive - Uni")]);
}

#[test]
fn an_empty_home_has_no_roots() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    assert_eq!(default_roots(base.as_path()).len(), 0);
}

#[cfg(unix)]
#[test]
fn a_symlinked_root_is_walked_under_its_resolved_path() {
    let dir = tempfile::tempdir().unwrap();
    let base = fs::canonicalize(dir.path()).unwrap();
    project(&base.join("real/api"));
    std::os::unix::fs::symlink(base.join("real"), base.join("alias")).unwrap();
    assert_eq!(
        find_projects(&base.join("alias"), 4),
        [base.join("real/api")]
    );
}
