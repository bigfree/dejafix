<!-- mcp-name: io.github.bigfree/dejafix -->
# dejafix

**Local-first memory and judgment for AI coding agents.** dejafix records issues,
fix attempts, decisions and notes as typed events in plain text, serves them to
agents through one MCP server for all your projects, and warns before an
approach that already failed is tried again — at edit time and at commit time.

> **dejafix is a fork of [projectmem](https://github.com/riponcm/projectmem)**
> by Ripon Chandra Malo, released under the MIT License. It started from
> projectmem 0.3.5 and continues independently under a new name. The original
> copyright notice is kept in [LICENSE](LICENSE). Coming from projectmem? See
> [Migrating from projectmem](#migrating-from-projectmem).

## Install

```bash
pipx install git+https://github.com/bigfree/dejafix
```

This installs four commands: `dejafix` (and its short alias `djf`), `djf-mcp`
(the MCP server) and `djf-edit-hook` (the edit-time hook for Claude Code and
Codex). Python 3.10 or newer is required.

## Quick start

```bash
cd your-project
djf init
```

`djf init` creates `.dejafix/`, registers the project, installs git hooks,
writes a short bridge block into `CLAUDE.md` and `AGENTS.md`, and prints an MCP
config block with your own Python path filled in.

## MCP server

Configure the server once; it serves every registered project.

Claude Desktop, Cursor, Claude Code (`~/.claude.json`) and other JSON clients:

```json
"mcpServers": {
  "dejafix": {
    "command": "/absolute/path/to/python",
    "args": ["-m", "dejafix.mcp_server"]
  }
}
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.dejafix]
command = "/absolute/path/to/python"
args = ["-m", "dejafix.mcp_server"]
```

Use the Python interpreter dejafix is installed into (with pipx:
`pipx environment --value PIPX_LOCAL_VENVS` + `/dejafix/bin/python`). With
several projects registered, agents pass `project="<name>"`, or you set a
default with `djf project use <name>`. `djf doctor` finds unregistered
projects, stale entries and configs pinned to one repository.

### Tools

Session: `start_session` (rules, summary and plan in one call),
`get_instructions`, `get_summary`, `get_project_map`, `get_plan`.
Before editing: `precheck_file`, `precheck_files`.
Lookup: `get_issue`, `search_events`, `get_context`, `get_score`,
`get_global_gotchas`, `list_projects`, `current_project`.
Writing: `log_issue`, `record_attempt`, `record_fix`, `add_decision`
(with `supersedes` to retire an older decision), `add_note`.

## Edit hook (Claude Code and Codex)

`djf-edit-hook` shows a file's lessons — failed and partial attempts, open
issues and decisions that name it — right before the agent edits that file.
Each lesson is shown once per session and again after the context is compacted.

Claude Code (`~/.claude/settings.json`):

```json
"hooks": {
  "PreToolUse": [
    { "matcher": "Edit|Write|MultiEdit",
      "hooks": [{ "type": "command", "command": "/absolute/path/to/djf-edit-hook", "timeout": 10 }] }
  ],
  "SessionStart": [
    { "matcher": "compact",
      "hooks": [{ "type": "command", "command": "/absolute/path/to/djf-edit-hook", "timeout": 10 }] }
  ]
}
```

Codex (`~/.codex/hooks.json`): the same command on `PreToolUse` with matcher
`apply_patch`, and on `PostCompact`.

Set `DJF_EDIT_HOOK_LOG=/path/to/calls.jsonl` to log every hook call.

## CLI

| Area | Commands |
|---|---|
| Memory | `log`, `attempt`, `fix`, `decision`, `note`, `show`, `search`, `regenerate` |
| Judgment | `precheck`, `brief`, `context`, `score`, `stats` |
| Projects | `init`, `project`, `doctor`, `global`, `migrate` |
| Views | `dashboard`, `visualize`, `map`, `plan`, `instructions`, `export` |
| Automation | `hooks`, `watch`, `backfill`, `wrap` |

Run `djf <command> --help` for options.

## Migrating from projectmem

```bash
djf migrate            # dry run: lists every change
djf migrate --apply    # performs it
```

`djf migrate` moves `~/.projectmem/` to `~/.dejafix/`, and in every registered
project renames `.projectmem/` to `.dejafix/`, updates the paths recorded
inside, refreshes `AI_INSTRUCTIONS.md` (keeping a `.bak`), updates `.gitignore`,
and replaces the projectmem blocks in `CLAUDE.md`, `AGENTS.md` and the git
hooks. Use `--path <dir>` for a project that is not registered. It never
overwrites existing dejafix data, and skips a project whose old watcher is
still running (stop it with `pjm watch --stop` first).

Client configs are your own files, so `migrate` only lists the ones that still
mention projectmem. Update them by hand: the MCP server becomes
`-m dejafix.mcp_server` under the key `dejafix`, the edit hook becomes
`djf-edit-hook`, and the variables `PROJECTMEM_*` / `PJM_*` become
`DEJAFIX_*` / `DJF_*`. Then commit the renamed `.dejafix/` folder in each
project and uninstall projectmem.

## Privacy

Everything stays on your machine. By default the distilled files
(`summary.md`, `PROJECT_MAP.md`, `AI_INSTRUCTIONS.md`, `issues/`) are committed
so teammates' agents inherit them, while the raw log and runtime files
(`events.jsonl`, `watch.pid`, `watch.log`) are gitignored. Add `.dejafix/` to
`.gitignore` to keep all of it private. Secrets are redacted from recorded
events (`DEJAFIX_NO_REDACT=1` turns that off). See [SECURITY.md](SECURITY.md).

## Research

The design dejafix inherits is described in the projectmem paper. If you use
it in research, please cite the original work:

> **PROJECTMEM: A Local-First, Event-Sourced Memory and Judgment Layer for AI
> Coding Agents.** Ripon Chandra Malo, Tong Qiu — University of Utah.
> [arXiv:2606.12329](https://arxiv.org/abs/2606.12329)

## License

MIT. Copyright (c) 2026 projectmem contributors, Copyright (c) 2026 dejafix
contributors. See [LICENSE](LICENSE).
