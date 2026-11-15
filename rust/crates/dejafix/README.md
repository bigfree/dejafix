# dejafix

Local-first memory and judgment for AI coding agents. One MCP server serves
every project. It warns before an agent repeats a fix that already failed, and
it shares library lessons across projects through a global memory that agents
query.

This package installs the native build of dejafix. It needs no Python at run
time. It provides four executables:

- `djf` and `dejafix`: the command line, for example `djf init`, `djf log`,
  `djf precheck`, `djf search` and `djf global query`;
- `djf-mcp`: the MCP server for Claude Code, Codex, Cursor, Claude Desktop and
  other clients;
- `djf-edit-hook`: the edit hook that shows a file's lessons before an agent
  changes it.

Source, documentation and issues: <https://github.com/bigfree/dejafix>.
