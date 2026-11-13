import json
import os
from pathlib import Path

import pytest
from conftest import set_fake_home

from dejafix.commands import migrate
from dejafix.project_registry import registry_path

OLD_HOOK = (
    "#!/bin/sh\n"
    "echo user hook\n"
    "# >>> projectmem auto-capture >>>\n"
    "pjm auto-capture\n"
    "# <<< projectmem auto-capture <<<\n"
)
OLD_BRIDGE = (
    "# CLAUDE.md\n\nMy own rules.\n\n"
    "<!-- >>> projectmem bridge >>> -->\n"
    "## projectmem (MANDATORY)\nEdit .projectmem/summary.md never.\n"
    "<!-- <<< projectmem bridge <<< -->\n"
)


@pytest.fixture
def legacy(tmp_path, monkeypatch):
    set_fake_home(monkeypatch, tmp_path / "home")
    old_home = tmp_path / "home" / ".projectmem"
    (old_home / "global").mkdir(parents=True)
    (old_home / "global" / "library_gotchas.jsonl").write_text("{}\n")

    root = tmp_path / "repo"
    mem = root / ".projectmem"
    (mem / "issues").mkdir(parents=True)
    (mem / "events.jsonl").write_text(
        json.dumps({"location": ".projectmem/summary.md", "summary": "keep me"}) + "\n"
    )
    (mem / "AI_INSTRUCTIONS.md").write_text(
        "# projectmem AI Instructions\n\n<!-- projectmem-instructions: 2 -->\n"
    )
    (root / ".gitignore").write_text("node_modules/\n.projectmem/events.jsonl\n")
    (root / "CLAUDE.md").write_text(OLD_BRIDGE)
    (root / ".git" / "hooks").mkdir(parents=True)
    (root / ".git" / "hooks" / "pre-commit").write_text(OLD_HOOK)
    (old_home / "projects.json").write_text(json.dumps([str(root)]))
    return old_home, root


def test_dry_run_changes_nothing(legacy):
    old_home, root = legacy
    migrate.run(apply=False, old_home=old_home)
    assert (root / ".projectmem").is_dir()
    assert not (root / ".dejafix").exists()
    assert (old_home / "projects.json").exists()
    assert (root / "CLAUDE.md").read_text() == OLD_BRIDGE


def test_apply_moves_memory_home_and_hooks(legacy):
    old_home, root = legacy
    migrate.run(apply=True, old_home=old_home)

    new_home = registry_path().parent
    assert not old_home.exists()
    assert json.loads((new_home / "projects.json").read_text()) == [str(root)]
    assert (new_home / "global" / "library_gotchas.jsonl").exists()

    mem = root / ".dejafix"
    assert not (root / ".projectmem").exists()
    event = json.loads((mem / "events.jsonl").read_text())
    assert event == {"location": ".dejafix/summary.md", "summary": "keep me"}
    assert "dejafix-instructions" in (mem / "AI_INSTRUCTIONS.md").read_text()
    assert "projectmem-instructions" in (mem / "AI_INSTRUCTIONS.md.bak").read_text()
    assert ".dejafix/events.jsonl" in (root / ".gitignore").read_text()

    claude = (root / "CLAUDE.md").read_text()
    assert "My own rules." in claude
    assert "projectmem" not in claude
    assert claude.count("<!-- >>> dejafix bridge >>> -->") == 1

    hook = (root / ".git" / "hooks" / "pre-commit").read_text()
    assert "echo user hook" in hook
    assert "projectmem auto-capture" not in hook and "pjm auto-capture" not in hook
    assert hook.count("# >>> dejafix auto-capture >>>") == 1


def test_a_second_run_is_a_no_op(legacy, capsys):
    old_home, root = legacy
    migrate.run(apply=True, old_home=old_home)
    claude = (root / "CLAUDE.md").read_text()
    migrate.run(apply=True, old_home=old_home)
    assert "already migrated" in capsys.readouterr().out
    assert (root / "CLAUDE.md").read_text() == claude


def test_existing_dejafix_folder_is_never_overwritten(legacy, capsys):
    old_home, root = legacy
    (root / ".dejafix").mkdir()
    migrate.run(apply=True, old_home=old_home)
    assert (root / ".projectmem" / "events.jsonl").exists()
    assert "merge them by hand" in capsys.readouterr().out


def test_project_with_a_running_old_watcher_is_skipped(legacy, capsys):
    old_home, root = legacy
    (root / ".projectmem" / "watch.pid").write_text(str(os.getpid()))
    migrate.run(apply=True, old_home=old_home)
    assert (root / ".projectmem").is_dir()
    assert "pjm watch --stop" in capsys.readouterr().out


def test_a_home_stamped_by_a_first_djf_run_is_replaced(legacy):
    old_home, _ = legacy
    (old_home / "projects.meta.json").write_text(json.dumps({"active_project": "repo"}))
    new_home = registry_path().parent
    (new_home / "global").mkdir(parents=True, exist_ok=True)
    (new_home / "projects.meta.json").write_text(json.dumps({"seen_version": "0.4.0"}))
    migrate.run(apply=True, old_home=old_home)
    meta = json.loads((new_home / "projects.meta.json").read_text())
    assert meta["active_project"] == "repo"
    assert "seen_version" not in meta
    assert (new_home / "global" / "library_gotchas.jsonl").exists()
    assert not old_home.exists()


