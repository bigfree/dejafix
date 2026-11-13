use super::{MAP_POINTER, SESSION_START_DONE, current_plan, placeholder_map, session_start};

const PLAN: &str = "# demo — plan\n\n## Ideas\n- maybe a cache\n\n## Active plans\n- [ ] ship the exporter\n\n## Next\n- review the exporter\n\n## Someday / maybe\n- rewrite in rust\n\n## Shipped\n- v1.0 released\n";
const RULES: &str = "# rules\n\nLog before you fix.";
const SUMMARY: &str = "# summary\n\n- [OPEN] #0001 boom";

fn start(plan: &str, map: &str) -> String {
    session_start(RULES, SUMMARY, plan.trim(), map)
}

#[test]
fn start_session_contains_rules_summary_and_active_plan() {
    let text = start(PLAN, "");
    for wanted in [
        "Log before you fix.",
        "#0001 boom",
        "ship the exporter",
        "review the exporter",
        "get_project_map()",
    ] {
        assert!(text.contains(wanted), "{wanted}");
    }
}

#[test]
fn start_session_omits_ideas_shipped_and_someday() {
    let text = start(PLAN, "");
    for unwanted in ["maybe a cache", "rewrite in rust", "v1.0 released"] {
        assert!(!text.contains(unwanted), "{unwanted}");
    }
}

#[test]
fn start_session_tolerates_missing_files() {
    assert_eq!(
        session_start("", "", "", ""),
        format!("{SESSION_START_DONE}\n\n{MAP_POINTER}")
    );
}

#[test]
fn start_session_keeps_custom_plan_headings() {
    let text = start(
        "# demo\n\n## In progress\n- [ ] migrate the queue\n\n## Shipped\n- v1\n",
        "",
    );
    assert!(text.contains("migrate the queue"));
    assert!(!text.contains("- v1"));
}

#[test]
fn start_session_reads_a_plan_that_starts_with_a_heading() {
    assert_eq!(
        current_plan("## Active plans\n- [ ] first item"),
        "## Active plans\n- [ ] first item"
    );
}

#[test]
fn start_session_survives_an_empty_trailing_heading() {
    assert_eq!(
        current_plan("# demo\n## \n## Next\n- review"),
        "## Next\n- review"
    );
}

#[test]
fn start_session_says_the_session_start_reads_are_done() {
    let text = start(PLAN, "");
    let head = text.split("Log before you fix.").next().unwrap_or_default();
    assert!(head.contains("already") && head.contains("get_summary"));
}

#[test]
fn start_session_includes_a_placeholder_map_for_setup_mode() {
    let map = "# Project map\n\nStatus: not created yet";
    assert!(start(PLAN, map).contains("Status: not created yet"));
    assert_eq!(placeholder_map(map), map);
}

#[test]
fn start_session_leaves_out_a_real_map() {
    let map = "# Project map\n\n## Structure\n- src/app.py entry point";
    assert!(!start(PLAN, map).contains("src/app.py entry point"));
    assert_eq!(placeholder_map(""), "");
}

#[test]
fn the_plan_keeps_sections_in_order_joined_by_a_blank_line() {
    assert_eq!(
        current_plan(PLAN.trim()),
        "## Active plans\n- [ ] ship the exporter\n\n## Next\n- review the exporter"
    );
}
