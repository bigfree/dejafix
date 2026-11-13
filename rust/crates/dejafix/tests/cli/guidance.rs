use std::fs;

use crate::support::{Repo, TestResult};

const BRIDGE_START: &str = "<!-- >>> dejafix bridge >>> -->";
const BRIDGE_END: &str = "<!-- <<< dejafix bridge <<< -->";

fn block(repo: &Repo, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let text = repo.read(name)?;
    let start = text.find(BRIDGE_START).ok_or("no bridge start")?;
    let end = text.find(BRIDGE_END).ok_or("no bridge end")?;
    Ok(text[start..end].to_owned())
}

fn init_stdout(repo: &Repo) -> Result<String, Box<dyn std::error::Error>> {
    let output = repo.djf(&["init"]).assert().success().get_output().clone();
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn init_writes_the_registered_name_into_claude_md() -> TestResult {
    let repo = Repo::bare_named("checkout-api")?;
    repo.djf(&["init"]).assert().success();
    assert!(repo.read("CLAUDE.md")?.contains("checkout-api"));
    Ok(())
}

#[test]
fn init_writes_the_bridge_to_both_rule_files() -> TestResult {
    let repo = Repo::bare_named("checkout-api")?;
    let out = init_stdout(&repo)?;
    assert!(out.contains("  CLAUDE.md: created with dejafix bridge.\n"));
    assert!(out.contains("  AGENTS.md: created with dejafix bridge.\n"));
    for name in ["CLAUDE.md", "AGENTS.md"] {
        let text = repo.read(name)?;
        assert!(text.contains("dejafix (MANDATORY)"), "{name}");
        assert!(text.contains("checkout-api"), "{name}");
    }
    Ok(())
}

#[test]
fn both_rule_files_say_exactly_the_same_thing() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    assert_eq!(block(&repo, "CLAUDE.md")?, block(&repo, "AGENTS.md")?);
    Ok(())
}

#[test]
fn an_existing_agents_md_is_preserved() -> TestResult {
    let repo = Repo::bare()?;
    fs::write(
        repo.root.join("AGENTS.md"),
        "# House rules\n\nAlways run the tests.\n",
    )?;
    let out = init_stdout(&repo)?;
    assert!(out.contains("  AGENTS.md: dejafix bridge appended.\n"));
    let text = repo.read("AGENTS.md")?;
    assert!(text.contains("Always run the tests."));
    assert!(text.contains("dejafix (MANDATORY)"));
    Ok(())
}

#[test]
fn re_running_init_does_not_duplicate_the_block() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init"]).assert().success();
    repo.djf(&["init"]).assert().success();
    for name in ["CLAUDE.md", "AGENTS.md"] {
        assert_eq!(repo.read(name)?.matches(BRIDGE_START).count(), 1, "{name}");
    }
    Ok(())
}

#[test]
fn no_claude_md_skips_both_rule_files() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init", "--no-claude-md"]).assert().success();
    assert!(!repo.root.join("CLAUDE.md").exists());
    assert!(!repo.root.join("AGENTS.md").exists());
    Ok(())
}
