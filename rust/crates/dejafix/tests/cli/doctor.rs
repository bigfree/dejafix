use std::fs;
use std::path::{Path, PathBuf};

use crate::support::{Repo, TestResult};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn registered(repo: &Repo) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = repo.registry().join("projects.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn memory(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(path.join(".dejafix/issues"))?;
    fs::write(path.join(".dejafix/config.toml"), "recent_days = 30\n")?;
    fs::write(path.join(".dejafix/events.jsonl"), "")?;
    Ok(path.to_path_buf())
}

fn version_block() -> String {
    format!(
        "\n  ✓ Running {VERSION}\n      Check PyPI for a newer release:  djf doctor --online\n      Check daily from now on:         djf doctor --auto\n      (dejafix makes no network calls unless you ask.)\n\n"
    )
}

#[test]
fn doctor_reports_then_registers_what_it_finds() -> TestResult {
    let repo = Repo::bare()?;
    let work = repo.home.join("work");
    let one = memory(&work.join("one"))?;
    let path = work.display().to_string();
    let report = repo.stdout(&["doctor", "--path", &path, "--depth", "2"])?;
    assert_eq!(
        report,
        format!(
            "dejafix {VERSION} — checking your setup\n\nScanning 1 location(s) for projects…\n  ⚠ 1 project(s) have memory but are not registered\n      {}\n\n  ✓ Every registered project still has its memory\n\n  ✓ No MCP client config is pinned to a single repo\n{}Run `djf doctor --fix` to apply the fixes above.\n",
            one.display(),
            version_block()
        )
    );
    assert_eq!(registered(&repo)?.len(), 0);
    let fixed = repo.stdout(&["doctor", "--fix", "-p", &path, "-d", "2"])?;
    assert!(fixed.contains("  ✓ Registered 1\n"));
    assert!(fixed.ends_with("Fixed what could be fixed automatically.\n"));
    assert_eq!(registered(&repo)?, [one.display().to_string()]);
    let clean = repo.stdout(&["doctor", "-p", &path])?;
    assert!(clean.contains("  ✓ All 1 project(s) found are registered\n"));
    assert!(clean.ends_with("Everything checks out.\n"));
    Ok(())
}

#[test]
fn the_default_depth_reaches_a_nested_repo_folder() -> TestResult {
    let repo = Repo::bare()?;
    let drive = repo.home.join("drive");
    let deep = memory(&drive.join("repo/Group/Project"))?;
    repo.djf(&["doctor", "--fix", "--path", &drive.display().to_string()])
        .assert()
        .success();
    assert_eq!(registered(&repo)?, [deep.display().to_string()]);
    Ok(())
}

#[cfg(unix)]
#[test]
fn the_same_folder_reached_twice_registers_once() -> TestResult {
    let repo = Repo::bare()?;
    let work = repo.home.join("work");
    memory(&work.join("api"))?;
    let again = repo.home.join("work-again");
    std::os::unix::fs::symlink(&work, &again)?;
    let out = repo.stdout(&[
        "doctor",
        "--fix",
        "--path",
        &work.display().to_string(),
        "--path",
        &again.display().to_string(),
    ])?;
    assert!(out.contains("Scanning 2 location(s) for projects…\n"));
    assert_eq!(registered(&repo)?.len(), 1);
    Ok(())
}

#[test]
fn doctor_prunes_entries_whose_memory_is_gone() -> TestResult {
    let repo = Repo::bare()?;
    let gone = memory(&repo.home.join("deleted"))?;
    repo.djf(&["project", "register", &gone.display().to_string()])
        .assert()
        .success();
    fs::remove_dir_all(gone.join(".dejafix"))?;
    let empty = repo.home.join("empty-dir").display().to_string();
    let out = repo.stdout(&["doctor", "--fix", "--path", &empty, "--depth", "1"])?;
    assert!(out.contains(&format!(
        "\n  ⚠ 1 registered project(s) no longer have memory\n      deleted  {}\n  ✓ Removed 1 from the registry (repos untouched)\n",
        gone.display()
    )));
    assert_eq!(registered(&repo)?.len(), 0);
    Ok(())
}

#[test]
fn doctor_tells_you_to_quit_the_client_before_editing_and_notices_a_revert() -> TestResult {
    let repo = Repo::bare()?;
    let config = repo.home.join(".cursor/mcp.json");
    fs::create_dir_all(repo.home.join(".cursor"))?;
    fs::write(&config, r#"{"mcpServers":{"dejafix":{"args":[]}}}"#)?;
    let empty = repo.home.join("nothing").display().to_string();
    repo.djf(&["doctor", "-p", &empty]).assert().success();
    fs::write(
        &config,
        r#"{"mcpServers":{"dejafix":{"args":["--root","/x"]}}}"#,
    )?;
    let out = repo.stdout(&["doctor", "-p", &empty])?;
    assert!(out.contains(&format!(
        "\n  ⚠ MCP client config(s) still pinned to a single repo\n      Cursor  {}\n      Remove --root / cwd / DEJAFIX_ROOT from the dejafix entry.\n      dejafix never edits client settings — that one is yours to change.\n      Quit the client completely before editing, then re-run `djf doctor`.\n      ! Cursor was clean on ",
        config.display()
    )));
    assert!(
        out.contains(" and is pinned again — the app most likely rewrote this file on exit.\n")
    );
    let state: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        repo.registry().join("doctor-state.json"),
    )?)?;
    assert_eq!(state[config.display().to_string()]["pinned"], true);
    Ok(())
}

