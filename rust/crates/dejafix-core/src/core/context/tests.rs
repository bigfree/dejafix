use super::sections::smart_truncate;
use super::{ContextInput, generate_context};
use crate::core::log::EventLog;
use crate::core::path::PathContext;
use crate::testing::{NoResolve, fixed_clock};

const ROOT: &str = "/work/r";

fn context() -> PathContext {
    PathContext {
        root: ROOT.into(),
        cwd: ROOT.into(),
        fold_case: false,
    }
}

fn run(
    lines: &str,
    budget: u32,
    focus: Option<&str>,
    git_files: &[&str],
    project_map: Option<&str>,
) -> String {
    let log = EventLog::parse(lines.as_bytes(), &fixed_clock(), &context(), &NoResolve);
    let git_files: Vec<String> = git_files.iter().map(|file| (*file).to_owned()).collect();
    generate_context(
        &log,
        &ContextInput {
            budget,
            focus,
            recent_days: 30,
            now: fixed_clock().now,
            git_files: &git_files,
            project_map,
        },
    )
}

fn headings(markdown: &str) -> Vec<&str> {
    markdown
        .lines()
        .filter_map(|line| line.strip_prefix("### "))
        .collect()
}

const WARNINGS: &str = "WARNINGS — Do Not Repeat";
const DECISIONS: &str = "Recent Decisions";
const FIXES: &str = "Relevant Fixes";
const GOTCHAS: &str = "File Gotchas";
const ARCH: &str = "Architecture Context";

#[test]
fn every_level_keeps_its_own_sections() {
    let all = [WARNINGS, DECISIONS, FIXES, GOTCHAS, ARCH];
    assert_eq!(headings(&run(MAIN, 4000, None, &[], Some(MAP))), all);
    assert_eq!(headings(&run(MAIN, 3999, None, &[], Some(MAP))), all);
    assert_eq!(headings(&run(MAIN, 1000, None, &[], Some(MAP))), all);
    assert_eq!(
        headings(&run(MAIN, 999, None, &[], Some(MAP))),
        [WARNINGS, DECISIONS, FIXES, GOTCHAS]
    );
    assert_eq!(
        headings(&run(MAIN, 200, None, &[], Some(MAP))),
        [WARNINGS, DECISIONS, FIXES, GOTCHAS]
    );
    assert_eq!(
        headings(&run(MAIN, 199, None, &[], Some(MAP))),
        [WARNINGS, DECISIONS, FIXES]
    );
    assert_eq!(
        headings(&run(MAIN, 100, None, &[], Some(MAP))),
        [WARNINGS, FIXES]
    );
}

#[test]
fn ultra_and_emergency_shorten_what_full_keeps() {
    let full = run(MAIN, 5000, None, &[], None);
    let ultra = run(MAIN, 500, None, &[], None);
    let emergency = run(MAIN, 100, None, &[], None);
    assert!(full.contains("  Files: web/form.ts, web/button.ts, web/test.ts"));
    assert!(!ultra.contains("  Files:"));
    assert!(full.contains("[src/auth/session.py:88]"));
    assert!(ultra.contains("[src/auth/session.py:88]"));
    assert!(!emergency.contains("[src/"));
    assert!(!emergency.contains("starved"));
    assert!(!ultra.contains("starved"));
}

#[test]
fn each_section_stops_at_its_share_of_the_budget() {
    assert_eq!(
        headings(&run(HEAVY_120, 1000, None, &[], Some(MAP))),
        [WARNINGS, DECISIONS, FIXES, GOTCHAS, ARCH]
    );
    assert_eq!(
        headings(&run(HEAVY_330, 1000, None, &[], Some(MAP))),
        [WARNINGS, FIXES, GOTCHAS, ARCH]
    );
    assert_eq!(
        headings(&run(HEAVY_390, 1000, None, &[], Some(MAP))),
        [WARNINGS, GOTCHAS]
    );
    assert_eq!(
        headings(&run(HEAVY_440, 1000, None, &[], Some(MAP))),
        [WARNINGS]
    );
}

