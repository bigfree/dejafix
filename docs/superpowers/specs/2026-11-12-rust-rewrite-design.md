# dejafix in Rust — design

Date: 2026-11-12. Status: approved design, implementation not started.

## Goal

Rewrite the core of dejafix in Rust so it ships as one native binary per
platform, with no Python interpreter, no virtualenv and no PATH lookup for
`djf` inside git hooks. The second goal is long-term maintainability: a type
system that makes invalid events unrepresentable and a codebase with enforced
layering.

Speed is a side effect, not the reason. Measured on a copy of the largest real
project (3,074 events, 2.2 MB `events.jsonl`), the Python version spends 47 ms
per edit-hook call, 170 ms per pre-commit precheck and 58 ms per warm MCP
`precheck_file`. An agent turn takes seconds, so these savings are not what
users will notice.

## Decisions

| Topic | Decision |
|---|---|
| Scope of 1.0 | The core: MCP server (19 tools), `init`, git hooks, edit hook, `summary.md` generation, project registry, `migrate`, and the manual memory commands |
| Platforms | macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64) from the first release |
| Data format | Unchanged. Python and Rust versions read and write the same files and can be used on the same project in turn |
| Transition | Parallel development in this repository under `rust/`; Python stays the released version until Rust 1.0 |
| Distribution | GitHub Releases with shell and PowerShell installers, a Homebrew tap and a PyPI wheel, all built on a tag |
| Architecture | One library crate and one binary crate in a Cargo workspace |

Out of scope for 1.0 and removed from the CLI: `visualize`, `dashboard`,
`watch`, `wrap`, `backfill`, `score`, `stats`, `context`, `export`. The logic
behind `score` and `context` stays in the core because the MCP tools
`get_score` and `get_context` use it. Users who need the removed commands stay
on Python 0.4.x. Visualisation (today `visualize` and `dashboard`, including the
local HTTP server) gets its own design after 1.0 and is not ported from Python.

## Workspace layout

```
rust/
  Cargo.toml              workspace: edition 2024, resolver 3, [workspace.lints]
  rust-toolchain.toml     pinned stable toolchain, clippy, rustfmt, llvm-tools
  justfile
  crates/
    dejafix-core/src/
      lib.rs              the only public API
      core/               pure logic, no I/O
      adapters/fs/        events.jsonl, registry, atomic writes, stack detection
      adapters/git/       hooks directory, staged files, commit metadata
      wiring/             composes core and adapters
      testing/            fakes, used only by tests
    dejafix/src/
      main.rs             clap dispatch
      config/             environment variables, read once
      cli/
      mcp/
      hook/
      bin/djf-mcp.rs, bin/djf-edit-hook.rs
```

The binary crate builds four executables: `djf` and `dejafix` from the same
`main.rs`, plus `djf-mcp` and `djf-edit-hook`, so existing client and hook
configurations keep working unchanged.

## Code rules

These follow the conventions of the author's other repositories (home-log-2,
cra-mcp, excalibur, onnet-nearnet), translated to Rust.

- No comments and no `///` doc comments. The only exceptions are
  `TODO(#issue)` and tool directives such as `#[expect(lint, reason = "...")]`.
  Names, types and structure carry the meaning.
- Types live in the module's `types.rs`; behaviour lives in sibling files.
- `[workspace.lints]` enforces: `unsafe_code = "forbid"`; clippy `pedantic`;
  `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`,
  `indexing_slicing` and `as_conversions` denied outside tests. Lint
  exceptions use `#[expect]`, never `#[allow]`.
- Errors are typed `thiserror` enums per domain. No `anyhow`.
- `core` never imports `adapters` or `wiring`. What it needs from I/O it
  declares as a trait.
- At most 12 source files per directory. Unit tests sit next to the code in
  `tests.rs`; integration tests live in the crate's `tests/`.
- Performance is part of correctness: no allocation, copy or re-parse inside a
  loop that can happen once outside it.
- Current Rust only, for new code and for every edit: let-chains,
  `LazyLock`/`OnceLock`, async closures, `async fn` in traits instead of the
  `async-trait` crate, iterator adapters instead of intermediate vectors,
  `std::env::home_dir`, `File::lock`.
