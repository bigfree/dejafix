from __future__ import annotations

from datetime import datetime, timedelta, timezone

from projectmem.commands.precheck import _analyze_files
from projectmem.models import Event
from projectmem.storage import initialize


def _project(tmp_path):
    initialize(tmp_path)
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "auth.py").write_text("x = 1\n", encoding="utf-8")
    return tmp_path


def _days_ago(days: int) -> str:
    return (datetime.now(timezone.utc) - timedelta(days=days)).isoformat()


def _lesson_lines(warnings) -> list[str]:
    return [
        detail
        for warning in warnings
        if warning["type"] == "failed_attempts"
        for detail in warning["details"]
    ]


def test_attempt_without_location_inherits_its_issue_file(tmp_path):
    root = _project(tmp_path)
    events = [
        Event(type="issue", issue_id="0001", summary="token refresh race", location="src/auth.py"),
        Event(type="attempt", issue_id="0001", outcome="failed", summary="wrapped refresh in a lock"),
    ]

    lines = _lesson_lines(_analyze_files(["src/auth.py"], events, root=root))

    assert any("wrapped refresh in a lock" in line for line in lines)


def test_partial_attempt_is_a_lesson(tmp_path):
    root = _project(tmp_path)
    events = [
        Event(type="issue", issue_id="0001", summary="token refresh race", location="src/auth.py"),
        Event(type="attempt", issue_id="0001", outcome="partial", summary="retry halves the failures", location="src/auth.py"),
    ]

    lines = _lesson_lines(_analyze_files(["src/auth.py"], events, root=root))

    assert any("retry halves the failures" in line for line in lines)


def test_failure_older_than_thirty_days_still_warns(tmp_path):
    root = _project(tmp_path)
    events = [
        Event(type="issue", issue_id="0001", summary="token refresh race", location="src/auth.py", timestamp=_days_ago(60)),
        Event(type="attempt", issue_id="0001", outcome="failed", summary="pinned the clock skew", location="src/auth.py", timestamp=_days_ago(60)),
    ]

    lines = _lesson_lines(_analyze_files(["src/auth.py"], events, root=root))

    assert any("pinned the clock skew" in line for line in lines)


def test_a_worked_attempt_is_not_a_lesson(tmp_path):
    root = _project(tmp_path)
    events = [
        Event(type="issue", issue_id="0001", summary="token refresh race", location="src/auth.py"),
        Event(type="attempt", issue_id="0001", outcome="worked", summary="serialized refresh per user", location="src/auth.py"),
    ]

    assert not _lesson_lines(_analyze_files(["src/auth.py"], events, root=root))