#[test]
fn an_empty_log_leaves_the_header() {
    assert_eq!(
        run("", 2000, None, &[], None),
        "## dejafix context (budget: 2000 tokens)\n"
    );
}

#[test]
fn focus_on_a_prefix_a_substring_and_a_directory_changes_the_ranking() {
    let prefix = run(MAIN, 2000, Some("src/auth/"), &[], Some(MAP));
    assert!(prefix.contains("`src/auth/session.py`"));
    assert!(!prefix.contains("`web/form.ts`"));
    assert!(prefix.contains("Notes: src/auth is the hot area"));
    let substring = run(MAIN, 2000, Some("ledger"), &[], Some(MAP));
    assert!(substring.contains("`src/billing/ledger.py`"));
    assert!(!substring.contains("`src/auth/session.py`"));
    let directory = run(MAIN, 2000, Some("src/billing//"), &[], Some(MAP));
    assert!(!directory.contains(GOTCHAS));
    assert_eq!(directory, FOCUS_SAME_DIR);
}

#[test]
fn a_file_in_git_status_lifts_its_events() {
    let plain = run(MAIN, 2000, None, &[], None);
    let boosted = run(MAIN, 2000, None, &["tools/import.py"], None);
    let order = |text: &str| (text.find("Run the importer"), text.find("Use sqlite"));
    let (importer, sqlite) = order(&plain);
    assert!(sqlite < importer);
    let (importer, sqlite) = order(&boosted);
    assert!(importer < sqlite);
}

#[test]
fn an_old_failed_attempt_is_kept_and_an_old_fix_is_dropped() {
    let markdown = run(MAIN, 2000, None, &[], None);
    assert!(markdown.contains("pinned the clock skew"));
    assert!(markdown.contains("(3mo ago)"));
    assert!(!markdown.contains("Dropped the legacy importer"));
    assert!(!markdown.contains("Keep the CLI in Python"));
}

#[test]
fn a_superseded_decision_is_dropped() {
    let lines = [
        r#"{"id": "evt_old", "type": "decision", "timestamp": "2027-01-12T08:00:00Z", "summary": "Use Python http.server for the web UI", "location": "src/api/routes.py:1"}"#,
        r#"{"id": "evt_new", "type": "decision", "timestamp": "2027-01-14T08:00:00Z", "summary": "Switch to TypeScript for the web UI", "location": "src/api/routes.py:1", "supersedes": "evt_old"}"#,
    ]
    .join("\n");
    let markdown = run(&lines, 2000, None, &[], None);
    assert!(markdown.contains("TypeScript"));
    assert!(!markdown.contains("http.server"));
}

#[test]
fn backfill_notes_do_not_reach_file_gotchas() {
    let markdown = run(MAIN, 5000, None, &[], None);
    assert!(!markdown.contains("Auto-backfilled"));
    assert!(!markdown.contains("b1.py"));
    assert!(!markdown.contains("imported history in bulk"));
}

#[test]
fn an_unparsable_timestamp_is_unknown_but_still_scored() {
    let markdown = run(MAIN, 2000, None, &[], None);
    assert!(markdown.contains("tried a longer timeout with a broken clock"));
    assert!(markdown.contains("(unknown)"));
}

#[test]
fn equal_scores_keep_the_log_order() {
    let lines = [
        r#"{"id": "evt_a", "type": "decision", "timestamp": "2027-01-15T07:00:00Z", "summary": "first choice"}"#,
        r#"{"id": "evt_b", "type": "decision", "timestamp": "2027-01-15T07:00:00Z", "summary": "second choice"}"#,
    ]
    .join("\n");
    let markdown = run(&lines, 2000, None, &[], None);
    assert!(markdown.find("first choice") < markdown.find("second choice"));
}

#[test]
fn a_duplicated_file_counts_twice_in_file_gotchas() {
    let lines = [
        r#"{"id": "evt_b", "type": "note", "timestamp": "2027-01-15T07:00:00Z", "summary": "two", "files": ["b.py"]}"#,
        r#"{"id": "evt_a", "type": "note", "timestamp": "2027-01-15T07:00:00Z", "summary": "one", "files": ["a.py"], "location": "a.py:3"}"#,
    ]
    .join("\n");
    let markdown = run(&lines, 2000, None, &[], None);
    assert!(markdown.find("`a.py`") < markdown.find("`b.py`"));
}

