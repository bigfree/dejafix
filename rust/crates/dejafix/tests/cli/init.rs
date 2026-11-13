use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::Path;

use crate::support::{self, Repo, TestResult};

const STALE: &str = "# dejafix AI Instructions\n\n\
Call `precheck_file(path)` before editing.\n\n\
## Global Memory — Inherited Knowledge\n\n\
### Library Gotchas\n\n\
- **vite**: dev server caches deps (from other-app)\n\n\
## Rules summary\n\n\
- old rule\n";

fn escaped(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let quoted = serde_json::to_string(path.to_str().ok_or("path is not utf-8")?)?;
    Ok(quoted[1..quoted.len() - 1].to_owned())
}

fn init_stdout(repo: &Repo, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let mut full = vec!["init"];
    full.extend_from_slice(args);
    let output = repo.djf(&full).assert().success().get_output().clone();
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn init_creates_the_memory_files_and_gitignores_the_log() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &[])?;
    assert!(out.starts_with(&format!(
        "Initialized {}",
        repo.root.join(".dejafix").display()
    )));
    let mem = repo.mem();
    for file in [
        "config.toml",
        "summary.md",
        "AI_INSTRUCTIONS.md",
        "PROJECT_MAP.md",
        "events.jsonl",
    ] {
        assert!(mem.join(file).is_file(), "{file}");
    }
    assert!(mem.join("issues").is_dir());
    assert!(repo.read(".gitignore")?.contains(".dejafix/events.jsonl"));
    let summary = fs::read_to_string(mem.join("summary.md"))?;
    let instructions = fs::read_to_string(mem.join("AI_INSTRUCTIONS.md"))?;
    let project_map = fs::read_to_string(mem.join("PROJECT_MAP.md"))?;
    assert!(summary.contains("Project purpose") && summary.contains("AI assistants"));
    assert!(instructions.contains("Start of every session"));
    assert!(instructions.contains("PROJECT_MAP.md"));
    assert!(instructions.contains("djf attempt") && instructions.contains("record_attempt"));
    assert!(project_map.contains("Status: not created yet"));
    assert!(project_map.contains("## Structure") && project_map.contains("## Relationships"));
    Ok(())
}

#[test]
fn instructions_command_prints_the_ai_protocol() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let out = repo.stdout(&["instructions"])?;
    assert!(out.contains("dejafix AI Instructions"));
    assert!(out.contains("djf fix") && out.contains("record_fix"));
    assert_eq!(
        out,
        format!("{}\n", repo.read(".dejafix/AI_INSTRUCTIONS.md")?)
    );
    Ok(())
}

#[test]
fn map_command_prints_the_project_map() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let out = repo.stdout(&["map"])?;
    assert!(out.contains("Project Map") && out.contains("Status: not created yet"));
    assert_eq!(out, format!("{}\n", repo.read(".dejafix/PROJECT_MAP.md")?));
    Ok(())
}

#[test]
fn map_build_flag_is_rejected() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["map", "--build"]).assert().code(2);
    Ok(())
}

#[test]
fn retired_init_flags_are_accepted_as_no_ops() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init", "--no-watch", "--no-backfill", "--no-structure"])
        .assert()
        .success();
    assert!(repo.mem().join("events.jsonl").is_file());
    Ok(())
}

#[test]
fn init_without_hooks_never_mentions_them() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &["--no-hooks"])?;
    assert!(!out.to_lowercase().contains("hook"));
    assert!(!repo.root.join(".git/hooks/pre-commit").exists());
    Ok(())
}

#[test]
fn init_creates_plan_md_with_the_core_sections() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let text = repo.read(".dejafix/plan.md")?;
    assert!(text.contains("— plan"));
    for section in ["## Ideas", "## Active plans", "## Next", "## Shipped"] {
        assert!(text.contains(section), "{section}");
    }
    assert!(text.contains("NOT the event log"));
    Ok(())
}

#[test]
fn plan_md_is_committed_not_gitignored() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    assert!(!repo.read(".gitignore")?.contains("plan.md"));
    Ok(())
}

#[test]
fn plan_add_appends_to_the_ideas_section() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let path = repo.mem().join("plan.md");
    repo.djf(&["plan", "add an export/migration adapter"])
        .assert()
        .success()
        .stdout(format!("Added to Ideas in {}\n", path.display()));
    let text = fs::read_to_string(&path)?;
    assert!(text.contains("- add an export/migration adapter"));
    let ideas = text
        .split("## Ideas")
        .nth(1)
        .and_then(|rest| rest.split("##").next())
        .ok_or("no ideas section")?;
    assert!(ideas.contains("add an export/migration adapter"));
    Ok(())
}

