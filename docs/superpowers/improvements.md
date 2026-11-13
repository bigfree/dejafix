# Improvement log

Ideas found during the Rust rewrite that would do something better than the
current implementation. Nothing here is approved: each entry waits for the
user's review and is then accepted (and scheduled), or rejected with a reason.
See "Improvement log" in `specs/2026-11-12-rust-rewrite-design.md`.

Append new entries at the end, one heading each, with this template:

```markdown
## <short title>

- Found: <YYYY-MM-DD>, <milestone>, <file or area>
- Today: <what the current implementation does>
- Proposal: <what to do instead>
- Gain: <what improves, measured where possible>
- Compatibility: <none | breaks the on-disk format, MCP replies or CLI, and how>
- Status: proposed
```

<!-- Entries start below this line. -->

## Close an unterminated last line before appending an event

- Found: 2026-11-12, M2a, `rust/crates/dejafix-core/src/adapters/fs/events.rs`
- Today: Python appends `json.dumps(event) + "\n"` with `open("a")`. If `events.jsonl` ends without a newline (a hand edit), the new event is glued to the last line, both become one unparsable line, and Python then fails on every read.
- Proposal: write a newline first when the file does not end with one. Rust M2a already does this; it came out of the second code-review round, before this log existed.
- Gain: no event is lost to a missing final newline; Rust reads both lines, Python 0.4.x no longer crashes on them.
- Compatibility: on such a file the bytes differ from Python's (two valid lines instead of one corrupt line). No other case changes.
- Status: implemented in M2a, awaiting review (revert to Python's behaviour if rejected)

## Rewrite only the issue file a write touched

- Found: 2026-11-12, M2a, `rust/crates/dejafix-core/src/wiring/project.rs`
- Today: every write regenerates all `issues/*.md` (Python rewrites them all; Rust compares each with the file on disk and rewrites only the changed ones, on up to 8 threads). On the 3,000-event fixture that check is about 20 of the 25 ms a write spends regenerating.
- Proposal: after `log`, `attempt`, `fix`, `decision` or `note`, write only the issue file of the issue the new events belong to, plus any issue file that is missing; keep the full check for `djf regenerate`.
- Gain: about 20 ms less per write at 3,000 events and no reads of hundreds of unrelated files; the summary budget gets headroom.
- Compatibility: a hand-edited or stale issue file of another issue is no longer repaired by the next write, only by `djf regenerate` (Python repairs it on any write).
- Status: proposed

## Ignore the atomic-write temp files

- Found: 2026-11-12, M2a, `rust/crates/dejafix-core/src/adapters/fs/write.rs`
- Today: atomic rewrites of `summary.md`, issue files and the marker go through a `.dejafix/.djf-*` temporary file. A process killed mid-write leaves it behind, and `djf init`'s `.gitignore` entries do not cover it.
- Proposal: add `.dejafix/.djf-*` to the entries `djf init` writes, or remove stale `.djf-*` files at the start of a write.
- Gain: no stray copy of `summary.md` committed after an interrupted write.
- Compatibility: an extra `.gitignore` line differs from what Python 0.4.x writes.
- Status: proposed

## Tell the user when the log has unreadable lines

- Found: 2026-11-12, M2a, `rust/crates/dejafix/src/cli/`
- Today: Python stops with a traceback on the first corrupt line of `events.jsonl`; Rust skips and counts such lines, and only `djf doctor` (M2c) will report them. Write and read commands stay silent.
- Proposal: print one stderr line such as `dejafix: 2 line(s) of events.jsonl could not be read; run djf doctor` when a command's log has skipped lines.
- Gain: a damaged log is noticed when it happens, not at the next `doctor` run.
- Compatibility: extra stderr output on damaged logs only.
- Status: proposed

## Never write issue files through a symlinked issues directory

- Found: 2026-11-12, M2a security review, `rust/crates/dejafix-core/src/wiring/project.rs`
- Today: Python's `write_issue_files` writes `.dejafix/issues/{id}-{slug}.md` with `write_text`, which follows a symlinked `issues/` directory. A cloned repository that ships `.dejafix/issues -> <somewhere outside>` plus a crafted `events.jsonl` makes the next `djf note` create or overwrite an attacker-chosen `*-*.md` file outside the repository, for example an agent command file.
- Proposal: skip writing issue files when `.dejafix/issues` is a symlink. Rust M2a already does this; the security review found it before this log existed. The same guard belongs in Python's `write_issue_files`.
- Gain: no write outside the repository from repository content.
- Compatibility: a project whose `issues/` is intentionally a symlink gets no issue files from Rust; `summary.md` and the log are unaffected.
- Status: implemented in M2a, awaiting review

## Keep unknown keys of the registry sidecar when saving

- Found: 2026-11-12, M2b, `rust/crates/dejafix-core/src/core/registry/load.rs`
- Today: `save_registry` (Python) and `meta_payload` (Rust) rebuild `projects.meta.json` from `schema_version`, `active_project` and `by_path` only. Every registration therefore drops `last_version`, and the next command re-records it; any key a newer release adds to the sidecar is lost the same way.
- Proposal: load the sidecar as a map, replace only the three keys the registry owns, and write the rest back in place.
- Gain: the upgrade stamp survives `djf init` and `djf project ...`, so the notice logic no longer depends on a silent re-record; a newer release's sidecar keys survive an older one.
- Compatibility: `projects.meta.json` keeps extra keys where Python 0.4.x drops them; readers ignore unknown keys, so nothing breaks.
- Status: proposed

## Never copy a symlink's outside target into a project file

- Found: 2026-11-12, M2b security review, `rust/crates/dejafix-core/src/wiring/init.rs`, `wiring/files.rs`
- Today: Python's `djf init` and `djf plan` read `CLAUDE.md`, `AGENTS.md`, `.gitignore`, `.dejafix/AI_INSTRUCTIONS.md` and `.dejafix/plan.md` through a symlink and write the bridge, gitignore lines, global-memory block or idea back through it, so a cloned repository can make dejafix append to an arbitrary file of the user's (for example `CLAUDE.md -> ~/.aws/credentials`). The Rust write path replaces symlinks instead of following them, which on its own would copy the secret into a regular, committable file.
- Proposal: read such a file only when it is not a symlink or its canonical target lies inside the project root; otherwise treat it as absent, so the link is replaced by a fresh file and its target is neither read nor touched. Rust M2b already does this. The same guard belongs in Python's `_ensure_bridge_file`, `ensure_gitignore_entry`, `refresh_ai_instructions`, `_inherit_global_memory` and `plan.run`.
- Gain: a hostile repository can neither exfiltrate a local file into a commit nor modify a file outside the repository.
- Compatibility: a rule file intentionally linked outside the repository loses its link on `djf init` (it becomes a regular file with the bridge); links inside the repository (`CLAUDE.md -> AGENTS.md`) are read as before.
- Status: implemented in M2b, awaiting review

## Compare scanned projects with the registry by identity, not by text

- Found: 2026-11-12, M2c, `rust/crates/dejafix-core/src/wiring/doctor.rs` (`scan_projects`)
- Today: `djf doctor` decides whether a found project is registered by `(device, inode)`, but `djf project scan` (Python `scan_command`, ported as is) compares path text. On a case-insensitive macOS volume, `djf project scan ~/Code` after `~/code/api` was registered lists `~/Code/api` as new, and registering it adds a second record `api-2` for the same folder, the very duplicate `dedupe_paths` was written to prevent.
- Proposal: mark a scanned project as known when its identity matches a registered path's identity, as `doctor` does, and register nothing for it.
- Gain: no duplicate registry entries when the same folder is reached under two spellings or through a symlink.
- Compatibility: `project scan` output shows such a folder as "(already registered)" where Python 0.4.x shows it as new; the registry no longer gains the duplicate.
- Status: proposed

## Save the registry once per `doctor --fix` section

- Found: 2026-11-12, M2c, `rust/crates/dejafix-core/src/wiring/doctor.rs`
- Today: Python's `doctor --fix` and `project scan` call `register` and `unregister` once per project, and each call loads and rewrites both registry files. Rust applies every registration in memory and saves once, then every removal and saves once more; the files end up identical.
- Proposal: keep the Rust behaviour, and when a save fails, report it per project instead of stopping with a traceback (Rust already does this and keeps the registry unchanged).
- Gain: two writes instead of up to two per project; an unwritable home gives a readable report.
- Compatibility: none for the files; on an unwritable home Rust prints `✗` lines where Python prints a traceback.
- Status: implemented in M2c, awaiting review

## Escape control characters in paths printed to the terminal

- Found: 2026-11-12, M2c security review (confidence 6 of 10, below the reporting threshold), `rust/crates/dejafix/src/cli/doctor.rs`, `cli/project.rs`
- Today: `djf doctor` and `djf project scan` print the paths of every project they find, and `djf project list` the registered ones, as they are (Python's `click.echo` does the same). A cloned repository can contain a folder whose name carries terminal escape sequences, for example OSC 52, which some terminals use to replace the clipboard, or cursor movements that overwrite doctor's warnings. Plain `djf doctor` walks `~/code` and similar folders, so it prints such names without the user having chosen them.
- Proposal: one helper in `cli/output.rs` that escapes C0, DEL and C1 control characters in file-system names before printing, used by `doctor`, `project scan` and `project list`.
- Gain: a hostile folder name can no longer drive the terminal.
- Compatibility: output differs from Python 0.4.x only for paths that contain control characters.
- Status: proposed

## Let the golden harness find the main checkout's virtualenv

- Found: 2026-11-12, M3, `rust/crates/dejafix/tests/golden/harness.rs`
- Today: without `DEJAFIX_PYTHON`, the harness looks for `.venv` in the repository it runs from. A git worktree has no `.venv`, so it falls back to the system `python3`, which lacks `typer`, and every golden test fails with an import error far from the cause.
- Proposal: when the worktree has no `.venv`, look in the main checkout (`git rev-parse --git-common-dir`, then its parent) before falling back; if no interpreter with `typer` is found, fail once with a message that names `DEJAFIX_PYTHON`.
- Gain: `just golden` and `just verify` work in every worktree without setup; a missing oracle is reported as such.
- Compatibility: none (test tooling only).
- Status: proposed

## Find the hooks directory through git

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/adapters/git/hooks.rs`
- Today: `djf hooks` and `djf init` use `<cwd>/.git/hooks`, as Python does. In a linked worktree `.git` is a file, and with `core.hooksPath` set (husky, lefthook, a shared hooks folder) git never runs what dejafix installs; both cases print "not a git repository" or install hooks that never fire.
- Proposal: ask git (`git rev-parse --git-path hooks`), which honours worktrees and `core.hooksPath`, and print where the hooks went.
- Gain: auto-capture and precheck work in worktrees and in repositories with a hooks manager.
- Compatibility: output changes in those two cases only (Python reports an error or installs into an unused folder).
- Status: proposed

## Bake a stable path for the hook binary

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/adapters/git/hooks.rs`
- Today: the hooks bake the `djf` next to `std::env::current_exe()`. On Linux that is the resolved path, so a Homebrew install bakes `…/Cellar/dejafix/<version>/bin/djf`, which disappears on the next upgrade; the hook then depends on its `command -v djf` fallback and the PATH git happens to have.
- Proposal: when a `djf` on `PATH` resolves to the same file as the running executable, bake the `PATH` spelling (for example `/home/linuxbrew/.linuxbrew/bin/djf`), else the executable's path.
- Gain: hooks keep working across package-manager upgrades without `djf init` again.
- Compatibility: none for the hook format; only the baked path differs.
- Status: proposed

## One git query per file for churn and staleness

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/wiring/hooks/precheck.rs`
- Today: precheck asks git twice per checked file (`git log --since=30.days.ago --oneline` for churn and `git log --format=%cI --since=<oldest claim>` for staleness), after `git diff --cached` has listed the staged files. On the measuring machine one git process costs 6–9 ms even for `git --version`, so a pre-commit run is at least two serial git start-ups (about 20 ms) before any work; measured whole-process median 29 ms against the 20 ms budget.
- Proposal: one `git log --format=%cI --since=<min(30 days ago, oldest claim)> -- FILE` per file, counting churn from the same times (tried in M3: same output on every golden scenario, fewer processes), and accept that git's own `--since` walk can differ from an in-memory cut-off on clock-skewed histories. Separately, decide whether the budget measures the work dejafix does (about 9 ms with process start) or includes git's start-up cost.
- Gain: half the git processes per precheck.
- Compatibility: HIGH CHURN counts can differ from Python on histories whose committer dates go backwards.
- Status: approved by the user on 2026-11-12 on condition of a clear speed-up, implemented and measured (A/B, same load: 25.6 ms against 26.3 ms median for a staged precheck, about 3 %), and therefore not kept. The critical path is two serial git start-ups; only removing one of them (for example listing staged files without a `git diff` process) can reach 20 ms on this machine.

## Pass log paths to git as literal pathspecs

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/adapters/git/history.rs`
- Today: staleness and churn pass a path taken from `events.jsonl` to `git log -- PATH`, as Python does, so a logged path such as `src/*.py` or `:(top)x` is read as a glob or pathspec magic and counts commits of other files.
- Proposal: add `--literal-pathspecs` (or `:(literal)`) to those queries.
- Gain: counts always refer to the one file the event cites; a hostile log cannot widen the query.
- Compatibility: counts change only for logged paths that contain glob or magic characters.
- Status: proposed

## Print the redacted summary after an auto-capture

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/wiring/hooks/capture.rs`
- Today: like Python, `djf _auto-capture` stores the redacted event but prints `[dejafix] Auto-captured: …` with the summary as it was before redaction, so a secret in a commit message reaches the terminal (the hooks send it to /dev/null; a manual run does not).
- Proposal: print the summary that was written.
- Gain: a secret never appears in dejafix's own output.
- Compatibility: stdout differs from Python only when a secret was redacted.
- Status: proposed

## List staged files without a git process

- Found: 2026-11-12, M3, `rust/crates/dejafix-core/src/wiring/hooks/precheck.rs`
- Today: a staged precheck starts `git diff --cached` to list the staged files and only then starts the history queries, so two git start-ups run in series (about 9.5 ms for the first alone); the whole process takes about 26 ms median against the 30 ms budget.
- Proposal: read the staged paths from `.git/index` in-process (the `gix` crate, or a small index parser covering index versions 2–4 and split index), compare with `HEAD`'s tree, and start the history queries at once.
- Gain: about 9 ms less per pre-commit run, which would bring precheck near the original 20 ms target.
- Compatibility: none in output; a new dependency or an index parser to maintain, and edge cases (sparse checkout, split index, `core.fsmonitor`) to test against git.
- Status: proposed

## Remember git answers per repository state in the MCP server

- Found: 2026-11-13, M4, `wiring/mcp`, `wiring/hooks/precheck.rs`
- Today: `precheck_file` on a file that has events runs `git log --since=30.days.ago --oneline -- FILE` (churn) and `git log --format=%cI -- FILE` (staleness) on every call, as Python does. On the 3,000-event fixture the warm call takes 12.5 ms, of which one `git log` is 10.9 ms; a file without events takes 0.1 ms and a cold call 3 ms. The 5 ms budget cannot be met for cited files while git runs on every call.
- Proposal: keep the two answers per `(root, file)` in the MCP state, keyed by the repository state read without a process (the `HEAD` file, the ref it names or `packed-refs`, through the `.git` file of a worktree), and give the churn answer a short lifetime (for example 60 s), because `--since=30.days.ago` moves with the clock.
- Gain: a cited file drops from about 12.5 ms to about 1.5 ms per warm call.
- Compatibility: changes MCP replies only near the 30-day boundary, for at most the memo's lifetime; needs a line in the compatibility contract.
- Status: accepted by the user on 2026-11-13 and implemented in M4 (`wiring/mcp/memo.rs`, `adapters/git/state.rs`); recorded in the spec's compatibility contract

## Keep the first line of `git status` intact in `get_context`

- Found: 2026-11-13, M4, `adapters/git/status.rs`
- Today: Python strips the whole `git status --porcelain` output before splitting it, so when the first entry is modified only in the working tree (` M a.py`) its leading space is lost and `line[3:]` drops the first letter of the path (`racked.txt`). Rust reproduces this, so that file never gets the git-status boost.
- Proposal: split first, then take `line[3:]` of each unstripped line.
- Gain: the file being edited right now ranks as Python intended.
- Compatibility: changes `get_context` output when the first `git status` line starts with a space.
- Status: proposed

## Refuse memory files that are symlinks out of the project in MCP reads

- Found: 2026-11-13, M4, `wiring/mcp/tools.rs`
- Today: `get_instructions`, `get_summary`, `get_project_map`, `get_plan`, `start_session` and `get_issue` read `.dejafix/*` through symlinks, as Python does, so a symlink planted in a cloned repository can hand any file the user can read to the agent.
- Proposal: read these files with `read_text_within(path, root)` (M2), as `djf plan` already does, and reply with the "not found" text when the target leaves the project.
- Gain: a hostile repository cannot leak files through the memory tools.
- Compatibility: changes replies only for symlinks that leave the project.
- Status: proposed

## Bound `precheck_files`

- Found: 2026-11-13, M4, `wiring/mcp`
- Today: `precheck_files` accepts any number of paths, as Python does; each path with events costs one or two git processes (at most 8 at a time).
- Proposal: refuse lists beyond a fixed length (for example 200 paths) with a reply that says so.
- Gain: one tool call cannot start thousands of git processes or produce a reply of megabytes.
- Compatibility: changes the reply for very long lists only.
- Status: proposed

## Refuse network paths in the MCP `project` argument

- Found: 2026-11-13, M4 security review, `core/resolve/decide.rs`, `wiring/mcp/probe.rs`
- Today: the `project` argument reaches `fs::canonicalize` and `is_dir` (also in pinned mode, through the registry lookup, before the mismatch is refused), as Python's `Path(...).resolve()` does. On Windows a UNC value such as `\\host\share` makes the server open an SMB connection and send the user's NetNTLM hash; a prompt-injected agent call can trigger it.
- Proposal: reject a `project` value that `is_network_path` (M2, already used by `FsResolver`) matches at the start of `resolve`, in every branch.
- Gain: no credential leak through a crafted tool argument on Windows.
- Compatibility: a UNC `project` value is refused with the "No project" error.
- Status: proposed

## Run git with repository-config hardening from the MCP server

- Found: 2026-11-13, M4 security review, `adapters/git/run.rs`
- Today: `get_context` runs `git status` and precheck runs `git log` in the project the `project` argument names, with the repository's own config, as Python does. A planted repository whose `.dejafix/` sits in a bare repository with `core.fsmonitor` set could run that command through `git status`.
- Proposal: pass `-c core.fsmonitor=false` (and consider `-c safe.bareRepository=explicit`) on every git call the server makes.
- Gain: a tool argument cannot reach a command configured by a cloned repository.
- Compatibility: none in output.
- Status: proposed

## Keep line endings when migrate rewrites a memory file

- Found: 2026-11-13, M5, `rust/crates/dejafix-core/src/wiring/setup/migrate.rs`
- Today: `djf migrate` reads each file it rewrites with universal newlines and writes `\n`, as Python's `_read`/`_write_atomic` do. A CRLF `events.jsonl` (Windows text mode) comes out as LF.
- Proposal: replace `.projectmem` in the raw bytes and leave line endings alone.
- Gain: a Windows project keeps the endings its other files and its git attributes expect; the diff of the migration commit shows only the renamed paths.
- Compatibility: on CRLF files the bytes differ from Python's (CRLF kept instead of LF). LF files are unchanged.
- Status: proposed

## Compare pre-releases correctly in the update check

- Found: 2026-11-13, M5, `rust/crates/dejafix-core/src/core/doctor/version.rs`
- Today: `doctor --online` compares only the leading digits of the first three version chunks, as Python's `_as_tuple` does, so `1.0.0rc1` equals `1.0.0`. Someone on `1.0.0-rc.1` is told "the latest" once `1.0.0` is out.
- Proposal: compare PEP 440 versions (release, then pre-release `aN < bN < rcN < final`), accepting Cargo's `1.0.0-rc.1` spelling for the running version.
- Gain: rc users learn that the final release exists.
- Compatibility: `doctor --online` output differs from Python only when one side is a pre-release.
- Status: proposed

## Verify the dist installer the release workflow downloads

- Found: 2026-11-13, M5, `.github/workflows/release.yml`
- Today: the `plan` job runs `curl … cargo-dist-installer.sh | sh` for a pinned dist version over TLS from GitHub, as `dist generate` writes it. Every other action is pinned to a commit SHA.
- Proposal: install dist with `cargo install --locked cargo-dist@0.34.0` (slower by a few minutes) or check the installer's SHA-256 before running it.
- Gain: a compromised release asset of dist cannot run in the job that later holds `contents: write` artifacts.
- Compatibility: none.
- Status: proposed

## A tag namespace for Rust releases

- Found: 2026-11-13, M5, `.github/workflows/release.yml`
- Today: the release workflow runs on every tag that looks like a version, so a Python `v0.4.1` tag also starts it; `dist plan` then fails because no package has that version, and nothing is published, but the run shows red.
- Proposal: either stop tagging Python releases after `1.0.0-rc.1`, or let the workflow skip tags below `v1` (dist's `tag-namespace` or an `if:` on `plan`).
- Gain: no failed release runs for Python tags.
- Compatibility: none.
- Status: proposed

## Wheels for musl and an older glibc floor for the archives

- Found: 2026-11-13, M5, `.github/workflows/release.yml`, `dist-workspace.toml`
- Today: the release builds five platform wheels and no sdist, so on a platform without a wheel (Alpine/musl, ppc64le) `pip install --pre dejafix` falls back to the last Python 0.4.x release without a warning. The dist archives for Linux are built on ubuntu-22.04 and need glibc 2.35, while the manylinux wheels need 2.17.
- Proposal: add `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` to both dist and the wheel matrix (musllinux wheels, static archives), or build the gnu archives with `cargo zigbuild` against glibc 2.17.
- Gain: one set of supported systems for every channel; no silent fallback to Python on Alpine; RHEL 8 and Debian 11 can use the shell installer.
- Compatibility: none.
- Status: proposed

## Proxy settings through config/

- Found: 2026-11-13, M5, `rust/crates/dejafix-core/src/adapters/http/pypi.rs`
- Today: the update check uses ureq's default configuration, which reads `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` itself, as Python's `urllib` does. That is the only environment read outside `config/`.
- Proposal: read the proxy variables in `config/` and pass an explicit proxy (or none) to `latest_version`, re-implementing ureq's `NO_PROXY` matching.
- Gain: the code rule holds without exception.
- Compatibility: none, if the matching is reproduced exactly.
- Status: proposed
