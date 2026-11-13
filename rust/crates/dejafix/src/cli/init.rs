use std::env;
use std::io::{self, Write};
use std::path::Path;

use dejafix_core::{
    BridgeOutcome, InitHooks, InitOptions, Settings, init_project, other_projects,
    pinned_client_configs,
};

use super::args::InitArgs;
use super::output::{ARROW, RULE_DOUBLE, WARN, json_escaped, secho};
use super::types::{McpBlock, Outcome, Platform, Style};
use crate::config::Config;

const RULE_WIDTH: usize = 62;
const MCP_SERVER: &str = "djf-mcp";

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

fn mcp_command() -> String {
    let name = format!("{MCP_SERVER}{}", env::consts::EXE_SUFFIX);
    let path = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join(&name)))
        .map_or(name, |path| path.to_string_lossy().into_owned());
    json_escaped(&path)
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn write_mcp_block(out: &mut impl Write, block: &McpBlock<'_>) -> io::Result<()> {
    let bar = RULE_DOUBLE.repeat(RULE_WIDTH);
    let root = json_escaped(&block.root.to_string_lossy());
    let command = block.command;
    writeln!(out)?;
    writeln!(out, "{bar}")?;
    if block.single {
        writeln!(out, "  MCP client configuration — this project only:")?;
    } else {
        writeln!(
            out,
            "  MCP client configuration — paste this ONCE for all projects:"
        )?;
    }
    writeln!(out)?;
    writeln!(out, "    {{")?;
    writeln!(out, "      \"mcpServers\": {{")?;
    writeln!(out, "        \"dejafix\": {{")?;
    writeln!(out, "          \"command\": \"{command}\",")?;
    if block.single {
        writeln!(out, "          \"args\": [\"--root\", \"{root}\"]")?;
    } else {
        writeln!(out, "          \"args\": []")?;
    }
    writeln!(out, "        }}")?;
    writeln!(out, "      }}")?;
    writeln!(out, "    }}")?;
    writeln!(out)?;
    if !block.single {
        write_shared_notes(out, block)?;
    }
    let (tilde, separator) = match block.platform {
        Platform::Windows => ("%USERPROFILE%\\", "\\"),
        Platform::Mac | Platform::Linux => ("~/", "/"),
    };
    writeln!(
        out,
        "  Codex uses TOML, not JSON — for {tilde}.codex{separator}config.toml:"
    )?;
    writeln!(out)?;
    writeln!(out, "    [mcp_servers.dejafix]")?;
    writeln!(out, "    command = \"{command}\"")?;
    if block.single {
        writeln!(out, "    args = [\"--root\", \"{root}\"]")?;
    } else {
        writeln!(out, "    args = []")?;
    }
    writeln!(out)?;
    writeln!(out, "  Client config file locations:")?;
    match block.platform {
        Platform::Mac => writeln!(
            out,
            "    Claude Desktop  ~/Library/Application Support/Claude/claude_desktop_config.json"
        )?,
        Platform::Windows => writeln!(
            out,
            "    Claude Desktop  %APPDATA%\\Claude\\claude_desktop_config.json"
        )?,
        Platform::Linux => writeln!(
            out,
            "    Claude Desktop  ~/.config/Claude/claude_desktop_config.json  ($XDG_CONFIG_HOME)"
        )?,
    }
    writeln!(
        out,
        "    Cursor          {tilde}.cursor{separator}mcp.json  (global; per-project is <repo>{separator}.cursor{separator}mcp.json)"
    )?;
    writeln!(
        out,
        "    Antigravity     {tilde}.gemini{separator}antigravity{separator}mcp_config.json  (legacy IDE; v2 path may differ)"
    )?;
    writeln!(
        out,
        "    Codex (TOML!)   {tilde}.codex{separator}config.toml"
    )?;
    writeln!(
        out,
        "    Claude Code     claude mcp add  (or {tilde}.claude.json)"
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "  After pasting: fully quit and restart your client (cold start)."
    )?;
    if !block.single {
        writeln!(
            out,
            "  Pinned to one repo instead:  djf init --mcp-config-single"
        )?;
    }
    writeln!(out, "{bar}")
}

