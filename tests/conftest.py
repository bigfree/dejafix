"""Shared test fixtures.

Run the session from a temporary folder with no project memory above it, so a
checkout that keeps its own `.dejafix/` is never read or written by the tests.
Isolate the cross-project registry (and any $DEJAFIX_HOME-scoped state) into a
per-test temp dir, so tests that run `djf init` never write to the real user
registry at ~/.dejafix/projects.json.
"""
import os
import shutil
import tempfile

import pytest

from dejafix.storage import discover_mem_dir

_session_dirs: dict[str, str] = {}


def pytest_sessionstart(session: pytest.Session) -> None:
    """Leave the checkout before any test module is imported.

    Commands walk up from the working directory to the nearest `.dejafix/`, and
    the MCP server pins itself to it, or to $DEJAFIX_ROOT, when it is imported.
    This checkout keeps its own memory, so without this the tests would read
    and write the real project memory instead of their temporary ones. It runs
    here rather than at import so that output paths pytest resolves while it
    configures itself (--junitxml, --basetemp) still resolve against the
    folder the run was started from.
    """
    os.environ.pop("DEJAFIX_ROOT", None)
    _session_dirs["start"] = os.getcwd()
    _session_dirs["neutral"] = tempfile.mkdtemp(prefix="djf-tests-")
    os.chdir(_session_dirs["neutral"])
    found = discover_mem_dir()
    if found is not None:
        pytest.exit(
            f"The temporary folder {_session_dirs['neutral']} sits below the "
            f"project memory {found}; point TMPDIR somewhere outside it.",
            returncode=pytest.ExitCode.USAGE_ERROR,
        )


def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    if "start" in _session_dirs:
        os.chdir(_session_dirs["start"])
        shutil.rmtree(_session_dirs["neutral"], ignore_errors=True)


@pytest.fixture(autouse=True)
def _isolate_dejafix_home(tmp_path_factory, monkeypatch):
    # Use a SEPARATE temp dir (not the test's own tmp_path), so a test that
    # chdir()s into its workdir doesn't see the registry home as a subfolder —
    # folder auto-detection would otherwise pick it up and change init output.
    home = tmp_path_factory.mktemp("pmhome")
    monkeypatch.setenv("DEJAFIX_HOME", str(home))


def set_fake_home(monkeypatch, path) -> None:
    """Point Path.home() at `path` on every platform.

    Tests used to set $HOME alone. Windows ignores it — Path.home() reads
    %USERPROFILE% there — so the fixture home was silently unused and the real
    profile got scanned instead, which fails on a developer's machine and would
    scan their actual disk in CI. Setting both keeps one call site portable.
    """
    monkeypatch.setenv("HOME", str(path))
    monkeypatch.setenv("USERPROFILE", str(path))