def test_home_entry_with_data_in_both_places_is_kept(legacy, capsys):
    old_home, _ = legacy
    (registry_path().parent / "projects.json").write_text("[]")
    new_global = registry_path().parent / "global"
    new_global.mkdir(parents=True, exist_ok=True)
    (new_global / "library_gotchas.jsonl").write_text("new\n")
    migrate.run(apply=True, old_home=old_home)
    assert (new_global / "library_gotchas.jsonl").read_text() == "new\n"
    assert (old_home / "global" / "library_gotchas.jsonl").exists()
    assert "merge it by hand" in capsys.readouterr().out


def test_unregistered_project_given_by_path(tmp_path, monkeypatch):
    set_fake_home(monkeypatch, tmp_path / "home")
    root = tmp_path / "loose"
    (root / ".projectmem").mkdir(parents=True)
    migrate.run(apply=True, extra_roots=[root], old_home=tmp_path / "missing")
    assert (root / ".dejafix").is_dir()


def test_stale_client_configs_are_reported(tmp_path, monkeypatch, capsys):
    home = tmp_path / "home"
    set_fake_home(monkeypatch, home)
    (home / ".codex").mkdir(parents=True)
    (home / ".codex" / "config.toml").write_text('[mcp_servers.projectmem]\ncommand = "pjm-mcp"\n')
    monkeypatch.setenv("PROJECTMEM_ROOT", "/x")
    migrate.run(apply=False, old_home=tmp_path / "missing")
    out = capsys.readouterr().out
    assert str(Path(home / ".codex" / "config.toml")) in out
    assert "PROJECTMEM_ROOT" in out


def test_dry_run_does_not_repair_the_legacy_registry(legacy):
    old_home, _ = legacy
    migrate.run(apply=False, old_home=old_home)
    assert not (old_home / "projects.meta.json").exists()


def test_a_run_that_stopped_after_the_rename_is_finished(legacy):
    old_home, root = legacy
    (root / ".projectmem").rename(root / ".dejafix")
    migrate.run(apply=True, old_home=old_home)
    event = json.loads((root / ".dejafix" / "events.jsonl").read_text())
    assert event["location"] == ".dejafix/summary.md"
    assert "dejafix-instructions" in (root / ".dejafix" / "AI_INSTRUCTIONS.md").read_text()
    assert "projectmem bridge" not in (root / "CLAUDE.md").read_text()
    assert "projectmem auto-capture" not in (root / ".git" / "hooks" / "pre-commit").read_text()
    assert ".projectmem" not in (root / ".gitignore").read_text()


def test_a_bridge_already_added_by_djf_init_is_not_duplicated(legacy):
    old_home, root = legacy
    (root / "CLAUDE.md").write_text(
        OLD_BRIDGE + "\n<!-- >>> dejafix bridge >>> -->\nold\n<!-- <<< dejafix bridge <<< -->\n"
    )
    migrate.run(apply=True, old_home=old_home)
    claude = (root / "CLAUDE.md").read_text()
    assert "My own rules." in claude
    assert "projectmem bridge" not in claude
    assert claude.count("<!-- >>> dejafix bridge >>> -->") == 1


def test_a_relative_path_gets_the_folder_name(tmp_path, monkeypatch, capsys):
    set_fake_home(monkeypatch, tmp_path / "home")
    root = tmp_path / "loose"
    (root / ".projectmem").mkdir(parents=True)
    monkeypatch.chdir(root)
    migrate.run(apply=False, extra_roots=[Path(".")], old_home=tmp_path / "missing")
    assert f"Project loose ({root.resolve()})" in capsys.readouterr().out


def test_a_symlinked_memory_folder_is_left_alone(tmp_path, monkeypatch, capsys):
    set_fake_home(monkeypatch, tmp_path / "home")
    elsewhere = tmp_path / "elsewhere"
    elsewhere.mkdir()
    (elsewhere / "summary.md").write_text("see .projectmem/summary.md\n")
    root = tmp_path / "repo"
    root.mkdir()
    (root / ".projectmem").symlink_to(elsewhere)
    migrate.run(apply=True, extra_roots=[root], old_home=tmp_path / "missing")
    assert (root / ".projectmem").is_symlink()
    assert (elsewhere / "summary.md").read_text() == "see .projectmem/summary.md\n"
    assert "symlink" in capsys.readouterr().out


def test_unregistered_home_data_is_not_deleted(legacy, capsys):
    old_home, _ = legacy
    new_global = registry_path().parent / "global"
    new_global.mkdir(parents=True, exist_ok=True)
    (new_global / "library_gotchas.jsonl").write_text("new\n")
    migrate.run(apply=True, old_home=old_home)
    assert (new_global / "library_gotchas.jsonl").read_text() == "new\n"
    assert "merge it by hand" in capsys.readouterr().out


def test_a_folder_named_projectmem_is_not_a_stale_config(tmp_path, monkeypatch, capsys):
    home = tmp_path / "home"
    set_fake_home(monkeypatch, home)
    (home / ".codex").mkdir(parents=True)
    (home / ".codex" / "config.toml").write_text(
        '[projects."/src/projectmem"]\ntrust_level = "trusted"\n'
    )
    for key in [k for k in os.environ if k.startswith(("PROJECTMEM_", "PJM_"))]:
        monkeypatch.delenv(key)
    migrate.run(apply=False, old_home=tmp_path / "missing")
    assert "Still pointing at projectmem" not in capsys.readouterr().out
