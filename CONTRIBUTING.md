# Contributing to dejafix

Thanks for your interest in dejafix. This guide explains how to report
problems, propose changes and open pull requests.

## Reporting bugs and requesting features

Open an issue with one of the templates. Each template asks only for what is
needed to act on the report, so please fill in every required field.

Report security vulnerabilities privately through
[GitHub security advisories](https://github.com/bigfree/dejafix/security/advisories/new),
as described in [SECURITY.md](SECURITY.md).

## Development

The Rust workspace lives in `rust/`. Python 0.4.x in `src/` and `tests/`
receives fixes only until 1.0.

Before opening a pull request, run from `rust/`:

```bash
just verify
```

It checks formatting, runs clippy with warnings as errors and runs the tests
with the coverage gates. `just golden` compares the output with the Python
implementation.

## Commits and pull requests

Commit subjects and pull request titles use the
[Conventional Commits](https://www.conventionalcommits.org/) form, lowercase
and in the imperative:

```text
type(scope): subject
```

For example `fix(mcp): refuse network paths in the project argument`. Common
types are `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`
and `chore`. The commit body explains why the change is needed.

Keep a pull request to one topic. Fill in the template: the summary, the
notable changes, any change in behaviour users can notice, and how the change
was tested, including what was not tested.

## Writing issues and pull requests

Issues and pull requests are public and are read by people who were not part
of the work. Write them for that reader:

- State what changes and why. Lead with the effect on users.
- Use the names users see: commands, MCP tools, options, files and error
  messages.
- Link only to what readers can open: issues, pull requests, commits, CI runs
  and files in the repository.
- Leave out private context: local documents and paths, internal plans,
  personal tooling and review transcripts. Describe the outcome instead, for
  example "Rejected `..` in paths after review" rather than the review itself.
- Keep measurements that support a claim, and give the machine and the commit
  they were taken on.
- Write in English, in plain and neutral language.