#[test]
fn plan_add_does_not_touch_events() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let before = repo.read(".dejafix/events.jsonl")?;
    repo.djf(&["plan", "ship the plan.md feature"])
        .assert()
        .success();
    assert_eq!(repo.read(".dejafix/events.jsonl")?, before);
    Ok(())
}

#[test]
fn plan_without_an_argument_prints_the_file() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let out = repo.stdout(&["plan"])?;
    assert_eq!(out, format!("{}\n", repo.read(".dejafix/plan.md")?));
    Ok(())
}

#[test]
fn a_fresh_init_writes_the_current_instructions_without_a_backup() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &[])?;
    assert!(
        repo.read(".dejafix/AI_INSTRUCTIONS.md")?
            .contains("<!-- dejafix-instructions: ")
    );
    assert!(!repo.mem().join("AI_INSTRUCTIONS.md.bak").exists());
    assert!(!out.contains("Refreshed"));
    Ok(())
}

#[test]
fn init_refreshes_a_stale_file_keeping_a_backup_and_the_global_block() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &["--no-global"])?;
    let path = repo.mem().join("AI_INSTRUCTIONS.md");
    fs::write(&path, STALE)?;
    let out = init_stdout(&repo, &["--no-global"])?;
    let text = fs::read_to_string(&path)?;
    assert!(!text.contains("precheck_file(path)"));
    assert!(text.contains("precheck_file(file_path)"));
    assert!(text.contains("<!-- dejafix-instructions: "));
    assert_eq!(
        text.matches("## Global Memory — Inherited Knowledge")
            .count(),
        1
    );
    assert!(text.contains("- **vite**: dev server caches deps (from other-app)"));
    let global = text.find("Global Memory").ok_or("no global block")?;
    let rules = text.find("## Rules summary").ok_or("no rules summary")?;
    assert!(global < rules);
    assert_eq!(
        support::text(&repo.mem().join("AI_INSTRUCTIONS.md.bak"))?,
        STALE
    );
    assert!(out.contains(
        "  Refreshed .dejafix/AI_INSTRUCTIONS.md to the current guidance (previous copy saved as AI_INSTRUCTIONS.md.bak).\n"
    ));
    assert!(
        repo.read(".gitignore")?
            .contains(".dejafix/AI_INSTRUCTIONS.md.bak")
    );
    Ok(())
}

#[test]
fn init_leaves_a_current_file_with_user_edits_untouched() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &["--no-global"])?;
    let path = repo.mem().join("AI_INSTRUCTIONS.md");
    let edited = format!(
        "{}\n## House rules\n\nAlways squash.\n",
        fs::read_to_string(&path)?
    );
    fs::write(&path, &edited)?;
    let out = init_stdout(&repo, &["--no-global"])?;
    assert_eq!(fs::read_to_string(&path)?, edited);
    assert!(!repo.mem().join("AI_INSTRUCTIONS.md.bak").exists());
    assert!(!out.contains("Refreshed"));
    Ok(())
}

#[test]
fn init_registers_the_project() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    let registry: Vec<String> =
        serde_json::from_str(&fs::read_to_string(repo.registry().join("projects.json"))?)?;
    assert_eq!(registry, [repo.root.to_str().ok_or("utf-8")?]);
    assert!(repo.registry().join("projects.meta.json").is_file());
    Ok(())
}

#[test]
fn registering_a_project_twice_keeps_one_entry() -> TestResult {
    let repo = Repo::bare()?;
    init_stdout(&repo, &[])?;
    init_stdout(&repo, &[])?;
    repo.djf(&["project", "register"]).assert().success();
    let registry: Vec<String> =
        serde_json::from_str(&fs::read_to_string(repo.registry().join("projects.json"))?)?;
    assert_eq!(registry.len(), 1);
    Ok(())
}

#[test]
fn stack_detect_package_json_populates_the_map() -> TestResult {
    let repo = Repo::bare()?;
    fs::write(
        repo.root.join("package.json"),
        r#"{"name":"demo","description":"A tiny Node app","scripts":{"start":"node index.js","test":"jest"},"dependencies":{"react":"^18.0.0","next":"^14.0.0"}}"#,
    )?;
    let out = init_stdout(&repo, &[])?;
    let map = repo.read(".dejafix/PROJECT_MAP.md")?;
    assert!(map.contains("A tiny Node app"));
    assert!(map.contains("react") || map.contains("nextjs"));
    assert!(map.contains("`npm run start`"));
    assert!(out.contains("  PROJECT_MAP.md: pre-populated from project manifests ("));
    Ok(())
}