- Before using any API of Rust or a dependency, its documentation is checked
  through Context7 at the version pinned in the workspace, not recalled.
- Dependencies: the latest stable release, pinned exactly (`=x.y.z`), only
  widely used and maintained crates, and the standard library first. The
  planned set is clap, serde, serde_json, thiserror, rmcp (features `server`,
  `transport-io`), tokio (only in `mcp/`), jiff, regex, tempfile, schemars;
  dev-only: proptest, insta, assert_cmd, criterion.
- Configuration: only `config/` reads `DEJAFIX_HOME`, `DEJAFIX_ROOT`,
  `DEJAFIX_NO_REDACT` and `DJF_EDIT_HOOK_LOG`. The library receives values as
  arguments.
- Commits: `type(scope): imperative lowercase subject`, a body that explains
  why, no assistant attribution of any kind. Work goes on a branch and lands
  through a pull request.
- Code, commit messages and documentation are in English.

## Improvement log

An agent that sees a way to do something better than the current
implementation (the Python behaviour, this spec, or Rust code from an earlier
milestone) records it in `docs/superpowers/improvements.md` and carries on with
the milestone as specified. It does not implement the idea unasked: the
compatibility contract and the milestone scope stay as they are until the user
reviews the log and decides.

Each entry is appended at the end of the file under its own heading, using the
template at the top of the file. Separate headings keep merges between
milestone branches trivial.

## Development tooling

Development runs locally. Nx is not used: Cargo already provides the
dependency graph, incremental builds and caching, and a remote cache has no
use on one machine.

- `just` runs the tasks: `verify` (fmt, clippy, nextest, coverage, golden),
  `golden`, `cov`, `bench`, `smoke`.
- `cargo nextest` runs the tests, `bacon` re-runs check, clippy or tests on
  save, `sccache` shares compiled dependencies across branches and git
  worktrees.
- The `dev` profile optimises dependencies (`opt-level = 2`) and keeps the
  workspace crates unoptimised with line-table debug info.

## Domain model

```
Event { id: EventId, at: Timestamp, kind: EventKind, origin: Origin, extra: BTreeMap }
EventKind = Issue | Hypothesis | Attempt { outcome } | Fix | Decision { supersedes } | Note
```

- Newtypes: `EventId`, `IssueId` (normalises `1`, `#1` and `0001`),
  `RelativePath`, `Outcome`, `Timestamp` (jiff).
- A serde adapter writes the same flat JSON as Python, with keys sorted like
  `json.dumps(sort_keys=True)`. Unknown fields are kept in `extra`, so neither
  version destroys what the other wrote.
- `EventLog` is built in one pass over `events.jsonl`, with zero-copy
  deserialisation (`Cow<str>`). It holds the events and indexes for open and
  closed issues, retired decisions, and file → events (from `location`,
  `files`, and the file an attempt inherits from its issue).
- Every output is a pure function over `EventLog`: `summary.md` and
  `issues/*.md`, the precheck report, a file's lessons for the edit hook,
  search, context, score, the issue view and `start_session`.

## Data flow

Write path:

1. A command (`log`, `attempt`, `fix`, `decision`, `note`) is validated and
   redacted.
2. The event is appended to `events.jsonl` as one line, under `File::lock`, so
   the MCP server, git hooks and the CLI can write concurrently.
3. The event is applied to the loaded `EventLog` without re-parsing.
4. `summary.md` and the issue file are rewritten atomically, the current-issue
   marker is updated, and the lesson is promoted to global memory.

Per entry point:

- MCP server (long-lived): a cache of `root → (size, mtime, EventLog)`
  validated by one `stat` per call; another writer's change triggers a re-read.
  `stdio_pipes()` is used when both standard streams are pipes, otherwise
  `stdio()`.
- Edit hook (short-lived): reads the log and computes lessons only for the
  edited files. Session markers stay in the temp directory with today's
  guarantees: mode 0700, owner check, `O_EXCL | O_NOFOLLOW`.
