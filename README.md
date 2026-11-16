<!-- mcp-name: io.github.bigfree/dejafix -->
# dejafix

**Your coding agent forgets what already failed. dejafix does not.**

dejafix is local-first memory and judgment for AI coding agents. It records
issues, fix attempts, decisions and notes as typed events in plain text, and
puts the relevant ones in front of the agent at the moment they matter: when a
session starts, right before a file is edited, and before a commit. One MCP
server covers all your projects, and lessons about a library travel with you
from one project to the next.

It ships as native executables with no runtime dependencies, works with Claude
Code, Codex, Cursor, Claude Desktop and any other MCP client, and keeps
everything on your machine.

## Why

Agents start every session from zero. The dead end you spent an afternoon on
last month looks like a fresh idea to them today. Telling them to check first
does not work well either: on one real project, agents looked up a file's
history before only **11 % of 8,355 edits**, even though their instructions
asked for it every time
([measurements](https://github.com/riponcm/projectmem/issues/37)).

dejafix does not rely on the agent remembering to ask. The edit hook shows a
file's lessons right before the agent changes it, and stays silent when there
is nothing to say:

```
dejafix lessons for src/auth/refresh.ts:
  Open issues on this file:
    - #0042 token refresh races under parallel requests
  Earlier attempts (1 failed, 1 partial), newest first:
    ~ retry halves the failures (#0042, 2026-10-30)
    x wrapped refresh in a lock (#0042, 2026-09-28)
  Decisions on this file:
    - refresh tokens rotate per user, never shared (2026-09-12)
```

## Highlights

- **Lessons at edit time.** Failed and partial attempts, open issues and
  decisions for the file being edited, once per session, in Claude Code and
  Codex. A failed attempt is linked to its issue's file even when the agent
  did not name the file.
- **One call to start a session.** `start_session` returns the rules, the
  memory summary and the current plan together.
- **Linked projects.** A backend and the portal that calls its API, or two
  systems sharing a database, can declare the link. An agent editing one side
  of the boundary sees the relevant lessons from the other side.
- **Global library gotchas.** Lessons about a library, checked before they are
  stored and queried by relevance before the agent uses that library again.
- **Ranked search.** `search_events`, `djf search` and `global_query` rank by
  relevance (BM25F), so a question finds the lesson even when it is worded
  differently.
- **Built for teams and for privacy.** The distilled memory is plain Markdown
  you can commit, so teammates' agents inherit it. The raw log stays local,
  secrets are redacted, and there is no cloud service or telemetry.

## By the numbers

| | |
|---|---|
| Edit hook, whole process, 3,000-event log | 5.7 ms |
| Edit on a boundary, two linked logs of 3,000 events | 11.3 ms |
| `djf search`, whole process, 3,000 events | 12.3 ms |
| `precheck_file` on a warm MCP server | 0.2 ms |
| Earlier failures shown before an edit, on 51 real cases | 72.5 %, up from 33.3 % |
| Context added in one week of real use (879 edits) | about 2,600 tokens |

Timings are medians from the repository's benchmarks (`just bench`) on a
laptop; the edit hook of the earlier Python version took 47 ms. The recall and
context figures come from public measurements on real projects
([recall](https://github.com/riponcm/projectmem/issues/32),
[context](https://github.com/riponcm/projectmem/issues/37)).

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
2. Installs the git hooks. In a linked worktree they go to the main
   repository's hooks folder. When `core.hooksPath` is set, for example by
   husky, nothing is written; `djf init` prints the lines to add to your hook
   manager instead.
3. Writes a short bridge block into `CLAUDE.md` and `AGENTS.md`.
4. Prints the MCP configuration with the path of `djf-mcp` filled in.

Then configure the MCP server and the edit hook once (below), and start a
session. Run `djf init` again after an upgrade to refresh the instructions.

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

Memory is kept in English. Agents read it in English and answer you in your
language.

## Edit hook (Claude Code and Codex)

`djf-edit-hook` shows a file's lessons right before the agent edits that file:
failed and partial attempts, open issues, and decisions that name it. Each
lesson is shown once per session and again after the context is compacted. A
file without lessons costs nothing: the hook prints nothing.

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

Clients without hooks get the same lessons from `precheck_file`. To log every
hook call, set `DJF_EDIT_HOOK_LOG=/path/to/calls.jsonl`.

## Linked projects

Projects that depend on each other, for example a backend and the portal that
calls its API, or two systems sharing one database, can declare it in the
committed `.dejafix/config.toml`:

```toml
boundary = ["src/Api/**", "db/migrations/**", "openapi.yaml"]

[[links]]
project = "github.com/org/inventory"
relation = "shares the database; creates device ports this project reads"
```

- **`boundary`** lists the paths, as globs relative to the repository root,
  that other projects depend on. Start each glob with the folder it covers:
  the rest of an edited file's path is what linked lessons are matched
  against.
- **`[[links]]`** names a linked project by its `origin` remote without the
  scheme, user, port and `.git`, so the value is the same on every machine. A
  project without a remote is named by its folder. A registry id or alias
  works too. `relation` is optional and is shown to the agent.
- **One side is enough.** A link declared in one project is seen from both.
- **Local only.** A link resolves only to projects registered on this machine
  (`djf project register`). dejafix reads a linked project's local memory and
  never writes to it.

What the agent sees:

- **At session start**, `start_session` and `djf brief` list the linked
  projects with the name that reads their memory through the `project`
  argument of the MCP tools.
- **Before editing a file on its own boundary**, the edit hook and
  `precheck_file` show the linked projects' open issues, failed or partial
  attempts and decisions on their boundary that match the edited file, at most
  three per project:

  ```
  dejafix lessons from linked projects for src/api/backend/ports.ts:
    inventory ("shares the database; creates device ports this project reads"):
      - open #0042 port allocation races with the nightly sync [src/Api/Ports.php]
      x wrapped allocation in a lock (#0042, 2026-10-30)
      - decision: released ports are never reused (2026-09-12)
  ```

  An unrelated edit shows nothing.

`djf links` shows how this project's links resolve, and `djf doctor` names
links to projects this machine does not have.

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

## CLI

| Area | Commands |
|---|---|
| Memory | `log`, `attempt`, `fix`, `decision`, `note`, `show`, `search`, `regenerate` |
| Judgment | `precheck`, `brief` |
| Projects | `init`, `project`, `links`, `doctor`, `hooks`, `migrate` |
| Global memory | `global` (`query`, `add-gotcha`, `list`, `edit`, `upgrade`, …) |
| Views | `instructions`, `map`, `plan` |

Run `djf <command> --help` for options.

Each write rebuilds `summary.md` and the issue files it touched.
`djf regenerate` rebuilds all of them and removes issue files left behind by an
issue whose title changed. When `events.jsonl` has lines that cannot be read,
dejafix says how many and points to `djf doctor`.

## Privacy

Everything stays on your machine: there is no cloud service and no telemetry.

By default, the distilled files are committed so that your teammates' agents
inherit them: `summary.md`, `PROJECT_MAP.md`, `AI_INSTRUCTIONS.md` and
`issues/`. The raw log and the runtime files are gitignored. To keep all of it
private, add `.dejafix/` to `.gitignore`. To keep it private to your own clone
without touching the shared `.gitignore`, add `.dejafix/` (and the bridge files
`CLAUDE.md` and `AGENTS.md`, if you like) to `.git/info/exclude` or to your
global excludes file. `djf init` then leaves `.gitignore` alone.

Secrets are redacted from recorded events. Set `DEJAFIX_NO_REDACT=1` to turn
that off. See [SECURITY.md](SECURITY.md).

## Upgrading from projectmem

To move an existing projectmem install, run `djf migrate` to see the changes,
then `djf migrate --apply` to make them. It moves the memory folders, the home
folder and the hooks. It also lists the client configs you need to update by
hand, including a project's own `.mcp.json`, `.cursor/mcp.json`,
`.claude/settings*.json` and `.codex/config.toml`.

## License

MIT, see [LICENSE](LICENSE). dejafix started from
[projectmem](https://github.com/riponcm/projectmem) 0.3.5 by Ripon Chandra Malo.
