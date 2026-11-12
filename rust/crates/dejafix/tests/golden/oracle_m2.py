"""Golden oracle for M2: the Python behaviour the write path, summary and CLI port.

``casefold``  every code point whose ``casefold()`` differs: ``hex<TAB>json``.
``dumps``     each stdin JSON line as ``indent=1`` and ``indent=2, sort_keys``.
``redact``    each stdin JSON string through ``redact``: ``[text, kinds]``.
``summary ROOT``  ``build_summary``, the key-file counts and ``write_issue_files``.
``purpose``   both purpose extractors over each stdin JSON string.
``stack ROOT...``  ``detect_stack`` of each root as sorted JSON.
``promote``   ``auto_promote_event`` per stdin ``[summary, type, outcome, libraries]``.
``word``      ranges ``start-end`` (hex) of code points where ``isalnum()`` or ``_``.
"""

import json
import sys


def casefold() -> None:
    for code in range(0x110000):
        if 0xD800 <= code <= 0xDFFF:
            continue
        char = chr(code)
        folded = char.casefold()
        if folded != char:
            print(f"{code:x}\t{json.dumps(folded)}")


def word() -> None:
    start = None
    for code in range(0x110001):
        char = chr(code) if code < 0x110000 and not 0xD800 <= code <= 0xDFFF else ""
        inside = bool(char) and (char.isalnum() or char == "_")
        if inside and start is None:
            start = code
        elif not inside and start is not None:
            print(f"{start:x}-{code - 1:x}")
            start = None


def dumps() -> None:
    for line in stdin_lines():
        doc = json.loads(line)
        print(json.dumps(doc, indent=1))
        print("\x00")
        print(json.dumps(doc, indent=2, sort_keys=True))
        print("\x00")


def redact_lines() -> None:
    from dejafix.redaction import redact

    for line in stdin_lines():
        out, matched = redact(json.loads(line))
        print(json.dumps([out, matched]))


def stdin_lines() -> list[str]:
    text = sys.stdin.buffer.read().decode("utf-8")
    return [line for line in text.split("\n") if line]


def summary(root: str) -> None:
    from collections import defaultdict
    from pathlib import Path

    from dejafix.models import location_to_file
    from dejafix.storage import read_events
    from dejafix.summary import build_summary, infer_file_mentions, write_issue_files

    project = Path(root)
    events = read_events(project)
    text = build_summary(events, project)
    write_issue_files(events, project)
    counts: dict[str, int] = defaultdict(int)
    for event in events:
        mentioned = [*event.files, *([event.location] if event.location else []), *infer_file_mentions(event.summary)]
        for path in {location_to_file(" ".join(p.split())) for p in mentioned}:
            if path and not path.startswith(".dejafix/"):
                counts[path] += 1
    issues = {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted((project / ".dejafix" / "issues").iterdir())
    }
    print(json.dumps({"summary": text, "counts": counts, "issues": issues}))


def purpose() -> None:
    from dejafix.summary import extract_project_purpose
    from dejafix.summary import extract_project_purpose_from_map
    import tempfile
    from pathlib import Path

    for line in stdin_lines():
        text = json.loads(line)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "map.md"
            path.write_text(text, encoding="utf-8", newline="")
            from_map = extract_project_purpose_from_map(path)
        print(json.dumps([from_map, extract_project_purpose(text)]))


def stack(*roots: str) -> None:
    from pathlib import Path

    from dejafix.global_memory import detect_stack

    for root in roots:
        print(json.dumps(detect_stack(Path(root)), sort_keys=True))


def promote() -> None:
    from dejafix.global_memory import auto_promote_event, gotchas_path

    for line in stdin_lines():
        summary, kind, outcome, libraries = json.loads(line)
        gotchas_path().unlink(missing_ok=True)
        entry = auto_promote_event(summary, kind, "proj", project_libraries=libraries, outcome=outcome)
        print(json.dumps(entry["library"] if entry else None))


COMMANDS = {"stack": stack, "promote": promote, "summary": summary, "purpose": purpose, "redact": redact_lines, "casefold": casefold, "word": word, "dumps": dumps}

if __name__ == "__main__":
    COMMANDS[sys.argv[1]](*sys.argv[2:])