- CLI and git hooks: the same composition from `wiring`, without the cache.

Global memory (`~/.dejafix/global`) is part of the core because
`get_global_gotchas` and lesson promotion need it. Stack detection
(`package.json`, `Cargo.toml`, `composer.json` and similar) is a filesystem
adapter.

## Errors

| Error | Example |
|---|---|
| `ParseError` | line 812 of `events.jsonl` is not valid JSON |
| `StoreError` | writing `.dejafix/summary.md` failed (path and I/O source) |
| `ValidationError` | empty summary, unknown issue, several open issues without `issue_id` |
| `ResolveError` | project not found or ambiguous, with the registered names |
| `RegistryError` | `projects.json` is corrupt |

- The edit hook never blocks an edit: any error exits 0 with no output and is
  written only to the optional `DJF_EDIT_HOOK_LOG`.
- Git hooks never stop a commit: auto-capture warns on stderr, precheck warns
  without blocking.
- MCP returns errors as tool results with `isError: true` and a next step, such
  as the list of open issues. Typed arguments with `schemars` schemas reject bad
  input; the server does not exit on it.
- The CLI prints to stderr and exits 1 for a user error, 2 for bad arguments
  (clap) and 3 for I/O.
- A corrupt log line is skipped and counted, and `djf doctor` reports it. The
  log is never rewritten.
- The release profile uses `panic = "abort"`; the lints keep panics out of the
  code.

## Compatibility contract

For the same input the Rust version produces byte-identical output to Python
0.4.x, verified by the golden tests:

- files: `events.jsonl`, `summary.md`, `issues/*.md`, `PROJECT_MAP.md`,
  `plan.md`, `AI_INSTRUCTIONS.md` (template version 3), `config.toml`,
  `~/.dejafix/projects.json` with its sidecar, and the global memory files;
- MCP: the same 19 tools, parameters, reply texts and server `instructions`;
- CLI: the same command names and flags within the 1.0 scope, and the same
  environment variables;
- git hooks: the same block markers, with the path pointing at the Rust binary.

Deliberate differences:

- writes take a file lock;
- `djf init` no longer backfills from git history, starts the watcher or builds
  the `structure.json` cache, since those features are out of scope;
- `djf map` only prints the project map; `--build` is gone with the cache.

## Testing

1. Unit tests in `core`, including property tests (`proptest`): any log line
   never panics; read and write preserve every field, unknown ones included;
   `IssueId` and path normalisation (Unix, Windows, `./`, `file:line`) are
   stable. Rendered texts (`summary.md`, MCP replies, edit-hook output) are
   snapshot tests (`insta`).
2. Integration tests with real processes in temporary directories: the CLI
   through `assert_cmd` in a temporary git repository; MCP through an rmcp
   client connected to the server over an in-process pipe, calling
   `initialize`, `list_tools` and all 19 tools; the edit hook with Claude Code
   (`Edit`, `Write`) and Codex (`apply_patch`) payloads, including repeats and
   the reset after compaction; git hooks through a real `git commit`; and
   every scenario of today's `test_migrate.py`.
3. Golden compatibility tests (`just golden`): the Python implementation from
   `src/` and the Rust binary run the same scenario (init, log, attempt, fix,
   superseding decision, precheck, edit-hook payloads, MCP calls) on copies of
   one fixture, and their outputs and resulting files are compared. Fixtures
   are synthetic, from a generator, at 100, 3,000 and 30,000 events. Random ids
   and timestamps are normalised to placeholders in order of appearance before
   the comparison, so production code has no test seams.
4. Porting the Python tests: each of the 504 Python tests gets a Rust
   counterpart or a stated reason it has none. M0 creates the mapping as
   `rust/PORTING.md`, one row per Python test, and each milestone ticks off its
   rows.
5. Coverage: `just cov` (`cargo llvm-cov`) fails below 90 % line coverage in
   `dejafix-core` and 80 % in the binary crate.
