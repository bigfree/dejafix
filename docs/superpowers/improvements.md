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
