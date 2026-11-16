use std::fs;
use std::path::{Path, PathBuf};

use super::project::project_links;
use super::*;
use crate::core::links::direction_text;

const NONE: Vec<(String, String)> = Vec::new();

struct World {
    _dir: tempfile::TempDir,
    home: PathBuf,
    portal: PathBuf,
    inventory: PathBuf,
}

fn project(root: &Path, origin: &str, config: &str, events: &str) {
    fs::create_dir_all(root.join(".dejafix")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(
        root.join(".git/config"),
        format!("[remote \"origin\"]\n\turl = {origin}\n"),
    )
    .unwrap();
    fs::write(root.join(".dejafix/config.toml"), config).unwrap();
    fs::write(root.join(".dejafix/events.jsonl"), events).unwrap();
}

fn register(home: &Path, roots: &[&Path]) {
    fs::create_dir_all(home).unwrap();
    let listed: Vec<String> = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect();
    fs::write(
        home.join("projects.json"),
        serde_json::to_string(&listed).unwrap(),
    )
    .unwrap();
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::adapters::fs::canonical_path(dir.path()).unwrap();
    let home = base.join("home/.dejafix");
    let portal = base.join("portal");
    let inventory = base.join("inventory");
    project(
        &portal,
        "git@github.com:org/portal.git",
        "boundary = [\"src/api/**\"]\n\n[[links]]\nproject = \"github.com/org/inventory\"\nrelation = \"reads device ports\"\n",
        "",
    );
    project(
        &inventory,
        "https://github.com/org/inventory",
        "boundary = [\"src/Api/**\"]\n",
        "{\"type\": \"issue\", \"issue_id\": \"0001\", \"summary\": \"port allocation races with the nightly sync\", \"location\": \"src/Api/Ports.php\"}\n",
    );
    register(&home, &[&portal, &inventory]);
    World {
        _dir: dir,
        home,
        portal,
        inventory,
    }
}

#[test]
fn both_sides_see_the_link() {
    let world = world();
    let from_portal = project_links(&world.home, &world.portal);
    assert_eq!(from_portal.resolution.links.len(), 1);
    assert_eq!(from_portal.resolution.links[0].path, world.inventory);
    let from_inventory = project_links(&world.home, &world.inventory);
    assert_eq!(from_inventory.resolution.links[0].path, world.portal);
    assert_eq!(
        direction_text(from_inventory.resolution.links[0].direction),
        "linked from there"
    );
}

#[test]
fn a_boundary_edit_gets_the_linked_lesson_and_others_get_nothing() {
    let world = world();
    let reports = boundary_reports(
        &world.home,
        &world.portal,
        &[
            "src/api/backend/ports.ts".to_owned(),
            "src/ui/ports.ts".to_owned(),
        ],
    );
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].0, "src/api/backend/ports.ts");
    assert!(
        reports[0].1.contains("port allocation races"),
        "{}",
        reports[0].1
    );
}

#[test]
fn the_session_text_lists_links_and_counts() {
    let world = world();
    assert_eq!(
        links_text(&world.home, &world.portal),
        "Linked projects (read their memory with project=\"<name>\"):\n  - inventory (github.com/org/inventory), links here: \"reads device ports\". 1 open issue(s) on its boundary."
    );
}

#[test]
fn a_broken_linked_project_is_skipped_and_a_broken_own_config_is_reported() {
    let world = world();
    fs::write(world.inventory.join(".dejafix/events.jsonl"), "{not json\n").unwrap();
    assert_eq!(
        boundary_reports(&world.home, &world.portal, &["src/api/ports.ts".to_owned()]),
        NONE
    );
    fs::write(world.portal.join(".dejafix/config.toml"), "[[links]\n").unwrap();
    assert!(
        links_text(&world.home, &world.portal)
            .starts_with("could not read links: config.toml is not valid TOML")
    );
    assert_eq!(
        boundary_reports(&world.home, &world.portal, &["src/api/ports.ts".to_owned()]),
        NONE
    );
}

#[test]
fn without_a_registry_or_links_nothing_is_said() {
    let world = world();
    let nowhere = world.home.join("missing");
    assert_eq!(links_text(&nowhere, &world.portal), "");
    assert_eq!(
        boundary_reports(&nowhere, &world.portal, &["src/api/ports.ts".to_owned()]),
        NONE
    );
}

#[test]
fn a_registered_worktree_of_a_linked_project_is_not_a_duplicate() {
    let world = world();
    let worktree = world.inventory.join(".claude/worktrees/feature");
    project(
        &worktree,
        "https://github.com/org/inventory",
        "boundary = [\"src/Api/**\"]\n",
        "",
    );
    register(&world.home, &[&world.portal, &worktree, &world.inventory]);
    let linked = project_links(&world.home, &world.portal);
    assert_eq!(linked.resolution.links[0].path, world.inventory);
    assert_eq!(linked.resolution.duplicates, Vec::<String>::new());
    assert_eq!(link_problems(&world.home), Vec::<String>::new());
}

#[test]
fn doctor_problems_name_unresolved_links_and_bad_configs() {
    let world = world();
    fs::write(
        world.portal.join(".dejafix/config.toml"),
        "[[links]]\nproject = \"billing\"\n",
    )
    .unwrap();
    fs::write(world.inventory.join(".dejafix/config.toml"), "links = 5\n").unwrap();
    let problems = link_problems(&world.home);
    assert!(
        problems.contains(&"portal: billing is not registered on this machine".to_owned()),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|problem| problem.starts_with("inventory: config.toml: 'links' must be a list")),
        "{problems:?}"
    );
}

#[test]
fn a_worktree_without_a_remote_is_named_after_its_main_checkout() {
    let world = world();
    let worktree = world.portal.join(".claude/worktrees/feature");
    fs::create_dir_all(worktree.join(".dejafix")).unwrap();
    fs::write(worktree.join(".dejafix/config.toml"), "").unwrap();
    fs::remove_dir_all(world.portal.join(".git")).unwrap();
    fs::write(
        world.inventory.join(".dejafix/config.toml"),
        "boundary = [\"src/Api/**\"]\n\n[[links]]\nproject = \"Portal\"\n",
    )
    .unwrap();
    let linked = project_links(&world.home, &worktree);
    assert_eq!(linked.own.identity, "portal");
    assert_eq!(linked.resolution.links[0].path, world.inventory);
    assert_eq!(
        direction_text(linked.resolution.links[0].direction),
        "linked from there"
    );
}