6. Performance: `just bench` (`criterion`) on the 3,000- and 30,000-event
   fixtures. These budgets are acceptance criteria at 3,000 events:

   | Measurement | Python today | Rust budget |
   |---|---|---|
   | edit hook, whole process | 47 ms | ≤ 10 ms |
   | MCP `precheck_file`, warm cache | 58 ms | ≤ 5 ms |
   | `djf precheck`, whole process | 170 ms | ≤ 20 ms |
   | `summary.md` regeneration | 530 ms | ≤ 30 ms |

7. Platforms: `just verify` runs locally on macOS. The release workflow runs
   `cargo nextest` on macOS, Linux and Windows before building anything and
   stops on a failure; it can also be started by hand to check Windows early.
8. Live check before every release (`just smoke`): install the built binary
   into a temporary directory, run `djf init` in a temporary repository, commit
   and check that the hooks recorded events, then start `djf mcp` and call it
   from a scripted client. A green test alone does not count as verification.

## Release

- `dist` (formerly cargo-dist) builds for `aarch64-apple-darwin`,
  `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`, with the `shell`,
  `powershell` and `homebrew` installers. The tap is `bigfree/homebrew-tap`.
  Every archive carries `djf`, `dejafix`, `djf-mcp` and `djf-edit-hook`.
- `maturin` with `bindings = "bin"` wraps the same binary in PyPI wheels and
  publishes through trusted publishing (OIDC), with no API token stored in
  GitHub.
- The release workflow runs only on a `v*` tag: tests on three systems, then
  `dist` (binaries, installers, GitHub Release, tap formula), then `maturin`
  (wheels to PyPI).
- Versions: Python continues as 0.4.x; Rust starts at `1.0.0-rc.N`. Both are
  the PyPI package `dejafix`, so `pipx upgrade dejafix` moves from the Python
  version to the Rust binary.
- The PyPI name `dejafix` is still unclaimed. Publishing Python 0.4.0 soon
  reserves it; until then `doctor` could point users at someone else's
  package.

## Milestones

Each milestone ends with a working, tested increment on its own branch and
pull request. Python remains the released version until M6.

| | Content | Done when |
|---|---|---|
| M0 | Install rustup (with the user's approval), workspace, lints, `justfile`, nextest, bacon, sccache, coverage gate, fixture generator, golden harness skeleton, `rust/PORTING.md` | `just verify` passes on the empty workspace |
| M1 | Event model, serde compatibility, `EventLog` and indexes, path and id normalisation | reading and writing every fixture is byte-identical to Python |
| M2 | Write path with lock, redaction, `summary.md`, issues, marker, global memory, instructions template, registry; CLI `init`, `log`, `attempt`, `fix`, `decision`, `note`, `show`, `search`, `regenerate`, `project`, `doctor` (offline), `instructions`, `plan`, `map` | the CLI golden scenario matches and the `summary.md` budget is met |
| M3 | Git hooks (install, uninstall, auto-capture, precheck) and the edit hook | the edit-hook and precheck budgets are met and a real `git commit` records events |
| M4 | MCP server: 19 tools, `instructions`, cache; CLI `brief` and `global` | MCP golden replies match and the `precheck_file` budget is met |
| M5 | `migrate`, `doctor --online` and `--auto`, `dist` and `maturin` configuration, release workflow, tap | `1.0.0-rc.1` is released and the author's projects and clients run on it |
| M6 | At least two weeks on `rc` in the author's projects, then remove the Python `src/` and `tests/`, update the documentation and CHANGELOG | `1.0.0` is released |

Changes approved by the user on 2026-11-12 (M2 plan, "Changes to the milestone
table"): `brief` moved from M2 to M4, because it needs staleness (M3) and the
score (M4); `doctor --online` and `--auto` moved to M5 with the HTTPS client and
the release channel, so M2 ports the offline `doctor`; `regenerate` joined M2;
`djf global` went to M4, next to `get_global_gotchas`; `init` keeps
`--no-watch`, `--no-backfill` and `--no-structure` as hidden flags that do
nothing.

Estimate: M0–M4 about 4–6 weeks, M5–M6 about 1–2 weeks plus the `rc` period;
6–10 weeks in total, toward the lower end because of the narrower scope.
