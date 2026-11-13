# dejafix

Coding agent memory: local-first memory and judgment for AI coding agents.
One MCP server for every project; it warns before an agent repeats a fix that
already failed.

This package is the native build of dejafix 1.x. It installs four executables
and needs no Python at run time:

- `djf` and `dejafix`: the command line (`djf init`, `djf log`, `djf precheck`,
  `djf doctor`, `djf migrate`, …);
- `djf-mcp`: the MCP server for Claude Code, Codex, Cursor, Claude Desktop and
  other clients;
- `djf-edit-hook`: the edit hook that shows a file's lessons before an agent
  changes it.

Moving from projectmem: `djf migrate` lists what it would change, and
`djf migrate --apply` moves the memory folders, the home folder and the hooks.

Source, documentation and issues: <https://github.com/bigfree/dejafix>.
