"""Golden oracle for M1: run the Python functions on the input Rust also reads.

``rewrite LOG``  one line per event, ``json.dumps(sort_keys=True)`` of what
                 ``Event.from_dict`` keeps plus the unknown keys left on disk,
                 then ``SKIP <line>`` for every line ``from_dict`` rejects.
``index ROOT``   open, closed and retired issues and precheck's file index.
``scalars KIND [ROOT]``  one JSON string per stdin line in, one JSON result out.
``chars``        code points where ``isspace``, ``isdecimal`` and
                 ``splitlines`` apply.
"""

import dataclasses
import json
import sys
from pathlib import Path

from dejafix.commands.precheck import _live_event_paths
from dejafix.models import Event, location_to_file, normalize_timestamp, superseded_ids
from dejafix.staleness import is_memory_path, project_relative
from dejafix.storage import normalize_issue_id, open_issue_ids

KNOWN = {field.name for field in dataclasses.fields(Event)}


def load(log: Path):
    text = log.read_bytes().decode("utf-8-sig")
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            raw = json.loads(line)
            yield number, raw, Event.from_dict(raw)
        except Exception:
            yield number, None, None


def rewrite(log: Path) -> None:
    skipped = []
    for number, raw, event in load(log):
        if event is None:
            skipped.append(number)
            continue
        out = {key: value for key, value in raw.items() if key not in KNOWN}
        out.update(event.to_dict())
        print(json.dumps(out, sort_keys=True))
    for number in skipped:
        print(f"SKIP {number}")


def index(root: Path) -> None:
    events = [event for _, _, event in load(root / ".dejafix" / "events.jsonl") if event]
    closed = {e.issue_id for e in events if e.type == "fix" and e.issue_id}
    files: dict[str, list[str]] = {}
    for event, paths in _live_event_paths(events, root):
        for path in paths:
            files.setdefault(path, []).append(event.id)
    print(json.dumps({
        "open": open_issue_ids(events),
        "closed": sorted(closed),
        "retired": sorted(superseded_ids(events)),
        "files": files,
    }, sort_keys=True))


def scalars(kind: str, root: Path | None) -> None:
    for line in sys.stdin.read().split("\n"):
        if not line:
            continue
        value = json.loads(line)
        try:
            if kind == "location":
                result = location_to_file(value)
            elif kind == "issue_id":
                result = normalize_issue_id(value)
            elif kind == "timestamp":
                result = normalize_timestamp(value)
            elif kind == "relative":
                result = project_relative(value, root)
            elif kind == "memory":
                result = is_memory_path(value, root)
            else:
                raise SystemExit(f"unknown kind {kind}")
        except (ValueError, OverflowError):
            result = "!!"
        except OSError:
            # Windows: the C runtime cannot convert a date outside 1970-3000.
            result = "!os"
        print(json.dumps(result))


def chars() -> None:
    every = [chr(code) for code in range(0x110000) if not 0xD800 <= code <= 0xDFFF]
    print(json.dumps([ord(c) for c in every if c.isspace()]))
    print(json.dumps([ord(c) for c in every if c.isdecimal()]))
    print(json.dumps([ord(c) for c in every if len(("a" + c + "b").splitlines()) == 2]))


def main(argv: list[str]) -> int:
    command, *rest = argv
    if command == "rewrite":
        rewrite(Path(rest[0]))
    elif command == "index":
        index(Path(rest[0]))
    elif command == "scalars":
        scalars(rest[0], Path(rest[1]) if len(rest) > 1 else None)
    elif command == "chars":
        chars()
    else:
        return 2
    return 0


if __name__ == "__main__":
    # Print \n on every platform: Windows text mode would print \r\n.
    sys.stdout.reconfigure(newline="\n")
    sys.exit(main(sys.argv[1:]))