fn write_shared_notes(out: &mut impl Write, block: &McpBlock<'_>) -> io::Result<()> {
    if block.others > 0 {
        writeln!(
            out,
            "  You already have {} other project(s) registered. The config",
            block.others
        )?;
        writeln!(out, "  above reaches all of them from one server.")?;
        writeln!(out)?;
    }
    if !block.pinned.is_empty() {
        secho(
            out,
            &format!("  {WARN} An existing config still pins dejafix to one repo:"),
            Style::Yellow,
            block.color,
        )?;
        for config in block.pinned {
            writeln!(out, "      {}  {}", config.client, config.path.display())?;
        }
        writeln!(
            out,
            "    Point that server at the config above — drop --root, cwd, and any"
        )?;
        writeln!(out, "    DEJAFIX_ROOT env entry —")?;
        writeln!(
            out,
            "    to serve every project, then fully restart the client. Leaving it as"
        )?;
        writeln!(
            out,
            "    it is keeps working, but only for the repo it points at."
        )?;
        writeln!(out)?;
    }
    let name = folder_name(block.root);
    writeln!(out, "  This project is registered as '{name}'. Your AI can")?;
    writeln!(
        out,
        "  pass project=\"<name>\" on any call, or you can set a default:"
    )?;
    writeln!(out, "    djf project use {name}")?;
    writeln!(out)
}

pub fn run(
    args: InitArgs,
    config: &Config,
    settings: &Settings,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<Outcome> {
    let InitArgs {
        no_hooks,
        no_global,
        no_watch: _,
        no_backfill: _,
        no_claude_md,
        no_stack_detect,
        no_mcp_config,
        mcp_config_single,
        no_structure: _,
        global_tags,
    } = args;
    let options = InitOptions {
        claude_md: !no_claude_md,
        stack_detect: !no_stack_detect,
        global: !no_global,
        global_tags,
        hooks: (!no_hooks).then(|| super::hooks::djf_binary(config)),
    };
    let root = &config.cwd;
    let report = match init_project(root, config.home.as_deref(), settings, &options) {
        Ok(report) => report,
        Err(error) => return Ok(Outcome::Failed(error)),
    };
    writeln!(out, "Initialized {}", report.mem_dir.display())?;
    if report.refreshed_instructions {
        writeln!(
            out,
            "  Refreshed .dejafix/AI_INSTRUCTIONS.md to the current guidance (previous copy saved as AI_INSTRUCTIONS.md.bak)."
        )?;
    }
    for (file, outcome) in &report.bridges {
        print_bridge(file, *outcome, out)?;
    }
    if let Some(suffix) = &report.project_map {
        writeln!(
            out,
            "  PROJECT_MAP.md: pre-populated from project manifests{suffix}."
        )?;
    }
    print_hooks(report.hooks.as_ref(), out, err)?;
    if let Some(inherited) = &report.inherited {
        writeln!(
            out,
            "\n  Global memory: Detected stack [{}]",
            inherited.tags.join(", ")
        )?;
        if inherited.gotchas > 0 {
            writeln!(
                out,
                "    {ARROW} {} library gotchas injected into AI_INSTRUCTIONS.md",
                inherited.gotchas
            )?;
        }
        if inherited.patterns > 0 {
            writeln!(
                out,
                "    {ARROW} {} patterns injected into AI_INSTRUCTIONS.md",
                inherited.patterns
            )?;
        }
    }
    if !no_mcp_config {
        let pinned = if mcp_config_single {
            Vec::new()
        } else {
            config
                .client_paths()
                .map(|paths| pinned_client_configs(&paths, None))
                .unwrap_or_default()
        };
        let command = mcp_command();
        write_mcp_block(
            out,
            &McpBlock {
                command: &command,
                root,
                single: mcp_config_single,
                others: if mcp_config_single {
                    0
                } else {
                    other_projects(settings, root)
                },
                pinned: &pinned,
                platform: Platform::current(),
                color: config.color,
            },
        )?;
    }
    writeln!(
        out,
        "\n  Next: Start an AI session to refine PROJECT_MAP.md and capture decisions."
    )?;
    Ok(Outcome::Done)
}

pub fn print_bridge(file: &str, outcome: BridgeOutcome, out: &mut impl Write) -> io::Result<()> {
    match outcome {
        BridgeOutcome::Created => writeln!(out, "  {file}: created with dejafix bridge."),
        BridgeOutcome::Appended => writeln!(out, "  {file}: dejafix bridge appended."),
        BridgeOutcome::Refreshed => writeln!(out, "  {file}: dejafix bridge refreshed."),
    }
}

fn print_hooks(
    hooks: Option<&InitHooks>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> io::Result<()> {
    match hooks {
        Some(InitHooks::Installed(hooks)) => {
            super::hooks::print_report(hooks, out, err)?;
            writeln!(
                out,
                "  Auto-capture active — events will be logged automatically."
            )
        }
        Some(InitHooks::NoGit) => writeln!(
            out,
            "  Note: No .git directory found. Run `djf hooks install` after `git init`."
        ),
        None => Ok(()),
    }
}
