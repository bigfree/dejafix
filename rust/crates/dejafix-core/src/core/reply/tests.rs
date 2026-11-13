use std::path::{Path, PathBuf};

use serde_json::json;

use super::{current_reply, gotchas_reply, precheck_report, projects_reply};
use crate::core::global::Entry;
use crate::core::path::PathContext;
use crate::core::precheck::{Severity, Warning, WarningKind};
use crate::core::registry::Registry;
use crate::core::resolve::{Resolution, Source};
use crate::testing::NoResolve;

fn context() -> PathContext {
    PathContext {
        root: "/work/proj".into(),
        cwd: "/work/proj".into(),
        fold_case: false,
    }
}

fn warning(file: &str, severity: Severity, title: &str, details: &[&str]) -> Warning {
    Warning {
        file: file.to_owned(),
        severity,
        kind: WarningKind::FailedAttempts,
        title: title.to_owned(),
        details: details.iter().map(|detail| (*detail).to_owned()).collect(),
    }
}

#[test]
fn precheck_report_has_one_block_per_path_as_given() {
    let warnings = [
        warning(
            "src/auth.py",
            Severity::Block,
            "3 failed attempts",
            &["tried a", "tried b"],
        ),
        warning("src/auth.py", Severity::Info, "decision", &[]),
    ];
    let paths = [
        " ./src/auth.py".to_owned(),
        "/work/proj/src/auth.py".to_owned(),
        "src/other.py".to_owned(),
    ];
    let block = "dejafix precheck: {}\n  [BLOCK] 3 failed attempts\n    - tried a\n    - tried b\n  [INFO] decision";
    assert_eq!(
        precheck_report(&paths, &warnings, &context(), &NoResolve),
        [
            block.replace("{}", " ./src/auth.py"),
            block.replace("{}", "/work/proj/src/auth.py"),
            "src/other.py: no warnings. Safe to modify.".to_owned(),
        ]
        .join("\n\n")
    );
}

#[test]
fn precheck_names_a_memory_file_instead_of_clearing_it() {
    let paths = [".dejafix/summary.md".to_owned()];
    assert_eq!(
        precheck_report(&paths, &[], &context(), &NoResolve),
        ".dejafix/summary.md: this is a dejafix memory file. It is regenerated from the event log, so do not edit it; record the change with add_note / add_decision / log_issue / record_fix instead."
    );
}

fn entries(values: &[serde_json::Value]) -> Vec<Entry> {
    values
        .iter()
        .filter_map(|value| value.as_object().cloned())
        .collect()
}

#[test]
fn gotchas_are_filtered_by_a_library_substring() {
    let gotchas = entries(&[
        json!({"library": "react-router", "gotcha": "loaders run twice", "source_project": "web"}),
        json!({"library": "bcrypt", "gotcha": "truncates at 72 bytes", "source_project": ""}),
        json!({"gotcha": "no library", "source_project": null}),
        json!({"library": null, "gotcha": 5}),
    ]);
    assert_eq!(
        gotchas_reply(&gotchas, Some("React")),
        "Global gotchas (1):\n  [react-router] loaders run twice (from web)"
    );
    assert_eq!(
        gotchas_reply(&gotchas, None),
        "Global gotchas (4):\n  [react-router] loaders run twice (from web)\n  [bcrypt] truncates at 72 bytes\n  [unknown] no library\n  [None] 5"
    );
    assert_eq!(
        gotchas_reply(&gotchas, Some("vue")),
        "No global gotchas found for vue."
    );
    assert_eq!(gotchas_reply(&[], Some("")), "No global gotchas found.");
}

#[test]
fn only_fifteen_gotchas_are_shown_but_all_are_counted() {
    let gotchas: Vec<Entry> = (0..20)
        .filter_map(|index| {
            json!({"library": "x", "gotcha": format!("g{index}")})
                .as_object()
                .cloned()
        })
        .collect();
    let reply = gotchas_reply(&gotchas, None);
    assert!(reply.starts_with("Global gotchas (20):"));
    assert_eq!(reply.lines().count(), 16);
}

fn registry(active: Option<&str>) -> Registry {
    let mut registry = Registry::default();
    for name in ["alpha", "beta", "gamma"] {
        let _ = registry.register(
            PathBuf::from(format!("/work/{name}")),
            None,
            "2026-01-01T00:00:00Z",
        );
    }
    if let Some(record) = registry.projects.get_mut(2) {
        record.alias = Some("g".to_owned());
    }
    registry.active_project = active.map(str::to_owned);
    registry
}

fn lookup(text: &str) -> Option<PathBuf> {
    Path::new(text).is_absolute().then(|| PathBuf::from(text))
}

#[test]
fn list_projects_leads_with_the_active_project() {
    assert_eq!(
        projects_reply(&registry(Some("beta")), None, lookup),
        "ACTIVE: beta — /work/beta\nCalls that omit `project` go here. You do not need to pass it.\n\n3 project(s) — pass one as `project`:\n  alpha — /work/alpha\n  beta (active) — /work/beta\n  gamma / g — /work/gamma"
    );
}

#[test]
fn list_projects_without_an_active_project_says_calls_will_fail() {
    assert_eq!(
        projects_reply(&registry(None), None, lookup),
        "3 project(s) — pass one as `project`:\n  alpha — /work/alpha\n  beta — /work/beta\n  gamma / g — /work/gamma\n\nNo active project: calls that omit `project` will fail rather than guess. Set one with `djf project use <name>`."
    );
    assert_eq!(
        projects_reply(&Registry::default(), None, lookup),
        "No projects registered. Run `djf init` in a repo (it registers automatically), or `djf project register <path>`."
    );
    assert_eq!(
        projects_reply(&registry(None), Some(Path::new("/work/alpha")), lookup),
        "This server is pinned to a single project: alpha (/work/alpha).\nThe `project` parameter is ignored."
    );
}

#[test]
fn current_project_names_the_source() {
    let resolution = Resolution {
        root: PathBuf::from("/work/beta"),
        source: Source::Active,
        name: "beta".to_owned(),
    };
    assert_eq!(
        current_reply(&resolution),
        "beta — /work/beta\nchosen by: active"
    );
}
