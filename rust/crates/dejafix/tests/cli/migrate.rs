use std::fs;

use crate::support::{Repo, TestResult};

const OLD_HOOK: &str = "#!/bin/sh\n\
echo user hook\n\
# >>> projectmem auto-capture >>>\n\
pjm auto-capture\n\
# <<< projectmem auto-capture <<<\n";

fn legacy() -> Result<Repo, Box<dyn std::error::Error>> {
    let repo = Repo::bare()?;
    let memory = repo.root.join(".projectmem");
    fs::create_dir_all(memory.join("issues"))?;
    fs::write(
        memory.join("events.jsonl"),
        "{\"location\": \".projectmem/summary.md\", \"summary\": \"keep me\"}\n",
    )?;
    fs::write(repo.root.join(".git/hooks/pre-commit"), OLD_HOOK)?;
    let old_home = repo.home.join(".projectmem");
    fs::create_dir_all(old_home.join("global"))?;
    fs::write(old_home.join("global/library_gotchas.jsonl"), "{}\n")?;
    fs::write(
        old_home.join("projects.json"),
        serde_json::json!([repo.root.display().to_string()]).to_string(),
    )?;
    Ok(repo)
}

#[test]
fn a_dry_run_lists_the_steps_and_changes_nothing() -> TestResult {
    let repo = legacy()?;
    let out = repo.stdout(&["migrate"])?;
    let old_home = repo.home.join(".projectmem");
    assert_eq!(
        out,
        format!(
            "Dry run — nothing is changed. Re-run with --apply to migrate.\n\nHome: {} -> {}\n  move global\n  move projects.json\nProject repo ({}):\n  rename .projectmem/ -> .dejafix/\n  update .projectmem paths recorded in 1 memory files\n  reinstall git hooks: pre-commit\n",
            old_home.display(),
            repo.registry().display(),
            repo.root.display()
        )
    );
    assert!(repo.root.join(".projectmem").is_dir());
    assert!(!repo.registry().join("projects.json").exists());
    assert!(repo.home.join(".projectmem/projects.json").is_file());
    Ok(())
}

#[test]
fn apply_moves_everything_and_a_second_run_is_a_no_op() -> TestResult {
    let repo = legacy()?;
    let out = repo.stdout(&["migrate", "--apply"])?;
    assert!(out.starts_with("Close running agent sessions"));
    assert!(out.contains("dejafix git hooks refreshed: pre-commit\n"));
    assert!(out.ends_with(
        "\nDone. Commit the renamed .dejafix/ folder in each project, and uninstall projectmem once your clients point at dejafix.\n"
    ));
    assert!(
        repo.read(".dejafix/events.jsonl")?
            .contains("\".dejafix/summary.md\"")
    );
    assert!(
        repo.registry()
            .join("global/library_gotchas.jsonl")
            .is_file()
    );
    assert!(!repo.home.join(".projectmem").exists());
    let hook = repo.read(".git/hooks/pre-commit")?;
    assert!(hook.contains("echo user hook") && hook.contains("# >>> dejafix auto-capture >>>"));
    let again = repo.stdout(&["migrate", "--apply"])?;
    assert!(again.contains(&format!(
        "Project repo ({}): already migrated.\n",
        repo.root.display()
    )));
    Ok(())
}

#[test]
fn an_unregistered_project_is_given_by_a_relative_path() -> TestResult {
    let repo = Repo::bare()?;
    fs::create_dir_all(repo.root.join(".projectmem"))?;
    let out = repo.stdout(&["migrate", "-p", "."])?;
    assert!(out.contains(&format!(
        "Home: {} not found, nothing to move.\nProject repo ({}):\n  rename .projectmem/ -> .dejafix/\n",
        repo.home.join(".projectmem").display(),
        repo.root.display()
    )));
    Ok(())
}

#[test]
fn legacy_variables_and_configs_are_reported() -> TestResult {
    let repo = Repo::bare()?;
    let config = repo.home.join(".codex/config.toml");
    fs::create_dir_all(config.parent().ok_or("no parent")?)?;
    fs::write(&config, "[mcp_servers.projectmem]\ncommand = \"pjm-mcp\"\n")?;
    let output = repo
        .djf(&["migrate"])
        .env("PROJECTMEM_ROOT", "/x")
        .env("PJM_EDIT_HOOK_LOG", "/y")
        .output()?;
    let out = String::from_utf8(output.stdout)?;
    assert!(out.contains(&format!(
        "\nStill pointing at projectmem — update these by hand:\n  Codex: {}\n  environment variable PJM_EDIT_HOOK_LOG\n  environment variable PROJECTMEM_ROOT\n",
        config.display()
    )));
    Ok(())
}

#[test]
fn a_write_failure_exits_with_an_io_error() -> TestResult {
    let repo = legacy()?;
    fs::write(repo.home.join("blocker"), "x")?;
    let output = repo
        .djf(&["migrate", "--apply"])
        .env("DEJAFIX_HOME", repo.home.join("blocker/home"))
        .output()?;
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8(output.stderr)?.starts_with("Error: cannot write "));
    assert!(repo.root.join(".projectmem").is_dir());
    Ok(())
}
