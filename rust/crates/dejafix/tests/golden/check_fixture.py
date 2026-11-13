"""Golden harness: check that Python reads a Rust-generated fixture.

Every line must load through ``read_events`` and must already be in the form
``json.dumps(sort_keys=True)`` writes, so Python would write the same bytes.
"""

import json
import sys
from pathlib import Path

from dejafix.storage import read_events


def main(root: Path) -> int:
    log = root / ".dejafix" / "events.jsonl"
    lines = log.read_text(encoding="utf-8").splitlines()
    for number, line in enumerate(lines, 1):
        if json.dumps(json.loads(line), sort_keys=True) != line:
            print(f"line {number} is not canonical: {line}", file=sys.stderr)
            return 1
    events = read_events(root)
    if len(events) != len(lines):
        print(f"read {len(events)} events from {len(lines)} lines", file=sys.stderr)
        return 1
    print(f"ok {len(events)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