#[test]
fn stack_detect_pyproject_populates_the_map() -> TestResult {
    let repo = Repo::bare()?;
    fs::write(
        repo.root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0.0.1\"\n\
description = \"A demo CLI built with Typer and pydantic.\"\n\
dependencies = [\"typer\", \"pydantic\", \"fastapi\"]\n\
[project.scripts]\ndemo = \"demo.cli:main\"\n",
    )?;
    fs::create_dir(repo.root.join("src"))?;
    fs::create_dir(repo.root.join("tests"))?;
    init_stdout(&repo, &[])?;
    let map = repo.read(".dejafix/PROJECT_MAP.md")?;
    assert!(map.contains("auto-detected"));
    assert!(map.contains("A demo CLI built with Typer and pydantic."));
    assert!(map.contains("fastapi"));
    assert!(map.contains("## Structure"));
    assert!(map.contains("`src/`") && map.contains("`tests/`"));
    assert!(map.contains("`demo`"));
    assert!(!map.contains("Status: not created yet"));
    Ok(())
}

#[test]
fn stack_detect_never_overwrites_a_user_edited_map() -> TestResult {
    let repo = Repo::bare()?;
    fs::create_dir_all(repo.mem().join("issues"))?;
    fs::write(repo.mem().join("config.toml"), "recent_days = 30\n")?;
    let custom = "# My Custom Project Map\n\nStatus: hand-written\n\nKeep this!";
    fs::write(repo.mem().join("PROJECT_MAP.md"), custom)?;
    fs::write(
        repo.root.join("pyproject.toml"),
        "[project]\ndescription = \"Should NOT appear\"\n",
    )?;
    let out = init_stdout(&repo, &[])?;
    assert_eq!(repo.read(".dejafix/PROJECT_MAP.md")?, custom);
    assert!(!out.contains("pre-populated"));
    Ok(())
}

#[test]
fn stack_detect_survives_a_corrupt_manifest() -> TestResult {
    let repo = Repo::bare()?;
    fs::write(repo.root.join("package.json"), "not valid {{{")?;
    let out = init_stdout(&repo, &[])?;
    assert!(out.contains("Next:"));
    assert!(repo.mem().join("PROJECT_MAP.md").is_file());
    Ok(())
}

#[test]
fn stack_detect_does_nothing_without_manifests() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &[])?;
    assert!(
        repo.read(".dejafix/PROJECT_MAP.md")?
            .contains("Status: not created yet")
    );
    assert!(!out.contains("pre-populated"));
    Ok(())
}

#[test]
fn no_stack_detect_leaves_the_placeholder_map() -> TestResult {
    let repo = Repo::bare()?;
    fs::write(
        repo.root.join("package.json"),
        r#"{"name":"demo","description":"A tiny Node app"}"#,
    )?;
    init_stdout(&repo, &["--no-stack-detect"])?;
    let map = repo.read(".dejafix/PROJECT_MAP.md")?;
    assert!(map.contains("Status: not created yet") && !map.contains("A tiny Node app"));
    Ok(())
}

#[test]
fn the_mcp_block_names_the_sibling_server_and_no_root() -> TestResult {
    let repo = Repo::bare_named("myproj")?;
    let out = init_stdout(&repo, &[])?;
    let server =
        Path::new(env!("CARGO_BIN_EXE_djf")).with_file_name(format!("djf-mcp{EXE_SUFFIX}"));
    assert!(out.contains("\"mcpServers\""));
    assert!(out.contains("\"dejafix\""));
    assert!(out.contains(&format!("\"command\": \"{}\"", escaped(&server)?)));
    assert!(server.is_absolute());
    assert!(out.contains("\"args\": []"));
    assert!(!out.contains("\"--root\""));
    assert!(out.contains("djf project use myproj"));
    for client in ["Claude Desktop", "Cursor", "Antigravity", "Codex"] {
        assert!(out.contains(client), "{client}");
    }
    assert!(out.contains("config.toml"));
    Ok(())
}

#[test]
fn the_single_project_block_still_pins_the_root() -> TestResult {
    let repo = Repo::bare_named("myproj")?;
    let out = init_stdout(&repo, &["--mcp-config-single"])?;
    let root = escaped(&repo.root)?;
    assert!(out.contains(&format!("\"args\": [\"--root\", \"{root}\"]")));
    assert!(out.contains(&format!("args = [\"--root\", \"{root}\"]")));
    assert!(out.contains("this project only"));
    assert!(!out.contains("djf project use myproj"));
    Ok(())
}