#[test]
fn the_compressed_context_snapshot() {
    insta::assert_snapshot!(run(MAIN, 2000, None, &[], Some(MAP)));
}

const MAIN: &str = r#"{"type": "attempt", "summary": "wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved.", "timestamp": "2027-01-15T06:00:00Z", "id": "evt_a01", "outcome": "failed", "location": "src/auth/session.py:88", "files": ["src/auth/session.py"]}
{"type": "attempt", "summary": "rounded cents per line item", "timestamp": "2027-01-12T08:00:00Z", "id": "evt_a02", "outcome": "failed", "location": "src/billing/ledger.py:12", "files": ["src/billing/ledger.py"]}
{"type": "attempt", "summary": "pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed", "timestamp": "2026-10-07T08:00:00Z", "id": "evt_a03", "outcome": "failed", "location": "src/auth/clock.py"}
{"type": "attempt", "summary": "serialized refresh through a queue", "timestamp": "2027-01-13T20:00:00Z", "id": "evt_a04", "outcome": "worked", "location": "src/auth/session.py:90", "files": ["src/auth/session.py"]}
{"type": "attempt", "summary": "retry halves the failures", "timestamp": "2027-01-05T08:00:00Z", "id": "evt_a05", "outcome": "partial", "files": ["src/api/client.ts"]}
{"type": "fix", "summary": "Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite", "timestamp": "2027-01-15T03:00:00Z", "id": "evt_f01", "files": ["web/form.ts", "web/button.ts", "web/test.ts", "web/extra.ts"]}
{"type": "fix", "summary": "Dropped the legacy importer", "timestamp": "2026-10-17T08:00:00Z", "id": "evt_f02", "files": ["tools/import.py"]}
{"type": "fix", "summary": "Bumped the pool size", "timestamp": "2026-12-26T08:00:00Z", "id": "evt_f03"}
{"type": "decision", "summary": "Use redis for the local cache", "timestamp": "2027-01-12T08:00:00Z", "id": "evt_old"}
{"type": "decision", "summary": "Use sqlite for the local cache", "timestamp": "2027-01-13T08:00:00Z", "id": "evt_new", "supersedes": "evt_old"}
{"type": "decision", "summary": "Keep the CLI in Python until 1.0", "timestamp": "2026-12-06T08:00:00Z", "id": "evt_d03"}
{"type": "decision", "summary": "Run the importer nightly, never during the day, because the lock table is shared with billing", "timestamp": "2027-01-06T08:00:00Z", "id": "evt_d04", "location": "tools/import.py:5"}
{"type": "issue", "summary": "token refresh race", "timestamp": "2027-01-15T02:00:00Z", "id": "evt_i01", "location": "src/auth/session.py:70", "issue_id": "0001"}
{"type": "hypothesis", "summary": "clock skew between pods", "timestamp": "2027-01-13T08:00:00Z", "id": "evt_h01", "files": ["src/auth/clock.py"]}
{"type": "note", "summary": "events.jsonl is append-only", "timestamp": "2027-01-07T08:00:00Z", "id": "evt_n01"}
{"type": "note", "summary": "Auto-backfilled from commit abc", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_n02", "notes": "Auto-backfilled from commit abc", "files": ["src/api/client.ts"]}
{"type": "note", "summary": "imported history in bulk", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_n03", "files": ["b1.py", "b2.py", "b3.py", "b4.py", "b5.py", "b6.py"]}
{"type": "attempt", "summary": "tried a longer timeout with a broken clock", "timestamp": "garbage", "id": "evt_a06", "outcome": "failed", "location": "src/api/client.ts:7"}
{"type": "note", "summary": "the unparsable timestamp counts as fifteen days", "timestamp": "garbage", "id": "evt_n04", "files": ["src/auth/session.py"]}
{"type": "note", "summary": "naive Unicode note — with an em dash, ünïcödé words and a long tail that goes on for a while to need a cut here", "timestamp": "2027-01-11T08:00:00Z", "id": "evt_n05", "files": ["src/ünï/çödé.py"]}"#;

const MAP: &str = r"# Project map

- `src/auth/` session handling
- `src/billing/` ledger
plain line without marker
- src/api client wrappers
- `web/` front end
Notes: src/auth is the hot area
";

const MAP_UNWRITTEN: &str = r"# Project map

Not created yet
- `x/` y
";

const HEAVY_120: &str = r#"{"type": "attempt", "summary": "approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T07:00:00Z", "id": "evt_w0", "outcome": "failed", "location": "src/m0.py:1", "files": ["src/m0.py"]}
{"type": "attempt", "summary": "approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T06:00:00Z", "id": "evt_w1", "outcome": "failed", "location": "src/m1.py:1", "files": ["src/m1.py"]}
{"type": "attempt", "summary": "approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T05:00:00Z", "id": "evt_w2", "outcome": "failed", "location": "src/m2.py:1", "files": ["src/m2.py"]}
{"type": "attempt", "summary": "approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T04:00:00Z", "id": "evt_w3", "outcome": "failed", "location": "src/m3.py:1", "files": ["src/m3.py"]}
{"type": "attempt", "summary": "approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T03:00:00Z", "id": "evt_w4", "outcome": "failed", "location": "src/m4.py:1", "files": ["src/m4.py"]}
{"type": "attempt", "summary": "approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T02:00:00Z", "id": "evt_w5", "outcome": "failed", "location": "src/m5.py:1", "files": ["src/m5.py"]}
{"type": "attempt", "summary": "approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T01:00:00Z", "id": "evt_w6", "outcome": "failed", "location": "src/m6.py:1", "files": ["src/m6.py"]}
{"type": "attempt", "summary": "approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T00:00:00Z", "id": "evt_w7", "outcome": "failed", "location": "src/m7.py:1", "files": ["src/m7.py"]}
{"type": "decision", "summary": "heavy decision", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hd"}
{"type": "fix", "summary": "heavy fix", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hf", "files": ["src/m0.py"]}"#;

const HEAVY_330: &str = r#"{"type": "attempt", "summary": "approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T07:00:00Z", "id": "evt_w0", "outcome": "failed", "location": "src/m0.py:1", "files": ["src/m0.py"]}
{"type": "attempt", "summary": "approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T06:00:00Z", "id": "evt_w1", "outcome": "failed", "location": "src/m1.py:1", "files": ["src/m1.py"]}
{"type": "attempt", "summary": "approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T05:00:00Z", "id": "evt_w2", "outcome": "failed", "location": "src/m2.py:1", "files": ["src/m2.py"]}
{"type": "attempt", "summary": "approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T04:00:00Z", "id": "evt_w3", "outcome": "failed", "location": "src/m3.py:1", "files": ["src/m3.py"]}
{"type": "attempt", "summary": "approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T03:00:00Z", "id": "evt_w4", "outcome": "failed", "location": "src/m4.py:1", "files": ["src/m4.py"]}
{"type": "attempt", "summary": "approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T02:00:00Z", "id": "evt_w5", "outcome": "failed", "location": "src/m5.py:1", "files": ["src/m5.py"]}
{"type": "attempt", "summary": "approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T01:00:00Z", "id": "evt_w6", "outcome": "failed", "location": "src/m6.py:1", "files": ["src/m6.py"]}
{"type": "attempt", "summary": "approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T00:00:00Z", "id": "evt_w7", "outcome": "failed", "location": "src/m7.py:1", "files": ["src/m7.py"]}
{"type": "decision", "summary": "heavy decision", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hd"}
{"type": "fix", "summary": "heavy fix", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hf", "files": ["src/m0.py"]}"#;

const HEAVY_390: &str = r#"{"type": "attempt", "summary": "approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T07:00:00Z", "id": "evt_w0", "outcome": "failed", "location": "src/m0.py:1", "files": ["src/m0.py"]}
{"type": "attempt", "summary": "approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T06:00:00Z", "id": "evt_w1", "outcome": "failed", "location": "src/m1.py:1", "files": ["src/m1.py"]}
{"type": "attempt", "summary": "approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T05:00:00Z", "id": "evt_w2", "outcome": "failed", "location": "src/m2.py:1", "files": ["src/m2.py"]}
{"type": "attempt", "summary": "approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T04:00:00Z", "id": "evt_w3", "outcome": "failed", "location": "src/m3.py:1", "files": ["src/m3.py"]}
{"type": "attempt", "summary": "approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T03:00:00Z", "id": "evt_w4", "outcome": "failed", "location": "src/m4.py:1", "files": ["src/m4.py"]}
{"type": "attempt", "summary": "approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T02:00:00Z", "id": "evt_w5", "outcome": "failed", "location": "src/m5.py:1", "files": ["src/m5.py"]}
{"type": "attempt", "summary": "approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T01:00:00Z", "id": "evt_w6", "outcome": "failed", "location": "src/m6.py:1", "files": ["src/m6.py"]}
{"type": "attempt", "summary": "approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T00:00:00Z", "id": "evt_w7", "outcome": "failed", "location": "src/m7.py:1", "files": ["src/m7.py"]}
{"type": "decision", "summary": "heavy decision", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hd"}
{"type": "fix", "summary": "heavy fix", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hf", "files": ["src/m0.py"]}"#;

const HEAVY_440: &str = r#"{"type": "attempt", "summary": "approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T07:00:00Z", "id": "evt_w0", "outcome": "failed", "location": "src/m0.py:1", "files": ["src/m0.py"]}
{"type": "attempt", "summary": "approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T06:00:00Z", "id": "evt_w1", "outcome": "failed", "location": "src/m1.py:1", "files": ["src/m1.py"]}
{"type": "attempt", "summary": "approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T05:00:00Z", "id": "evt_w2", "outcome": "failed", "location": "src/m2.py:1", "files": ["src/m2.py"]}
{"type": "attempt", "summary": "approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T04:00:00Z", "id": "evt_w3", "outcome": "failed", "location": "src/m3.py:1", "files": ["src/m3.py"]}
{"type": "attempt", "summary": "approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T03:00:00Z", "id": "evt_w4", "outcome": "failed", "location": "src/m4.py:1", "files": ["src/m4.py"]}
{"type": "attempt", "summary": "approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T02:00:00Z", "id": "evt_w5", "outcome": "failed", "location": "src/m5.py:1", "files": ["src/m5.py"]}
{"type": "attempt", "summary": "approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T01:00:00Z", "id": "evt_w6", "outcome": "failed", "location": "src/m6.py:1", "files": ["src/m6.py"]}
{"type": "attempt", "summary": "approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx", "timestamp": "2027-01-15T00:00:00Z", "id": "evt_w7", "outcome": "failed", "location": "src/m7.py:1", "files": ["src/m7.py"]}
{"type": "decision", "summary": "heavy decision", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hd"}
{"type": "fix", "summary": "heavy fix", "timestamp": "2027-01-14T08:00:00Z", "id": "evt_hf", "files": ["src/m0.py"]}"#;

const FULL: &str = r"## dejafix context (budget: 5000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const FULL_EDGE: &str = r"## dejafix context (budget: 4000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const COMPRESSED_TOP: &str = r"## dejafix context (budget: 3999 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const COMPRESSED: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const COMPRESSED_EDGE: &str = r"## dejafix context (budget: 1000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const ULTRA_TOP: &str = r"## dejafix context (budget: 999 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the … [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is … (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced … (5h ago)
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …
";

const ULTRA: &str = r"## dejafix context (budget: 500 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the … [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is … (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced … (5h ago)
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …
";

const ULTRA_EDGE: &str = r"## dejafix context (budget: 200 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the … [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is … (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced … (5h ago)
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
";

const EMERGENCY_TOP: &str = r"## dejafix context (budget: 199 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under … (2h ago)
- FAILED: rounded cents per line item (3d ago)
- FAILED: tried a longer timeout with a broken clock (unknown)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
- Fixed: Bumped the pool size (2w ago)
";

const EMERGENCY: &str = r"## dejafix context (budget: 100 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under … (2h ago)
- FAILED: rounded cents per line item (3d ago)
- FAILED: tried a longer timeout with a broken clock (unknown)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
";

const FOCUS_PREFIX: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/auth/clock.py`: HYPOTHESIS: clock skew between pods

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
Notes: src/auth is the hot area
";

const FOCUS_SUBSTRING: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const FOCUS_SAME_DIR: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const FOCUS_SLASH: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)
- Use sqlite for the local cache (2d ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
# Project map

- `src/auth/` session handling
- `src/billing/` ledger
plain line without marker
- src/api client wrappers
- `web/` front end
Notes: src/auth is the hot area
";

const GIT_BOOST: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)
- Use sqlite for the local cache (2d ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `tools/import.py`: DECISION: Run the importer nightly, never during the day, because the …
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const NO_MAP: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …
";

const NOT_CREATED: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …
";

const EMPTY_FOCUS: &str = r"## dejafix context (budget: 2000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: wrapped the refresh in a global lock. It deadlocked under load because the callback re-entered the lock and the worker pool starved. [src/auth/session.py:88] (2h ago)
- FAILED: rounded cents per line item [src/billing/ledger.py:12] (3d ago)
- FAILED: tried a longer timeout with a broken clock [src/api/client.ts:7] (unknown)
- FAILED: pinned the clock skew to a constant, which hid the drift in staging for a week before anyone noticed [src/auth/clock.py] (3mo ago)

### Recent Decisions
- Use sqlite for the local cache (2d ago)
- Run the importer nightly, never during the day, because the lock table is shared with billing (1w ago)

### Relevant Fixes
- Fixed: Fixed the double submit by disabling the button on the first click; reproduced with the e2e suite (5h ago)
  Files: web/form.ts, web/button.ts, web/test.ts
- Fixed: Bumped the pool size (2w ago)

### File Gotchas
- `src/auth/session.py`: ATTEMPT (failed): wrapped the refresh in a global lock. It deadlocked under …
- `src/billing/ledger.py`: ATTEMPT (failed): rounded cents per line item
- `src/api/client.ts`: ATTEMPT (failed): tried a longer timeout with a broken clock
- `web/form.ts`: FIX: Fixed the double submit by disabling the button on the …
- `web/button.ts`: FIX: Fixed the double submit by disabling the button on the …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const W120: &str = r"## dejafix context (budget: 1000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m0.py:1] (1h ago)
- FAILED: approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m1.py:1] (2h ago)
- FAILED: approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m2.py:1] (3h ago)
- FAILED: approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m3.py:1] (4h ago)
- FAILED: approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m4.py:1] (5h ago)
- FAILED: approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m5.py:1] (6h ago)
- FAILED: approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m6.py:1] (7h ago)
- FAILED: approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m7.py:1] (8h ago)

### Recent Decisions
- heavy decision (yesterday)

### Relevant Fixes
- Fixed: heavy fix (yesterday)
  Files: src/m0.py

### File Gotchas
- `src/m0.py`: ATTEMPT (failed): approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m1.py`: ATTEMPT (failed): approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m2.py`: ATTEMPT (failed): approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m3.py`: ATTEMPT (failed): approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m4.py`: ATTEMPT (failed): approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const W330: &str = r"## dejafix context (budget: 1000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m0.py:1] (1h ago)
- FAILED: approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m1.py:1] (2h ago)
- FAILED: approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m2.py:1] (3h ago)
- FAILED: approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m3.py:1] (4h ago)
- FAILED: approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m4.py:1] (5h ago)
- FAILED: approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m5.py:1] (6h ago)
- FAILED: approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m6.py:1] (7h ago)
- FAILED: approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m7.py:1] (8h ago)

### Relevant Fixes
- Fixed: heavy fix (yesterday)
  Files: src/m0.py

### File Gotchas
- `src/m0.py`: ATTEMPT (failed): approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m1.py`: ATTEMPT (failed): approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m2.py`: ATTEMPT (failed): approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m3.py`: ATTEMPT (failed): approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m4.py`: ATTEMPT (failed): approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …

### Architecture Context
- `src/auth/` session handling
- `src/billing/` ledger
- src/api client wrappers
- `web/` front end
";

const W390: &str = r"## dejafix context (budget: 1000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m0.py:1] (1h ago)
- FAILED: approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m1.py:1] (2h ago)
- FAILED: approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m2.py:1] (3h ago)
- FAILED: approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m3.py:1] (4h ago)
- FAILED: approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m4.py:1] (5h ago)
- FAILED: approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m5.py:1] (6h ago)
- FAILED: approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m6.py:1] (7h ago)
- FAILED: approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m7.py:1] (8h ago)

### File Gotchas
- `src/m0.py`: ATTEMPT (failed): approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m1.py`: ATTEMPT (failed): approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m2.py`: ATTEMPT (failed): approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m3.py`: ATTEMPT (failed): approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
- `src/m4.py`: ATTEMPT (failed): approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx …
";

const W440: &str = r"## dejafix context (budget: 1000 tokens)

### WARNINGS — Do Not Repeat
- FAILED: approach 0 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m0.py:1] (1h ago)
- FAILED: approach 1 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m1.py:1] (2h ago)
- FAILED: approach 2 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m2.py:1] (3h ago)
- FAILED: approach 3 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m3.py:1] (4h ago)
- FAILED: approach 4 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m4.py:1] (5h ago)
- FAILED: approach 5 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m5.py:1] (6h ago)
- FAILED: approach 6 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m6.py:1] (7h ago)
- FAILED: approach 7 xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx [src/m7.py:1] (8h ago)
";

#[test]
fn python_full() {
    assert_eq!(run(MAIN, 5000, None, &[], Some(MAP)), FULL);
}

#[test]
fn python_full_edge() {
    assert_eq!(run(MAIN, 4000, None, &[], Some(MAP)), FULL_EDGE);
}

#[test]
fn python_compressed_top() {
    assert_eq!(run(MAIN, 3999, None, &[], Some(MAP)), COMPRESSED_TOP);
}

#[test]
fn python_compressed() {
    assert_eq!(run(MAIN, 2000, None, &[], Some(MAP)), COMPRESSED);
}

#[test]
fn python_compressed_edge() {
    assert_eq!(run(MAIN, 1000, None, &[], Some(MAP)), COMPRESSED_EDGE);
}

#[test]
fn python_ultra_top() {
    assert_eq!(run(MAIN, 999, None, &[], Some(MAP)), ULTRA_TOP);
}

#[test]
fn python_ultra() {
    assert_eq!(run(MAIN, 500, None, &[], Some(MAP)), ULTRA);
}

#[test]
fn python_ultra_edge() {
    assert_eq!(run(MAIN, 200, None, &[], Some(MAP)), ULTRA_EDGE);
}

#[test]
fn python_emergency_top() {
    assert_eq!(run(MAIN, 199, None, &[], Some(MAP)), EMERGENCY_TOP);
}

#[test]
fn python_emergency() {
    assert_eq!(run(MAIN, 100, None, &[], Some(MAP)), EMERGENCY);
}

#[test]
fn python_focus_prefix() {
    assert_eq!(
        run(MAIN, 2000, Some("src/auth/"), &[], Some(MAP)),
        FOCUS_PREFIX
    );
}

#[test]
fn python_focus_substring() {
    assert_eq!(
        run(MAIN, 2000, Some("ledger"), &[], Some(MAP)),
        FOCUS_SUBSTRING
    );
}

#[test]
fn python_focus_same_dir() {
    assert_eq!(
        run(MAIN, 2000, Some("src/billing//"), &[], Some(MAP)),
        FOCUS_SAME_DIR
    );
}

#[test]
fn python_focus_slash() {
    assert_eq!(run(MAIN, 2000, Some("/"), &[], Some(MAP)), FOCUS_SLASH);
}

#[test]
fn python_git_boost() {
    assert_eq!(
        run(MAIN, 2000, None, &["tools/import.py"], Some(MAP)),
        GIT_BOOST
    );
}

#[test]
fn python_no_map() {
    assert_eq!(run(MAIN, 2000, None, &[], None), NO_MAP);
}

#[test]
fn python_not_created() {
    assert_eq!(run(MAIN, 2000, None, &[], Some(MAP_UNWRITTEN)), NOT_CREATED);
}

#[test]
fn python_empty_focus() {
    assert_eq!(run(MAIN, 2000, Some(""), &[], Some(MAP)), EMPTY_FOCUS);
}

#[test]
fn python_w120() {
    assert_eq!(run(HEAVY_120, 1000, None, &[], Some(MAP)), W120);
}

#[test]
fn python_w330() {
    assert_eq!(run(HEAVY_330, 1000, None, &[], Some(MAP)), W330);
}

#[test]
fn python_w390() {
    assert_eq!(run(HEAVY_390, 1000, None, &[], Some(MAP)), W390);
}

#[test]
fn python_w440() {
    assert_eq!(run(HEAVY_440, 1000, None, &[], Some(MAP)), W440);
}

#[test]
fn truncate_sentence() {
    assert_eq!(
        smart_truncate(
            r"The first sentence is long enough to clear the floor easily. The second sentence runs on and on until the limit cuts it.",
            80
        ),
        r"The first sentence is long enough to clear the floor easily. …"
    );
}

#[test]
fn truncate_exclaim() {
    assert_eq!(
        smart_truncate(
            r"Never call the importer during billing hours at all! It holds the shared lock table for several minutes at a time ok.",
            90
        ),
        r"Never call the importer during billing hours at all! …"
    );
}

#[test]
fn truncate_question() {
    assert_eq!(
        smart_truncate(
            r"Why does the refresh deadlock under load every single time? Nobody knows yet and the pool just starves quietly anyway.",
            85
        ),
        r"Why does the refresh deadlock under load every single time? …"
    );
}

#[test]
fn truncate_early_sentence() {
    assert_eq!(
        smart_truncate(
            r"Short one. then a very long run of words without any further sentence ends that goes past the limit for sure",
            60
        ),
        r"Short one. then a very long run of words without any …"
    );
}

#[test]
fn truncate_word() {
    assert_eq!(
        smart_truncate(
            r"restart the worker pool before retrying the refresh because stale sockets linger in the pool forever",
            50
        ),
        r"restart the worker pool before retrying the …"
    );
}

#[test]
fn truncate_word_comma() {
    assert_eq!(
        smart_truncate(
            r"restart the worker pool before retrying, then wait for it",
            42
        ),
        r"restart the worker pool before retrying …"
    );
}

#[test]
fn truncate_word_dash() {
    assert_eq!(
        smart_truncate(
            r"restart the worker pool before retrying — then wait for it",
            43
        ),
        r"restart the worker pool before retrying  …"
    );
}

#[test]
fn truncate_hard() {
    assert_eq!(
        smart_truncate(
            r"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            30
        ),
        r"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx…"
    );
}

#[test]
fn truncate_unchanged() {
    assert_eq!(smart_truncate(r"short text", 60), r"short text");
}

#[test]
fn truncate_exact() {
    assert_eq!(
        smart_truncate(
            r"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            60
        ),
        r"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn truncate_unicode() {
    assert_eq!(
        smart_truncate(
            r"ünïcödé wörds with — ümlauts and ççç long tail that needs a cut so that it ends up truncated somewhere",
            55
        ),
        r"ünïcödé wörds with — ümlauts and ççç long tail that …"
    );
}

#[test]
fn truncate_hard_space() {
    assert_eq!(
        smart_truncate(
            r"aaaaaaaaaa bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            40
        ),
        r"aaaaaaaaaa bbbbbbbbbbbbbbbbbbbbbbbbbbbbb…"
    );
}