#[test]
fn doctor_reports_unreadable_configs_and_corrupt_lines() -> TestResult {
    let repo = Repo::new()?;
    fs::create_dir_all(repo.home.join(".codex/config.toml"))?;
    fs::write(
        repo.mem().join("events.jsonl"),
        "{broken\n2\n3\n4\n5\n6\n7\n",
    )?;
    repo.djf(&["project", "register"]).assert().success();
    let empty = repo.home.join("nothing").display().to_string();
    let out = repo.stdout(&["doctor", "--fix", "-p", &empty])?;
    let events = repo.mem().join("events.jsonl");
    assert!(out.contains(&format!(
        "\n  ⚠ Could not read some client config(s) — not checked\n      Codex  {}\n      A config that cannot be read is not a config that is fine.\n",
        repo.home.join(".codex/config.toml").display()
    )));
    assert!(out.contains(&format!(
        "\n  ⚠ 7 line(s) of {} could not be read\n      line 1: line is not valid JSON\n      line 2: line is not a JSON object\n      line 3: line is not a JSON object\n      line 4: line is not a JSON object\n      line 5: line is not a JSON object\n      … and 2 more\n      The log is never rewritten; fix or remove those lines by hand.\n\n  ✓ Running ",
        events.display()
    )));
    assert!(out.ends_with("Fixed what could be fixed automatically.\n"));
    assert_eq!(fs::read_to_string(events)?, "{broken\n2\n3\n4\n5\n6\n7\n");
    Ok(())
}

#[test]
fn doctor_output_is_utf8_and_a_negative_depth_finds_nothing() -> TestResult {
    let repo = Repo::new()?;
    let output = repo
        .djf(&["doctor", "-p", &repo.root.display().to_string(), "-d", "-1"])
        .output()?;
    assert_eq!(output.status.code(), Some(0));
    let out = String::from_utf8(output.stdout)?;
    assert!(out.contains("— checking your setup") && !out.contains('\u{fffd}'));
    assert!(out.contains("  ✓ All 0 project(s) found are registered\n"));
    Ok(())
}

#[test]
fn scan_finds_projects_the_registry_never_knew_about() -> TestResult {
    let repo = Repo::bare()?;
    let work = repo.home.join("work");
    let one = memory(&work.join("one"))?;
    let two = memory(&work.join("nested/two"))?;
    memory(&work.join("node_modules/pkg"))?;
    fs::create_dir_all(work.join("plain"))?;
    let path = work.display().to_string();
    let preview = repo.stdout(&["project", "scan", &path, "--dry-run"])?;
    assert!(preview.starts_with(&format!("Scanning {path} (depth 4)…\n")));
    assert!(preview.contains(&format!("  + {}\n", one.display())));
    assert!(preview.contains(&format!("  + {}\n", two.display())));
    assert!(preview.ends_with("\n2 would be registered. Re-run without --dry-run.\n"));
    assert_eq!(registered(&repo)?.len(), 0);
    let result = repo.stdout(&["project", "scan", &path])?;
    assert!(result.ends_with(
        "\n✓ Registered 2 project(s).\n  One MCP server now reaches them all — `djf project list`.\n"
    ));
    let mut paths = registered(&repo)?;
    paths.sort();
    assert_eq!(
        paths,
        [two.display().to_string(), one.display().to_string()]
    );
    Ok(())
}

#[test]
fn scan_is_idempotent() -> TestResult {
    let repo = Repo::bare()?;
    let work = repo.home.join("work");
    let one = memory(&work.join("one"))?;
    let path = work.display().to_string();
    repo.djf(&["project", "scan", &path]).assert().success();
    let second = repo.stdout(&["project", "scan", &path])?;
    assert_eq!(
        second,
        format!(
            "Scanning {path} (depth 4)…\n    {}  (already registered)\n\n✓ All 1 already registered.\n",
            one.display()
        )
    );
    assert_eq!(registered(&repo)?.len(), 1);
    Ok(())
}

#[test]
fn scan_accepts_several_roots() -> TestResult {
    let repo = Repo::bare()?;
    let first = repo.home.join("d-drive");
    let second = repo.home.join("e-drive");
    let alpha = memory(&first.join("alpha"))?;
    let beta = memory(&second.join("beta"))?;
    repo.djf(&[
        "project",
        "scan",
        &first.display().to_string(),
        &second.display().to_string(),
        "--depth",
        "2",
    ])
    .assert()
    .success();
    assert_eq!(
        registered(&repo)?,
        [alpha.display().to_string(), beta.display().to_string()]
    );
    Ok(())
}

#[test]
fn scan_refuses_a_missing_root_and_reports_an_empty_one() -> TestResult {
    let repo = Repo::bare()?;
    let missing = repo.home.join("missing");
    repo.djf(&["project", "scan", &missing.display().to_string()])
        .assert()
        .code(1)
        .stdout(format!("✗ {} is not a directory.\n", missing.display()));
    let empty = repo.home.join("empty");
    fs::create_dir_all(&empty)?;
    repo.djf_in(&empty, &["project", "scan", "-n", "-d", "1"])
        .assert()
        .success()
        .stdout(format!(
            "Scanning {} (depth 1)…\nNo projects with memory found here.\n",
            empty.display()
        ));
    Ok(())
}