#[test]
fn the_mcp_command_is_an_absolute_path_to_djf_mcp() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &[])?;
    let command = out
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"command\": \""))
        .ok_or("no command line")?
        .trim_end_matches("\",");
    assert!(command.ends_with(&format!("djf-mcp{EXE_SUFFIX}")));
    assert!(Path::new(&command.replace("\\\\", "\\")).is_absolute());
    Ok(())
}

#[test]
fn no_mcp_config_suppresses_the_block() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &["--no-mcp-config"])?;
    assert!(!out.contains("mcpServers") && !out.contains("Client config file locations"));
    assert!(out.ends_with(
        "\n  Next: Start an AI session to refine PROJECT_MAP.md and capture decisions.\n"
    ));
    Ok(())
}

#[test]
fn init_warns_when_a_client_config_still_pins_one_repo() -> TestResult {
    let repo = Repo::bare()?;
    let config = repo.home.join(".cursor").join("mcp.json");
    fs::create_dir_all(config.parent().ok_or("no parent")?)?;
    let pinned =
        r#"{"mcpServers":{"dejafix":{"args":["-m","dejafix.mcp_server","--root","/old/repo"]}}}"#;
    fs::write(&config, pinned)?;
    let out = init_stdout(&repo, &[])?;
    assert!(out.contains("still pins dejafix to one repo"));
    assert!(out.contains(&format!("      Cursor  {}", config.display())));
    assert_eq!(fs::read_to_string(&config)?, pinned);
    Ok(())
}

#[test]
fn init_detects_a_pin_set_through_the_environment() -> TestResult {
    let repo = Repo::bare()?;
    let config = repo
        .home
        .join("Library/Application Support/Claude/claude_desktop_config.json");
    fs::create_dir_all(config.parent().ok_or("no parent")?)?;
    fs::write(
        &config,
        r#"{"mcpServers":{"dejafix":{"command":"python","args":["-m","dejafix.mcp_server"],"env":{"DEJAFIX_ROOT":"/old/repo"}}}}"#,
    )?;
    let out = init_stdout(&repo, &[])?;
    assert!(out.contains("still pins dejafix to one repo"));
    assert!(out.contains("Claude Desktop"));
    Ok(())
}

#[test]
fn a_client_config_that_does_not_pin_raises_no_warning() -> TestResult {
    let repo = Repo::bare()?;
    let config = repo.home.join(".cursor").join("mcp.json");
    fs::create_dir_all(config.parent().ok_or("no parent")?)?;
    fs::write(
        &config,
        r#"{"mcpServers":{"dejafix":{"command":"djf-mcp"}}}"#,
    )?;
    let out = init_stdout(&repo, &[])?;
    assert!(!out.contains("still pins dejafix to one repo"));
    Ok(())
}

#[test]
fn init_prints_the_codex_toml_form() -> TestResult {
    let repo = Repo::bare()?;
    let out = init_stdout(&repo, &[])?;
    let server =
        Path::new(env!("CARGO_BIN_EXE_djf")).with_file_name(format!("djf-mcp{EXE_SUFFIX}"));
    assert!(out.contains("[mcp_servers.dejafix]"));
    assert!(out.contains(&format!("command = \"{}\"", escaped(&server)?)));
    assert!(out.contains("args = []"));
    Ok(())
}

#[test]
fn linux_client_paths_are_scanned_and_xdg_is_honoured() -> TestResult {
    let repo = Repo::bare()?;
    let xdg = repo.home.with_file_name("elsewhere");
    let config = xdg.join("Claude").join("claude_desktop_config.json");
    fs::create_dir_all(config.parent().ok_or("no parent")?)?;
    fs::write(
        &config,
        r#"{"mcpServers":{"dejafix":{"args":["-m","dejafix.mcp_server","--root","/old/repo"]}}}"#,
    )?;
    let output = repo
        .djf(&["init"])
        .env("XDG_CONFIG_HOME", &xdg)
        .assert()
        .success()
        .get_output()
        .clone();
    let out = String::from_utf8(output.stdout)?;
    assert!(out.contains("still pins dejafix to one repo"));
    assert!(out.contains(&config.display().to_string()));
    Ok(())
}

#[test]
fn init_output_is_utf8_with_its_real_characters() -> TestResult {
    let repo = Repo::bare()?;
    let output = repo.djf(&["init"]).assert().success().get_output().clone();
    let out = String::from_utf8(output.stdout)?;
    assert!(out.contains('═') && out.contains('—'));
    assert!(out.contains("Client config file locations"));
    assert!(out.contains("Next:"));
    assert!(!out.contains('\u{fffd}'));
    assert_eq!(output.stderr.len(), 0);
    Ok(())
}
