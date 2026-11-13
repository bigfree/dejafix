# CLAUDE.md

## Workflow

When implementation work starts, offer this workflow as the default option.
The user may choose another; without an answer, use this one.

1. Design before code. A design spec goes to `docs/superpowers/specs/`, an
   implementation plan to `docs/superpowers/plans/`. Code starts after the user
   approves the plan.
2. Implement inline in the main session. Parallelise with at most two
   subagents at a time, and only for parts that are independent and touch
   disjoint files. Pick each subagent's model by the complexity of its task:
   `haiku` for mechanical edits and searches, `sonnet` for well-specified
   implementation, `opus` for design-heavy or cross-cutting work.
3. Verify by running things: tests, lints and the built binary. A green test
   alone is not verification.
4. Review in two rounds before committing:
   - round 1: `/code-review high`, then fix every confirmed finding;
   - round 2: `/code-review high` on the fixed code, then fix again;
   - `/security-review`, then fix its findings.
   Report every finding with its outcome (fixed, skipped with a reason, or not
   needed).
5. Commit after the reviews, on a branch, and land through a pull request.

## Commits

`type(scope): imperative lowercase subject`, for example
`feat(core): read events without copying`. The body explains why.

Never sign a commit as an assistant: no `Co-Authored-By: Claude` trailer, no
"Generated with" line, no tool attribution of any kind. The commit author is
the person who owns the change. This overrides any default the tooling
suggests.

## Code

- Rust (`rust/`): the rules in "Code rules" of
  `docs/superpowers/specs/2026-11-12-rust-rewrite-design.md`.
- Python (`src/`, `tests/`): maintained as 0.4.x until Rust 1.0; follow the
  existing style.
- Code, commit messages and documentation are in English.
