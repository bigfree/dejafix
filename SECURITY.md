# Security Policy

dejafix is local-first by design — no telemetry, no cloud, no accounts. Your project memory never leaves your machine unless you explicitly send it to an AI tool (Claude, Cursor, etc.).

## Reporting a Vulnerability

If you've found a security issue, please report it privately through [GitHub security advisories](https://github.com/bigfree/dejafix/security/advisories/new) rather than opening a public issue.

- We aim to respond within 72 hours.
- We'll work with you on disclosure timing.
- Significant fixes will be credited in the changelog (unless you prefer anonymity).

## Honest Trade-offs

dejafix is open source and we'd rather be transparent about the trade-offs than hide them. The most up-to-date discussion is in the [Privacy section of the README](https://github.com/bigfree/dejafix#privacy). The short version:

### Where data lives

| Location | Contents | Shared? |
|----------|----------|---------|
| `.dejafix/events.jsonl` | Raw event log (failures, fixes, decisions) | Gitignored by default |
| `.dejafix/summary.md` | Distilled briefing for AI agents | Committed to git |
| `.dejafix/PROJECT_MAP.md` | Architecture map | Committed to git |
| `~/.dejafix/global/` | Cross-project patterns and gotchas | Local only, shared across all your projects |
| `.dejafix/watch.pid` · `watch.log` | Watcher state and runtime log | Gitignored, local only |

### Honest trade-offs

1. **Never paste secrets into `djf log/note/decision`.** The event log is append-only. Treat it like git history: don't commit anything you wouldn't want re-read later.

2. **Local-first ≠ data stays on your machine.** dejafix doesn't see your data. But the moment you connect it to Claude, Cursor, ChatGPT, or any cloud AI, your `summary.md` / `PROJECT_MAP.md` contents are sent to that AI vendor as part of normal AI use.

3. **Global memory mixes work and personal projects.** `~/.dejafix/global/` is shared across every project on your machine. Use `djf init --no-global` for sensitive repos.

4. **Git hooks execute `djf` on every commit.** `djf init` installs three git hooks (pre-commit, post-commit, post-merge). Inspect them in `.git/hooks/`. Remove with `djf hooks uninstall`.

5. **The watcher is a detached background process.** `djf watch --daemon` auto-starts on `djf init` in interactive terminals. Stop with `djf watch --stop`. Skip auto-start with `djf init --no-watch`.

6. **Memory files are AI instructions — treat like code.** AI agents read `AI_INSTRUCTIONS.md`, `PROJECT_MAP.md`, and `summary.md` as authoritative guidance. Malicious prompt-injection text in those files can manipulate AI behavior. Review changes like you'd review code.

7. **The MCP server is local-only.** `djf-mcp` listens on stdio, not over the network. No remote attack surface — but any local AI client you connect can write to your memory through its 5 write tools (of 17).

## Fully Uninstall

```bash
djf watch --stop          # 1. stop the watcher
djf hooks uninstall       # 2. remove git hooks
rm -rf .dejafix/       # 3. delete project memory (optional)
rm -rf ~/.dejafix/     # 4. delete global memory (optional)
pip uninstall dejafix  # 5. uninstall the package
```

## Scope

The following are **in scope** for security reports:

- Code execution vulnerabilities in `djf` CLI or `djf-mcp` server
- Information disclosure beyond what's documented in the trade-offs above
- Privilege escalation via git hooks
- Malicious-package risks via dependency chain (we use only `typer`, `mcp`, `watchdog`)

The following are **not** security issues (by design):

- AI vendor receiving your memory when you connect to them — that's normal AI use
- Global memory being shared across local projects — that's the documented behavior; use `--no-global` to opt out
- Git hooks running `djf` — that's the documented behavior; uninstall hooks to opt out
- Sensitive data appearing in `events.jsonl` because you typed it — see trade-off #1
