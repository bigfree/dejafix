use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::support::{Repo, TestResult};

fn linked(repo: &Repo) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let status = Command::new("git")
        .args(["remote", "add", "origin", "git@github.com:org/portal.git"])
        .current_dir(&repo.root)
        .status()?;
    assert!(status.success());
    fs::write(
        repo.mem().join("config.toml"),
        "recent_days = 30\nboundary = [\"src/api/**\"]\n\n[[links]]\nproject = \"github.com/org/inventory\"\nrelation = \"reads device ports\"\n\n[[links]]\nproject = \"billing\"\n",
    )?;
    let inventory = repo.home.join("inventory");
    fs::create_dir_all(inventory.join(".git"))?;
    fs::write(
        inventory.join(".git/config"),
        "[remote \"origin\"]\n\turl = https://github.com/org/inventory\n",
    )?;
    fs::create_dir_all(inventory.join(".dejafix"))?;
    fs::write(
        inventory.join(".dejafix/config.toml"),
        "boundary = [\"src/Api/**\"]\n",
    )?;
    fs::write(
        inventory.join(".dejafix/events.jsonl"),
        "{\"type\": \"issue\", \"issue_id\": \"0001\", \"summary\": \"port allocation races\", \"location\": \"src/Api/Ports.php\"}\n",
    )?;
    repo.djf(&["project", "register"]).assert().success();
    repo.djf(&["project", "register", &inventory.display().to_string()])
        .assert()
        .success();
    Ok(inventory)
}

#[test]
fn links_lists_resolved_and_unresolved_links() -> TestResult {
    let repo = Repo::new()?;
    let inventory = linked(&repo)?;
    let out = repo.stdout(&["links"])?;
    assert!(
        out.starts_with("dejafix links — repo (github.com/org/portal)\n  Boundary: src/api/**\n"),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "  - inventory: links here, github.com/org/inventory, {}\n      \"reads device ports\"\n",
            inventory.display()
        )),
        "{out}"
    );
    assert!(
        out.ends_with("  Not registered on this machine: billing\n"),
        "{out}"
    );
    let reverse = String::from_utf8(repo.djf_in(&inventory, &["links"]).output()?.stdout)?;
    assert!(
        reverse.contains("  - repo: linked from there, github.com/org/portal, "),
        "{reverse}"
    );
    Ok(())
}

#[test]
fn brief_shows_the_linked_projects() -> TestResult {
    let repo = Repo::new()?;
    linked(&repo)?;
    let out = repo.stdout(&["brief"])?;
    assert!(
        out.contains("🔗 Linked projects (read their memory with project=\"<name>\"):\n   - inventory (github.com/org/inventory), links here: \"reads device ports\". 1 open issue(s) on its boundary.\n"),
        "{out}"
    );
    Ok(())
}

#[test]
fn links_without_any_says_how_to_declare_one() -> TestResult {
    let repo = Repo::new()?;
    let out = repo.stdout(&["links"])?;
    assert!(
        out.ends_with(
            "  No linked projects. Declare one under [[links]] in .dejafix/config.toml.\n"
        ),
        "{out}"
    );
    Ok(())
}

#[test]
fn links_reports_a_config_it_cannot_read() -> TestResult {
    let repo = Repo::new()?;
    fs::write(repo.mem().join("config.toml"), "links = 5\n")?;
    let out = repo.stdout(&["links"])?;
    assert!(
        out.ends_with("\n  config.toml: 'links' must be a list\n"),
        "{out}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn links_escape_control_characters_in_paths() -> TestResult {
    let repo = Repo::new()?;
    fs::write(
        repo.mem().join("config.toml"),
        "[[links]]\nproject = \"evil\"\n",
    )?;
    let evil = repo.home.join("evil\u{1b}]0;x\u{7}");
    fs::create_dir_all(evil.join(".dejafix"))?;
    fs::write(evil.join(".dejafix/config.toml"), "")?;
    fs::write(evil.join(".dejafix/events.jsonl"), "")?;
    repo.djf(&["project", "register"]).assert().success();
    repo.djf(&[
        "project",
        "register",
        &evil.display().to_string(),
        "--alias",
        "evil",
    ])
    .assert()
    .success();
    let out = repo.stdout(&["links"])?;
    assert!(!out.contains('\u{1b}'), "{out:?}");
    assert!(out.contains("evil\\x1b]0;x\\x07"), "{out}");
    Ok(())
}
