import json
import pathlib
import sys

MS = 1_000_000

BUDGETS = [
    ("process/edit_hook_3000", 10 * MS, "edit hook, whole process, 3 000 events"),
    ("process/precheck_3000", 30 * MS, "djf precheck, whole process, 3 000 events"),
    ("mcp/precheck_file_warm_cited_3000", 5 * MS, "precheck_file warm, cited file"),
    ("mcp/precheck_file_warm_uncited_3000", 5 * MS, "precheck_file warm, uncited file"),
    ("summary/regenerate/3000", 30 * MS, "summary.md regeneration, 3 000 events"),
    ("summary/note/3000", 20 * MS, "djf note, one write, 3 000 events"),
    ("mcp/global_query_warm_500", 1 * MS, "global_query warm, 500 records"),
    ("mcp/global_query_cold_500", 5 * MS, "global_query cold, 500 records"),
    ("mcp/search_events_warm_3000", 5 * MS, "search_events warm, 3 000 events"),
    ("process/search_3000", 30 * MS, "djf search, whole process, 3 000 events"),
    ("process/global_query_500", 20 * MS, "djf global query, whole process, 500 records"),
    ("mcp/add_global_gotcha_500", 10 * MS, "add_global_gotcha, 500 records"),
]


def median(root, name):
    path = root / name / "new" / "estimates.json"
    if not path.exists():
        return None
    return json.loads(path.read_text())["median"]["point_estimate"]


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/criterion")
    failed = False
    for name, budget, label in BUDGETS:
        value = median(root, name)
        if value is None:
            print(f"MISSING  {label}: no estimate at {name}")
            failed = True
            continue
        verdict = "ok" if value <= budget else "OVER"
        failed |= value > budget
        print(f"{verdict:8} {label}: {value / MS:.3f} ms (budget {budget / MS:g} ms)")
    sys.exit(1 if failed else 0)


main()
