<!-- mcp-name: io.github.bigfree/dejafix -->
# dejafix

**Local-first memory and judgment for AI coding agents.** dejafix records
issues, fix attempts, decisions and notes as typed events in plain text. It
serves them to agents through one MCP server for all your projects, and warns
before an approach that already failed is tried again, at edit time and at
commit time. Lessons that hold beyond one project go into a global memory that
agents query before they use a library.

dejafix ships as native executables with no runtime dependencies. Everything
stays on your machine.

## Install

```bash
brew install bigfree/tap/dejafix          # macOS and Linux
pipx install --pip-args=--pre dejafix     # any platform with Python
```

Shell and PowerShell installers and archives for every platform are attached to
each [release](https://github.com/bigfree/dejafix/releases).

You get four executables:

- `djf`, also available as `dejafix`: the command line;
- `djf-mcp`: the MCP server;
- `djf-edit-hook`: the edit-time hook for Claude Code and Codex.

## Quick start

```bash
cd your-project
djf init
```

`djf init` does the following:

1. Creates `.dejafix/` and registers the project.
2. Installs the git hooks.
3. Writes a short bridge block into `CLAUDE.md` and `AGENTS.md`.
4. Prints the MCP configuration with the path of `djf-mcp` filled in.

Run it again after an upgrade to refresh the instructions.

## MCP server

Configure the server once; it serves every registered project.

Claude Code (`~/.claude.json`, or `claude mcp add`), Claude Desktop, Cursor and
other JSON clients:

```json
"mcpServers": {
  "dejafix": {
    "command": "/absolute/path/to/djf-mcp",
    "args": []
  }
}
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.dejafix]
command = "/absolute/path/to/djf-mcp"
args = []
```

With several projects registered, agents pass `project="<name>"` on any call.
You can also set a default with `djf project use <name>`. `djf doctor` finds
unregistered projects, stale entries and configs pinned to one repository.

### Tools

| Purpose | Tools |
|---|---|
| Session | `start_session` (rules, summary and plan in one call), `get_instructions`, `get_summary`, `get_project_map`, `get_plan` |
| Before editing | `precheck_file`, `precheck_files` |
| Lookup | `search_events`, `get_issue`, `get_context`, `get_score`, `list_projects`, `current_project` |
| Global memory | `global_query`, `get_global_gotchas`, `add_global_gotcha` |
| Writing | `log_issue`, `record_attempt`, `record_fix`, `add_decision` (with `supersedes` to retire an older decision), `add_note` |

`search_events` and `djf search` rank results by relevance (BM25F with an
English analyzer). Memory is kept in English. Agents read it in English and
answer you in your language.

## Global memory

Global memory holds library gotchas that apply in any project, for example how
a library behaves in a given version. It lives in `~/.dejafix/global/`.

- **Written on purpose.** An agent calls `add_global_gotcha` with a library, a
  few keywords and the lesson. `djf global add-gotcha` does the same from the
  command line.
- **Checked before it is stored.** dejafix refuses a lesson that:
  - is not in English;
  - has no package name as its library;
  - duplicates an existing lesson;
  - carries project details such as paths, ticket ids or internal hosts.

  Secrets are redacted.
- **Read by query.** `global_query` returns the best matching lessons within a
  token budget. The command-line equivalent is `djf global query`.
- **Maintained by hand.** `djf global list`, `edit`, `remove`, `prune`,
  `export` and `import` manage the store. `djf global upgrade` converts lessons
  written by older versions: it exports a draft, which an agent fills and you
  review before applying it.

## Edit hook (Claude Code and Codex)

`djf-edit-hook` shows a file's lessons right before the agent edits that file:
failed and partial attempts, open issues, and decisions that name it. Each
lesson is shown once per session and again after the context is compacted.

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

Codex (`~/.codex/hooks.json`): use the same command on `PreToolUse` with the
matcher `apply_patch`, and on `PostCompact`.

To log every hook call, set `DJF_EDIT_HOOK_LOG=/path/to/calls.jsonl`.

## CLI

| Area | Commands |
|---|---|
| Memory | `log`, `attempt`, `fix`, `decision`, `note`, `show`, `search`, `regenerate` |
| Judgment | `precheck`, `brief` |
| Projects | `init`, `project`, `doctor`, `hooks`, `migrate` |
| Global memory | `global` (`query`, `add-gotcha`, `list`, `edit`, `upgrade`, …) |
| Views | `instructions`, `map`, `plan` |

Run `djf <command> --help` for options.

## Privacy

Everything stays on your machine: there is no cloud service and no telemetry.

By default, the distilled files are committed so that your teammates' agents
inherit them: `summary.md`, `PROJECT_MAP.md`, `AI_INSTRUCTIONS.md` and
`issues/`. The raw log and the runtime files are gitignored. To keep all of it
private, add `.dejafix/` to `.gitignore`.

Secrets are redacted from recorded events. Set `DEJAFIX_NO_REDACT=1` to turn
that off. See [SECURITY.md](SECURITY.md).

## Upgrading from projectmem

To move an existing projectmem install, run `djf migrate` to see the changes,
then `djf migrate --apply` to make them. It moves the memory folders, the home
folder and the hooks. It also lists the client configs you need to update by
hand.

## License

MIT, see [LICENSE](LICENSE). dejafix started from
[projectmem](https://github.com/riponcm/projectmem) 0.3.5 by Ripon Chandra Malo.
