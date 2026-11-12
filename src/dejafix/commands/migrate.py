"""djf migrate — carry a projectmem install over to dejafix.

dejafix began as a fork of projectmem and renamed everything it writes: the
per-project `.projectmem/` folder, the `~/.projectmem/` home, the git-hook and
CLAUDE.md/AGENTS.md markers, and the generated AI instructions. This command
moves an existing install across so no memory is lost.

It is a dry run unless `--apply` is given. Every step checks what is still
left to convert, so a run that stopped halfway is finished by running it
again, and nothing that already exists on the dejafix side is overwritten.
Client configs (MCP servers, edit hooks) are only reported — they are the
user's files, written by hand, and `djf init` never writes them either.
"""
from __future__ import annotations

import os
import re
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path

import typer

from dejafix.commands.hooks import (
    HOOK_CONFIGS,
    HOOK_MARKER_END,
    HOOK_MARKER_START,
    install_hooks,
)
from dejafix.commands.init import (
    _CLAUDE_MD_BRIDGE_END,
    _CLAUDE_MD_BRIDGE_START,
    _client_configs,
    _ensure_claude_md,
)
from dejafix.commands.watch import _pid_alive
from dejafix.project_registry import load_registry, meta_path, registry_path, slugify
from dejafix.storage import (
    AI_INSTRUCTIONS_FILE,
    INSTRUCTIONS_VERSION,
    MEM_DIR,
    instructions_version,
    refresh_ai_instructions,
)

LEGACY_DIR = ".projectmem"
LEGACY_PATH_RE = re.compile(r"\.projectmem\b")
# What a client config or hook file contains while it still runs projectmem.
# The bare word is not enough: it matches any folder that happens to be named
# projectmem, such as a checkout of the original repository.
LEGACY_CONFIG_TOKENS = (
    "projectmem.mcp_server",
    "mcp_servers.projectmem",
    '"projectmem":',
    "pjm-mcp",
    "pjm-edit-hook",
    "PJM_EDIT_HOOK_LOG",
    "PROJECTMEM_",
)
LEGACY_ENV_PREFIXES = ("PROJECTMEM_", "PJM_")
TEXT_SUFFIXES = {".md", ".jsonl", ".json", ".toml", ".bak"}
BRIDGE_FILES = ("CLAUDE.md", "AGENTS.md")
HOOK_NAMES = (*HOOK_CONFIGS, "pre-commit")
# The tool's home holds nothing yet but the version stamp any `djf` run writes.
STAMP_ONLY = "projects.meta.json"


@dataclass(frozen=True)
class Block:
    """A marked block projectmem wrote into a file it shares with the user."""

    old_start: str
    old_end: str
    new_start: str
    new_end: str


HOOK_BLOCK = Block(
    "# >>> projectmem auto-capture >>>", "# <<< projectmem auto-capture <<<",
    HOOK_MARKER_START, HOOK_MARKER_END,
)
BRIDGE_BLOCK = Block(
    "<!-- >>> projectmem bridge >>> -->", "<!-- <<< projectmem bridge <<< -->",
    _CLAUDE_MD_BRIDGE_START, _CLAUDE_MD_BRIDGE_END,
)


@dataclass(frozen=True)
class LegacyProject:
    name: str
    root: Path


def legacy_home() -> Path:
    return Path.home() / LEGACY_DIR


