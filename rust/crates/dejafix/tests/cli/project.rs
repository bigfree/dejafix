use std::fs;

use crate::support::{Repo, TestResult};

const NOTICE: &str = concat!(
    "\ndejafix upgraded to ",
    env!("CARGO_PKG_VERSION"),
    ".\n  Run `djf init` in each project to refresh its instructions and hooks.\n  Check your setup with:  djf doctor\n\n"
);

fn registered(repo: &Repo) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = repo.registry().join("projects.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn folder(repo: &Repo, relative: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = repo.home.join(relative);
    fs::create_dir_all(path.join(".dejafix/issues"))?;
    fs::write(path.join(".dejafix/config.toml"), "recent_days = 30\n")?;
    Ok(path)
}

#[test]
fn project_cli_round_trip() -> TestResult {
    let repo = Repo::new()?;
    let root = repo.root.display().to_string();
    let out = repo.stdout(&["project", "register", &root, "--alias", "a"])?;
    assert_eq!(
        out,
        format!("✓ Registered a\n  {root}\n  Use it from any folder:  djf project use a\n")
    );
    assert_eq!(
        repo.stdout(&["project", "use", "a"])?,
        format!("✓ Active project: a\n  {root}\n")
    );
    let listing = repo.stdout(&["project", "list"])?;
    assert!(listing.starts_with("1 project(s)   ● = active\n"));
    assert!(listing.contains(" ● repo  (a)"));
    assert!(listing.contains(&format!("     {root}\n")));
    assert_eq!(
        repo.stdout(&["project", "remove", "a"])?,
        format!("✓ Removed a from the registry\n  {root} and its .dejafix/ are untouched.\n")
    );
    assert_eq!(registered(&repo)?.len(), 0);
    assert!(repo.mem().is_dir());
    Ok(())
}

#[test]
fn registering_an_uninitialised_folder_is_refused() -> TestResult {
    let repo = Repo::new()?;
    let plain = repo.home.join("not-a-project");
    fs::create_dir_all(&plain)?;
    let path = plain.display().to_string();
    repo.djf(&["project", "register", &path])
        .assert()
        .code(1)
        .stdout(format!(
            "✗ {path} has no .dejafix/ — run `djf init` there first.\n"
        ));
    assert_eq!(registered(&repo)?.len(), 0);
    Ok(())
}

#[test]
fn an_empty_registry_says_so() -> TestResult {
    let repo = Repo::new()?;
    let out = repo.stdout(&["project", "list"])?;
    assert!(out.starts_with("No projects registered yet."));
    Ok(())
}

#[test]
fn project_use_without_an_argument_clears_the_active_project() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    repo.djf(&["project", "use", "repo"]).assert().success();
    repo.djf(&["project", "use"])
        .assert()
        .success()
        .stdout("✓ Active project cleared\n");
    Ok(())
}

#[test]
fn project_use_of_an_unknown_project_lists_the_known_ones() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    repo.djf(&["project", "use", "x"])
        .assert()
        .code(1)
        .stdout("✗ No project 'x'. Registered: repo\n");
    Ok(())
}

#[test]
fn project_alias_rejects_an_invalid_alias_and_accepts_a_valid_one() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    repo.djf(&["project", "alias", "repo", "BAD_ONE"])
        .assert()
        .code(1)
        .stdout("✗ Alias 'BAD_ONE' must be lowercase letters, digits and hyphens.\n");
    repo.djf(&["project", "alias", "repo", "my-repo"])
        .assert()
        .success()
        .stdout("✓ repo is now also 'my-repo'\n");
    Ok(())
}

#[test]
fn project_tag_sets_and_removes_a_tag() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    repo.djf(&["project", "tag", "repo", "web"])
        .assert()
        .success()
        .stdout("✓ repo: #web\n");
    assert!(repo.stdout(&["project", "list"])?.contains("repo  #web\n"));
    repo.djf(&["project", "tag", "repo", "web", "-r"])
        .assert()
        .success()
        .stdout("✓ repo: (none)\n");
    Ok(())
}

#[test]
fn ids_do_not_collide_for_same_named_folders() -> TestResult {
    let repo = Repo::new()?;
    let first = folder(&repo, "a/web")?.display().to_string();
    let second = folder(&repo, "b/web")?.display().to_string();
    repo.djf(&["project", "register", &first])
        .assert()
        .success();
    repo.djf(&["project", "register", &second])
        .assert()
        .success()
        .stdout(format!(
            "✓ Registered web-2\n  {second}\n  Use it from any folder:  djf project use web-2\n"
        ));
    let listing = repo.stdout(&["project", "list"])?;
    assert!(listing.contains(" web\n") || listing.contains("web  "));
    assert!(listing.contains("web-2"));
    Ok(())
}

#[test]
fn project_lookup_is_case_insensitive() -> TestResult {
    let repo = Repo::new()?;
    let path = folder(&repo, "MatilyRec")?.display().to_string();
    repo.djf(&["project", "register", &path, "-a", "mrec"])
        .assert()
        .success();
    for name in ["matilyrec", "MatilyRec", "MATILYREC", "mrec", "MRec"] {
        let out = repo.stdout(&["project", "use", name])?;
        assert!(out.starts_with("✓ Active project: mrec\n"), "{name}: {out}");
    }
    Ok(())
}

#[test]
fn a_registered_project_whose_memory_is_gone_is_flagged() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    assert!(
        !repo
            .stdout(&["project", "list"])?
            .contains("no memory here")
    );
    fs::remove_dir_all(repo.mem())?;
    let listing = repo.stdout(&["project", "list"])?;
    assert!(listing.contains("   ⚠ no memory here\n"));
    Ok(())
}

#[test]
fn the_upgrade_notice_fires_once() -> TestResult {
    let repo = Repo::new()?;
    fs::create_dir_all(repo.registry())?;
    fs::write(repo.registry().join("projects.json"), "[]")?;
    fs::write(
        repo.registry().join("projects.meta.json"),
        r#"{"by_path": {}, "last_version": "0.2.0"}"#,
    )?;
    let first = repo.stdout(&["project", "list"])?;
    let second = repo.stdout(&["project", "list"])?;
    assert!(first.starts_with(NOTICE), "{first:?}");
    assert!(!second.contains("upgraded to"));
    Ok(())
}

#[test]
fn no_notice_on_a_first_ever_run() -> TestResult {
    let repo = Repo::new()?;
    let out = repo.stdout(&["project", "list"])?;
    assert!(!out.contains("upgraded to") && !out.contains("djf doctor"));
    let again = repo.stdout(&["project", "list"])?;
    assert!(!again.contains("upgraded to"));
    Ok(())
}
