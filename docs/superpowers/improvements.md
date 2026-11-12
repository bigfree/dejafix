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
- Gain: half the git processes per precheck; a budget that can be met on macOS.
- Compatibility: HIGH CHURN counts can differ from Python on histories whose committer dates go backwards.
- Status: proposed

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