def _read(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


def _write_atomic(path: Path, text: str) -> None:
    """Replace a file in one step — events.jsonl must never be left half-written."""
    fd, tmp = tempfile.mkstemp(dir=str(path.parent), prefix=".djf-", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as handle:
            handle.write(text)
        shutil.copymode(path, tmp)
        os.replace(tmp, path)
    except OSError:
        Path(tmp).unlink(missing_ok=True)
        raise


def _has_old_block(path: Path, block: Block) -> bool:
    return block.old_start in (_read(path) or "")


def _convert_block(path: Path, block: Block) -> None:
    """Hand projectmem's block over to dejafix.

    The markers are renamed so the dejafix writer refreshes the block in place.
    Where `djf init` already added a dejafix block, the old one is dropped
    instead, so the file never ends up with two.
    """
    text = _read(path)
    if text is None or block.old_start not in text:
        return
    if block.new_start in text:
        pattern = re.escape(block.old_start) + r".*?" + re.escape(block.old_end) + r"\n?"
        text = re.sub(pattern, "", text, flags=re.DOTALL)
    else:
        text = text.replace(block.old_start, block.new_start)
        text = text.replace(block.old_end, block.new_end)
    _write_atomic(path, text)


def _rewrite_paths(path: Path) -> None:
    text = _read(path)
    if text is None:
        return
    updated = LEGACY_PATH_RE.sub(MEM_DIR, text)
    if updated != text:
        _write_atomic(path, updated)


def _files_with_legacy_paths(mem: Path) -> list[Path]:
    """Memory files that still record `.projectmem` paths, relative to `mem`."""
    return [
        path.relative_to(mem)
        for path in sorted(mem.rglob("*"))
        if path.is_file()
        and not path.is_symlink()
        and path.suffix in TEXT_SUFFIXES
        and LEGACY_PATH_RE.search(_read(path) or "")
    ]


def _holds_files(path: Path) -> bool:
    return path.is_file() or any(p.is_file() for p in path.rglob("*"))


def _live_watcher(legacy: Path) -> int | None:
    try:
        pid = int((legacy / "watch.pid").read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None
    return pid if _pid_alive(pid) else None


def _legacy_projects(old_home: Path, new_home: Path, apply: bool) -> list[LegacyProject]:
    """Registered projects, read before the home moves.

    The legacy registry is read when it exists — after a partial run the
    projects may already be listed under the new home instead. load_registry()
    repairs a registry it reads, so a dry run reads a throwaway copy.
    """
    registry_file = old_home / "projects.json"
    if not registry_file.exists():
        registry_file = new_home / "projects.json"
    if apply or not registry_file.exists():
        registry = load_registry(registry_file)
    else:
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / registry_file.name
            shutil.copy2(registry_file, copy)
            if meta_path(registry_file).exists():
                shutil.copy2(meta_path(registry_file), meta_path(copy))
            registry = load_registry(copy)
    return [LegacyProject(r.name, r.path) for r in registry.projects]


def _migrate_home(old_home: Path, new_home: Path, apply: bool) -> None:
    if not old_home.is_dir():
        typer.echo(f"Home: {old_home} not found, nothing to move.")
        return
    typer.echo(f"Home: {old_home} -> {new_home}")
    if apply:
        new_home.mkdir(parents=True, exist_ok=True)
    in_use = (new_home / "projects.json").exists()
    for entry in sorted(old_home.iterdir()):
        dest = new_home / entry.name
        stamp = not in_use and entry.name == STAMP_ONLY
        if dest.exists() and _holds_files(dest) and not stamp:
            typer.echo(f"  skip {entry.name}: {dest} already has data, merge it by hand")
            continue
        typer.echo(f"  move {entry.name}")
        if apply:
            if dest.is_dir():
                shutil.rmtree(dest)
            elif dest.exists():
                dest.unlink()
            shutil.move(str(entry), str(dest))
    if apply and not any(old_home.iterdir()):
        old_home.rmdir()


def _needs_instructions(mem: Path) -> bool:
    text = _read(mem / AI_INSTRUCTIONS_FILE)
    return text is not None and instructions_version(text) < INSTRUCTIONS_VERSION


def _migrate_project(project: LegacyProject, apply: bool) -> None:
    root = project.root
    legacy = root / LEGACY_DIR
    target = root / MEM_DIR
    label = f"Project {project.name} ({root})"

    if legacy.is_symlink() or target.is_symlink():
        typer.echo(f"{label}: the memory folder is a symlink, migrate it by hand.")
        return
    rename = legacy.is_dir()
    if rename and target.exists():
        typer.echo(f"{label}: both {LEGACY_DIR}/ and {MEM_DIR}/ exist, merge them by hand.")
        return
    if not rename and not target.is_dir():
        typer.echo(f"{label}: no {LEGACY_DIR}/ found, skipped.")
        return
    pid = _live_watcher(legacy) if rename else None
    if pid is not None:
        typer.echo(
            f"{label}: the old watcher (pid {pid}) is still running. "
            "Stop it with `pjm watch --stop` in that project, then re-run."
        )
        return

    hooks_dir = root / ".git" / "hooks"
    bridges = [root / n for n in BRIDGE_FILES if _has_old_block(root / n, BRIDGE_BLOCK)]
    hooks = [hooks_dir / n for n in HOOK_NAMES if _has_old_block(hooks_dir / n, HOOK_BLOCK)]
    gitignore = root / ".gitignore"
    rewrites_gitignore = LEGACY_PATH_RE.search(_read(gitignore) or "") is not None
    mem = legacy if rename else target
    instructions = _needs_instructions(mem)
    stale_paths = _files_with_legacy_paths(mem)

    steps = []
    if rename:
        steps.append(f"rename {LEGACY_DIR}/ -> {MEM_DIR}/")
    if stale_paths:
        steps.append(f"update {LEGACY_DIR} paths recorded in {len(stale_paths)} memory files")
    if instructions:
        steps.append(f"refresh {AI_INSTRUCTIONS_FILE} (previous copy kept as .bak)")
    if rewrites_gitignore:
        steps.append("update .gitignore entries")
    if bridges:
        steps.append(f"replace the bridge block in {', '.join(p.name for p in bridges)}")
    if hooks:
        steps.append(f"reinstall git hooks: {', '.join(p.name for p in hooks)}")
    if not steps:
        typer.echo(f"{label}: already migrated.")
        return
    typer.echo(f"{label}:")
    for step in steps:
        typer.echo(f"  {step}")
    if not apply:
        return

    if rename:
        legacy.rename(target)
    for relative in stale_paths:
        _rewrite_paths(target / relative)
    if rewrites_gitignore:
        _rewrite_paths(gitignore)
    if instructions:
        refresh_ai_instructions(root)
    if bridges:
        for path in bridges:
            _convert_block(path, BRIDGE_BLOCK)
        _ensure_claude_md(root, project.name)
    if hooks:
        for path in hooks:
            _convert_block(path, HOOK_BLOCK)
        install_hooks(hooks_dir)


def _report_leftovers() -> None:
    home = Path.home()
    files = [
        *_client_configs(),
        ("Claude Code hooks", home / ".claude/settings.json"),
        ("Codex hooks", home / ".codex/hooks.json"),
    ]
    stale = [
        (name, path)
        for name, path in files
        if any(token in (_read(path) or "") for token in LEGACY_CONFIG_TOKENS)
    ]
    env = sorted(k for k in os.environ if k.startswith(LEGACY_ENV_PREFIXES))
    if not (stale or env):
        return
    typer.echo("\nStill pointing at projectmem — update these by hand:")
    for name, path in stale:
        typer.echo(f"  {name}: {path}")
    for key in env:
        typer.echo(f"  environment variable {key}")
    typer.echo(
        "  MCP server: `djf-mcp` or `python -m dejafix.mcp_server` (`djf init` "
        "prints a block to copy). Edit hook: `djf-edit-hook`, log variable "
        "DJF_EDIT_HOOK_LOG. Variables PROJECTMEM_* become DEJAFIX_*."
    )


def run(
    apply: bool = False,
    extra_roots: list[Path] | None = None,
    old_home: Path | None = None,
) -> None:
    old_home = old_home or legacy_home()
    new_home = registry_path().parent
    projects = _legacy_projects(old_home, new_home, apply)
    known = {p.root.resolve() for p in projects}
    for root in extra_roots or []:
        resolved = root.resolve()
        if resolved not in known:
            projects.append(LegacyProject(slugify(resolved.name), resolved))

    if apply:
        typer.echo(
            "Close running agent sessions (Claude Code, Codex, Cursor, Claude "
            "Desktop) first: a projectmem server still running writes to the old "
            "folders.\n"
        )
    else:
        typer.echo("Dry run — nothing is changed. Re-run with --apply to migrate.\n")
    _migrate_home(old_home, new_home, apply)
    for project in projects:
        _migrate_project(project, apply)
    _report_leftovers()
    if apply:
        typer.echo(
            "\nDone. Commit the renamed .dejafix/ folder in each project, and "
            "uninstall projectmem once your clients point at dejafix."
        )
