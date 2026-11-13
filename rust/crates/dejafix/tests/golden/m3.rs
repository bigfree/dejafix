use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use dejafix_core::testing::{FIXTURE_SEED, write_project};
use regex::Regex;
use serde_json::{Value, json};

use crate::harness::{diff, fixed, run_actions};
use crate::scenario::canonical;
use crate::types::{Action, Implementation, Run};

type TestResult = Result<(), Box<dyn Error>>;
type Prepared = Result<Vec<String>, Box<dyn Error>>;
type Cleaned = Result<Run, Box<dyn Error>>;

const INIT: &[&str] = &[
    "init",
    "--no-hooks",
    "--no-mcp-config",
    "--no-structure",
    "--no-backfill",
    "--no-watch",
];
const INIT_HOOKS: &[&str] = &[
    "init",
    "--no-mcp-config",
    "--no-structure",
    "--no-backfill",
    "--no-watch",
];
const EPOCH: &str = "2026-01-01T09:00:00Z";
const OLD_PRE_COMMIT: &str = "# >>> dejafix auto-capture >>>\n\
# Pre-commit warning check against project memory.\n\
# Installed by: djf hooks install (or djf init)\n\
# Remove with:  djf hooks uninstall\n\
# Bypass once:  git commit --no-verify\n\
DJF_BIN=\"/opt/anaconda3/bin/djf\"\n\
if [ ! -x \"$DJF_BIN\" ]; then\n\
\x20   DJF_BIN=\"$(command -v djf 2>/dev/null || command -v dejafix 2>/dev/null)\"\n\
fi\n\
if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n\
\x20   \"$DJF_BIN\" precheck --level warn || true\n\
fi\n\
# <<< dejafix auto-capture <<<\n";
const BROKEN_POST_MERGE: &str = "#!/usr/bin/env bash\n\
# >>> dejafix auto-capture >>>\n\
# Automatically captures development events into dejafix.\n\
DJF_BIN=\"/opt/anaconda3/bin/djf\"\n\
if [ -d \".dejafix\" ] && [ -n \"$DJF_BIN\" ]; then\n\
\x20   \"$DJF_BIN\" _auto-capture \"merge\" >/dev/null 2>&1 &\n\
fi\n";

pub(crate) fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_owned()).collect()
}

pub(crate) fn djf(list: &'static [&'static str]) -> Action<'static> {
    Action::Djf(fixed(list))
}

pub(crate) fn git(dir: &Path, list: &[&str], date: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(list)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {list:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

pub(crate) fn repository(dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    git(dir, &["init", "-q", "-b", "main"], EPOCH)?;
    Ok(())
}

pub(crate) fn write(root: &Path, relative: &str, text: &str) -> Result<(), Box<dyn Error>> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}

pub(crate) fn commit(
    project: &Path,
    files: &[(String, String)],
    message: &str,
    date: &str,
) -> Result<(), Box<dyn Error>> {
    let mut add = vec!["add", "--"];
    for (path, text) in files {
        write(project, path, text)?;
        add.push(path);
    }
    git(project, &add, date)?;
    git(
        project,
        &["commit", "--no-verify", "-q", "-m", message],
        date,
    )?;
    Ok(())
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

pub(crate) fn compare(
    fixture: &Path,
    actions: &[Action<'_>],
    clean: fn(Run) -> Cleaned,
) -> Result<(Run, Vec<String>), Box<dyn Error>> {
    let python = clean(canonical(run_actions(
        Implementation::Python,
        fixture,
        None,
        actions,
    )?))?;
    let rust = clean(canonical(run_actions(
        Implementation::Rust,
        fixture,
        None,
        actions,
    )?))?;
    let differences = diff(&python, &rust);
    Ok((python, differences))
}

const DIAGNOSES: &[(&str, &str)] = &[(
    "home/.dejafix/global/library_gotchas.jsonl: only on the right",
    "Rust bug: every attempt, decision or note write creates an empty global \
     library_gotchas.jsonl, because GlobalStore::append_gotcha_if_new \
     (dejafix-core/src/adapters/fs/global.rs) opens the file with create(true) \
     before `decide` runs; Python's auto_promote_event writes the file only when \
     a gotcha is promoted",
)];

pub(crate) fn assert_same(scenario: &str, differences: &[String]) {
    let explained: Vec<String> = differences
        .iter()
        .map(|line| {
            DIAGNOSES
                .iter()
                .find(|(known, _)| line == known)
                .map_or_else(|| line.clone(), |(_, why)| format!("{line}\n  -> {why}"))
        })
        .collect();
    assert!(
        differences.is_empty(),
        "{scenario}: Rust differs from Python (left = Python, right = Rust)\n{}",
        explained.join("\n")
    );
}

fn stdout_of(run: &Run, index: usize) -> &str {
    run.steps.get(index).map_or("", |step| step.stdout.as_str())
}

fn rewrite_hooks(project: &Path) -> Prepared {
    let hooks = project.join(".git/hooks");
    fs::write(
        hooks.join("pre-commit"),
        format!(
            "#!/usr/bin/env bash\nset -e\necho 'lint first'\n\n{OLD_PRE_COMMIT}\n# then tests\n"
        ),
    )?;
    fs::write(hooks.join("post-merge"), BROKEN_POST_MERGE)?;
    Ok(args(&["hooks", "install"]))
}

#[test]
fn hooks_match_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let repo = fixture.path().join("repo");
    let fresh = fixture.path().join("fresh");
    let plain = fixture.path().join("plain");
    repository(&repo)?;
    repository(&fresh)?;
    fs::create_dir_all(&plain)?;
    let actions = vec![
        djf(INIT),
        djf(&["hooks", "install"]),
        djf(&["hooks", "install"]),
        Action::Djf(Box::new(rewrite_hooks)),
        djf(&["hooks", "uninstall"]),
        djf(&["hooks", "uninstall"]),
        djf(&["hooks", "bogus"]),
        djf(&["hooks"]),
    ];
    let (python, differences) = compare(&repo, &actions, Ok)?;
    assert_same("hooks in a repository", &differences);
    assert!(stdout_of(&python, 1).contains("installed: post-commit, post-merge, pre-commit"));
    assert!(stdout_of(&python, 3).contains("refreshed: pre-commit"));
    assert!(
        python
            .files
            .contains_key("project/.git/hooks/pre-commit (mode)")
    );
    let actions = vec![djf(INIT_HOOKS), djf(&["hooks", "install"])];
    let (python, differences) = compare(&fresh, &actions, Ok)?;
    assert_same("init in a fresh repository", &differences);
    assert!(stdout_of(&python, 0).contains("Auto-capture active"));
    let actions = vec![
        djf(INIT_HOOKS),
        djf(&["hooks", "install"]),
        djf(&["hooks", "uninstall"]),
    ];
    let (python, differences) = compare(&plain, &actions, Ok)?;
    assert_same("init without .git", &differences);
    assert!(stdout_of(&python, 0).contains("No .git directory found"));
    Ok(())
}

fn capture(files: Vec<(String, String)>, message: String, date: &'static str) -> Action<'static> {
    Action::Djf(Box::new(move |project: &Path| {
        commit(project, &files, &message, date)?;
        Ok(args(&["_auto-capture", "commit"]))
    }))
}

fn rename(project: &Path) -> Prepared {
    let date = "2026-01-11T10:00:00Z";
    git(project, &["mv", "src/old.py", "src/renamed.py"], date)?;
    git(
        project,
        &[
            "commit",
            "--no-verify",
            "-q",
            "-m",
            "fix: rename the old module",
        ],
        date,
    )?;
    Ok(args(&["_auto-capture", "commit"]))
}

fn merge(project: &Path) -> Prepared {
    git(project, &["checkout", "-q", "-b", "topic"], EPOCH)?;
    commit(
        project,
        &pairs(&[("src/topic.py", "topic = 1\n")]),
        "feat: topic work",
        "2026-01-15T10:00:00Z",
    )?;
    git(project, &["checkout", "-q", "main"], EPOCH)?;
    commit(
        project,
        &pairs(&[("src/main_side.py", "side = 1\n")]),
        "fix: main side work",
        "2026-01-16T10:00:00Z",
    )?;
    git(
        project,
        &[
            "merge",
            "--no-ff",
            "-q",
            "--no-edit",
            "-m",
            "Merge branch 'topic'",
            "topic",
        ],
        "2026-01-17T10:00:00Z",
    )?;
    Ok(args(&["_auto-capture", "merge"]))
}

fn disable(project: &Path) -> Prepared {
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(project.join(".dejafix/config.toml"))?;
    config.write_all(b"auto_capture = false\n")?;
    commit(
        project,
        &pairs(&[("src/late.py", "late = 1\n")]),
        "fix: after the switch",
        "2026-01-19T10:00:00Z",
    )?;
    Ok(args(&["_auto-capture", "commit"]))
}

fn capture_actions() -> Vec<Action<'static>> {
    let twelve: Vec<(String, String)> = (1..=12)
        .map(|n| (format!(".dejafix/issues/90{n:02}-x.md"), format!("# {n}\n")))
        .chain([("src/app.py".to_owned(), "app = 3\n".to_owned())])
        .collect();
    vec![
        capture(
            pairs(&[
                ("src/app.py", "app = 1\n"),
                ("README.md", "# demo\n"),
                ("src/old.py", "old = 1\nold = 2\nold = 3\n"),
                ("tests/test_app.py", "def test(): pass\n"),
            ]),
            "feat: scaffold the app".into(),
            "2026-01-02T10:00:00Z",
        ),
        capture(
            pairs(&[("src/app.py", "app = 0\n")]),
            "Revert \"x\"\n\nThis reverts commit 0123456.".into(),
            "2026-01-03T10:00:00Z",
        ),
        capture(
            pairs(&[
                ("Dockerfile", "FROM scratch\n"),
                ("README.md", "# demo cart\n"),
                ("src/cart.py", "cart = 1\n"),
            ]),
            "fix: guard the empty cart".into(),
            "2026-01-04T10:00:00Z",
        ),
        capture(
            pairs(&[("src/compat.py", "compat = 2\n")]),
            "feat!: drop the python 2 shim\n\nBREAKING CHANGE: the shim is gone".into(),
            "2026-01-05T10:00:00Z",
        ),
        capture(
            pairs(&[
                ("tests/test_export.py", "def test(): pass\n"),
                ("src/export.py", "export = 1\n"),
            ]),
            "feat: add the csv export".into(),
            "2026-01-06T10:00:00Z",
        ),
        capture(
            pairs(&[("src/a.py", "a = 1\n"), ("src/b.py", "b = 1\n")]),
            "refactor: split the modules".into(),
            "2026-01-07T10:00:00Z",
        ),
        djf(&["_auto-capture", "commit"]),
        capture(
            pairs(&[("docs/setup.md", "# Setup\n")]),
            "docs: explain the setup".into(),
            "2026-01-08T10:00:00Z",
        ),
        capture(
            pairs(&[("package-lock.json", "{}\n")]),
            "chore: bump the dependencies".into(),
            "2026-01-09T10:00:00Z",
        ),
        Action::Djf(Box::new(rename)),
        capture(
            pairs(&[("src/café.py", "accent = 1\n")]),
            "fix: handle accented names".into(),
            "2026-01-12T10:00:00Z",
        ),
        capture(
            pairs(&[(".dejafix/summary.md", "# Fixed summary\n")]),
            "fix: refresh the memory".into(),
            "2026-01-13T10:00:00Z",
        ),
        capture(
            twelve,
            "fix: close twelve issues".into(),
            "2026-01-14T10:00:00Z",
        ),
        Action::Djf(Box::new(merge)),
        djf(&["_auto-capture", "commit"]),
        capture(
            pairs(&[("src/key.py", "key = None\n")]),
            format!("fix: rotate the leaked key sk-{}", "a".repeat(48)),
            "2026-01-18T10:00:00Z",
        ),
        Action::Djf(Box::new(disable)),
    ]
}

#[test]
fn auto_capture_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    write_project(fixture.path(), 0, FIXTURE_SEED)?;
    repository(fixture.path())?;
    let actions = capture_actions();
    let (python, differences) = compare(fixture.path(), &actions, Ok)?;
    let captured = python
        .steps
        .iter()
        .filter(|step| step.stdout.contains("[dejafix] Auto-captured"))
        .count();
    assert_eq!(captured, 12, "{:?}", python.steps);
    assert!(
        python
            .steps
            .iter()
            .any(|step| !step.stderr.is_empty() && step.stdout.contains("rotate")),
        "{:?}",
        python.steps
    );
    assert_same("auto-capture", &differences);
    Ok(())
}

pub(crate) fn days_ago(now: i64, days: i64) -> Result<String, Box<dyn Error>> {
    Ok(jiff::Timestamp::from_second(now - days * 86_400)?.to_string())
}

const TRACKED: [&str; 7] = [
    "src/app.py",
    "src/auth/session.py",
    "src/api/routes.ts",
    "web/components/Login.tsx",
    "crates/core/src/lib.rs",
    "docs/setup.md",
    "scripts/deploy.sh",
];

fn precheck_events(now: i64) -> Result<String, Box<dyn Error>> {
    let events = [
        json!({"id": "evt_e3a00000000000000001", "timestamp": days_ago(now, 20)?, "type": "decision",
            "summary": "Keep the app entry point thin", "location": "src/app.py:10"}),
        json!({"id": "evt_e3a00000000000000002", "timestamp": days_ago(now, 15)?, "type": "note",
            "summary": "The gone module keeps the legacy hooks", "location": "src/gone.py:3"}),
        json!({"id": "evt_e3a00000000000000003", "timestamp": days_ago(now, 3)?, "type": "attempt",
            "outcome": "failed", "summary": "Reverted: cache sessions in memory",
            "files": ["src/auth/session.py"], "auto_captured": true,
            "capture_source": "git_post_revert", "capture_confidence": "high",
            "git_message": "Revert \"cache sessions in memory\""}),
        json!({"id": "evt_e3a00000000000000004", "timestamp": days_ago(now, 1)?, "type": "decision",
            "summary": "Routes stay declarative", "files": ["src/api/routes.ts"]}),
    ];
    let mut text = String::new();
    for event in events {
        text.push_str(&event.to_string());
        text.push('\n');
    }
    Ok(text)
}

pub(crate) fn precheck_fixture(root: &Path, now: i64) -> Result<(), Box<dyn Error>> {
    write_project(root, 3_000, FIXTURE_SEED)?;
    let mut log = fs::OpenOptions::new()
        .append(true)
        .open(root.join(".dejafix/events.jsonl"))?;
    log.write_all(precheck_events(now)?.as_bytes())?;
    repository(root)?;
    let initial: Vec<(String, String)> = TRACKED
        .iter()
        .map(|path| ((*path).to_owned(), format!("v0 {path}\n")))
        .collect();
    commit(root, &initial, "chore: import", &days_ago(now, 12)?)?;
    let history: [(&str, i64); 9] = [
        ("src/app.py", 10),
        ("src/auth/session.py", 9),
        ("src/app.py", 8),
        ("src/auth/session.py", 7),
        ("src/app.py", 6),
        ("src/auth/session.py", 5),
        ("src/app.py", 4),
        ("web/components/Login.tsx", 3),
        ("src/app.py", 2),
    ];
    for (path, days) in history {
        let files = vec![(path.to_owned(), format!("v{days} {path}\n"))];
        commit(
            root,
            &files,
            &format!("chore: touch {path}"),
            &days_ago(now, days)?,
        )?;
    }
    Ok(())
}

fn staged(project: &Path) -> Prepared {
    write(project, "web/components/Login.tsx", "staged\n")?;
    git(project, &["add", "web/components/Login.tsx"], EPOCH)?;
    Ok(args(&["precheck"]))
}

fn working(project: &Path) -> Prepared {
    write(project, "scripts/deploy.sh", "working\n")?;
    Ok(args(&["precheck", "--working"]))
}

fn absolute(project: &Path) -> Vec<String> {
    vec![
        "precheck".to_owned(),
        project.join("src/app.py").display().to_string(),
        "./src/app.py".to_owned(),
    ]
}

pub(crate) fn clock(run: Run) -> Cleaned {
    let minutes = Regex::new(r"\d\d:\d\d UTC")?;
    let seconds = Regex::new(r"\d\d:\d\d:\d\d\+00:00")?;
    let clean = |text: &str| {
        let text = minutes.replace_all(text, "<HH:MM> UTC");
        seconds.replace_all(&text, "<HH:MM:SS>+00:00").into_owned()
    };
    Ok(Run {
        steps: run
            .steps
            .into_iter()
            .map(|mut step| {
                step.stdout = clean(&step.stdout);
                step.stderr = clean(&step.stderr);
                step
            })
            .collect(),
        files: run
            .files
            .into_iter()
            .map(|(path, text)| (path, clean(&text)))
            .collect(),
    })
}

#[test]
fn precheck_matches_python() -> TestResult {
    let now = jiff::Timestamp::now().as_second();
    let fixture = tempfile::tempdir()?;
    precheck_fixture(fixture.path(), now)?;
    let actions = vec![
        djf(&["precheck"]),
        djf(&["precheck", "src/app.py"]),
        djf(&["precheck", "src/auth/session.py", "src/api/routes.ts"]),
        djf(&["precheck", "src/gone.py", "crates/core/src/lib.rs"]),
        Action::Djf(Box::new(|project: &Path| Ok(absolute(project)))),
        Action::Djf(Box::new(staged)),
        Action::Djf(Box::new(working)),
        djf(&[
            "precheck",
            "--level",
            "info",
            "src/app.py",
            "src/api/routes.ts",
        ]),
        djf(&["precheck", "--level", "block", "src/app.py"]),
        djf(&["precheck", "-l", "block", "src/api/routes.ts"]),
        djf(&["precheck", "--quiet", "README.md"]),
        djf(&["precheck", "-q", "src/app.py"]),
        djf(&["precheck", ".dejafix/summary.md"]),
        djf(&["precheck", "."]),
        djf(&["precheck", "--snooze", "2h"]),
        djf(&["precheck"]),
        djf(&["precheck", "--unsnooze"]),
        djf(&["precheck", "--unsnooze"]),
        djf(&["precheck", "--snooze", "soon"]),
        djf(&["precheck", "--snooze", "0m"]),
    ];
    let (python, differences) = compare(fixture.path(), &actions, clock)?;
    for needle in [
        "HIGH CHURN",
        "possibly-stale memory",
        "cited file no longer exists",
        "Recent revert affected this file",
        "Recent decision affects this file",
        "What already failed here",
    ] {
        assert!(
            python.steps.iter().any(|step| step.stdout.contains(needle)),
            "{needle}: {:?}",
            python.steps
        );
    }
    assert_same("precheck", &differences);
    Ok(())
}

const EDIT_EVENTS: &[&str] = &[
    r#"{"id": "evt_e3b00000000000000001", "timestamp": "2026-03-01T10:00:00Z", "type": "issue", "issue_id": "0001", "summary": "Session cookie dropped on refresh – naïve façade 🚀", "location": "src/auth/session.py:42"}"#,
    r#"{"id": "evt_e3b00000000000000002", "timestamp": "2026-03-02T10:00:00Z", "type": "attempt", "issue_id": "0001", "outcome": "failed", "summary": "Set SameSite=None on the cookie", "files": ["src/auth/session.py"]}"#,
    r#"{"id": "evt_e3b00000000000000003", "timestamp": "2026-03-03T10:00:00Z", "type": "attempt", "issue_id": "0001", "outcome": "partial", "summary": "Refresh the cookie in middleware"}"#,
    r#"{"id": "evt_e3b00000000000000004", "timestamp": "2026-03-04T10:00:00Z", "type": "attempt", "issue_id": "0001", "outcome": "worked", "summary": "Store the session server-side", "files": ["src/auth/session.py"]}"#,
    r#"{"id": "evt_e3b00000000000000005", "timestamp": "2026-03-05T10:00:00Z", "type": "decision", "summary": "Sessions live in Redis", "files": ["src/auth/session.py"]}"#,
    r#"{"id": "evt_e3b00000000000000006", "timestamp": "2026-03-06T10:00:00Z", "type": "decision", "summary": "Sessions live in Postgres", "files": ["./src/auth/session.py"]}"#,
    r#"{"id": "evt_e3b00000000000000007", "timestamp": "2026-03-07T10:00:00Z", "type": "decision", "summary": "Sessions live in Postgres, cached in Redis", "files": ["src/auth/session.py"], "supersedes": "evt_e3b00000000000000006"}"#,
    r#"{"id": "evt_e3b00000000000000008", "timestamp": "2026-03-08T10:00:00Z", "type": "decision", "summary": "Keep src/app.py free of business logic"}"#,
    r#"{"id": "evt_e3b00000000000000009", "timestamp": "2026-03-09T10:00:00Z", "type": "decision", "summary": "Refactor: split the app", "files": ["src/app.py"], "auto_captured": true, "capture_source": "git_post_commit"}"#,
    r#"{"id": "evt_e3b00000000000000010", "timestamp": "2026-03-10T10:00:00Z", "type": "issue", "issue_id": "0002", "summary": "Startup crash without an env file", "location": "src/app.py:10"}"#,
    r#"{"id": "evt_e3b00000000000000011", "timestamp": "2026-03-11T10:00:00Z", "type": "fix", "issue_id": "0002", "summary": "Load the defaults first", "files": ["src/app.py"], "capture_source": "git_commit"}"#,
    r#"{"id": "evt_e3b00000000000000012", "timestamp": "2026-03-12T10:00:00Z", "type": "issue", "issue_id": "0003", "summary": "Config reload races the watcher", "location": "./src/app.py:5"}"#,
    r#"{"id": "evt_e3b00000000000000013", "timestamp": "2026-03-13T10:00:00Z", "type": "attempt", "issue_id": "0003", "outcome": "failed", "summary": "Lock the   config\n  file during reload, which turned out to deadlock the watcher thread whenever two reloads arrived within the debounce window and the lock was held across the callback boundary", "files": ["src/app.py"]}"#,
    r#"{"id": "evt_e3b00000000000000014", "timestamp": "2026-03-14T10:00:00Z", "type": "attempt", "issue_id": "0003", "outcome": "failed", "summary": "Debounce reloads", "files": ["src/app.py:12"]}"#,
    r#"{"id": "evt_e3b00000000000000015", "timestamp": "2026-03-15T10:00:00Z", "type": "attempt", "issue_id": "bad id!", "outcome": "partial", "summary": "Queue reloads", "files": ["src/app.py"]}"#,
    r#"{"id": "evt_e3b00000000000000016", "timestamp": "2026-03-16T10:00:00Z", "type": "attempt", "issue_id": "0003", "outcome": "failed", "summary": "Reload in a subprocess", "files": ["src\\app.py"]}"#,
    r#"{"id": "evt_e3b00000000000000017", "timestamp": "2026-03-17T10:00:00Z", "type": "attempt", "issue_id": "0003", "outcome": "failed", "summary": "Poll instead of watching", "files": ["src/app.py"]}"#,
    "not json",
    "[1, 2]",
    r#"{"type": "attempt", "outcome": "failed", "summary": 7, "files": ["src/app.py"]}"#,
    r#"{"type": "issue", "issue_id": "0004", "summary": "Windows path issue", "location": "src\\auth\\session.py:12", "timestamp": 5}"#,
    r#"{"type": "decision", "summary": "Mentions src/app.pyc and xsrc/app.py but not the file"}"#,
];

fn edit_fixture(root: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join(".dejafix"))?;
    fs::write(root.join(".dejafix/config.toml"), "recent_days = 30\n")?;
    let mut log = String::from("\u{feff}");
    for line in EDIT_EVENTS {
        log.push_str(line);
        log.push('\n');
    }
    fs::write(root.join(".dejafix/events.jsonl"), log)?;
    write_project(&root.join("big"), 3_000, FIXTURE_SEED)?;
    Ok(())
}

fn located(project: &Path, file: &str) -> String {
    project.join(file).display().to_string()
}

fn claude_edit(project: &Path, session: Option<&str>, agent: Option<&str>, file: &str) -> String {
    let mut payload = serde_json::Map::new();
    payload.insert("transcript_path".into(), json!("/dev/null"));
    payload.insert("cwd".into(), json!(project.display().to_string()));
    payload.insert("hook_event_name".into(), json!("PreToolUse"));
    payload.insert("tool_name".into(), json!("Edit"));
    payload.insert(
        "tool_input".into(),
        json!({"file_path": located(project, file), "old_string": "a", "new_string": "b"}),
    );
    if let Some(session) = session {
        payload.insert("session_id".into(), json!(session));
    }
    if let Some(agent) = agent {
        payload.insert("agent_id".into(), json!(agent));
    }
    Value::Object(payload).to_string()
}

fn edit(
    session: Option<&'static str>,
    agent: Option<&'static str>,
    file: &'static str,
) -> Action<'static> {
    Action::EditHook(Box::new(move |project: &Path| {
        Ok(claude_edit(project, session, agent, file))
    }))
}

fn raw(payload: &'static str) -> Action<'static> {
    Action::EditHook(Box::new(move |_: &Path| Ok(payload.to_owned())))
}

fn lifecycle(event: &'static str, extra: Value) -> Action<'static> {
    Action::EditHook(Box::new(move |project: &Path| {
        let mut payload = json!({
            "session_id": "s1",
            "cwd": project.display().to_string(),
            "hook_event_name": event,
        });
        if let (Some(target), Some(source)) = (payload.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        Ok(payload.to_string())
    }))
}

fn first_edit(project: &Path) -> Result<String, Box<dyn Error>> {
    let line = json!({
        "type": "attempt",
        "outcome": "partial",
        "summary": "Absolute path attempt",
        "files": [located(project, "src/auth/session.py")],
    });
    let mut log = fs::OpenOptions::new()
        .append(true)
        .open(project.join(".dejafix/events.jsonl"))?;
    writeln!(log, "{line}")?;
    Ok(claude_edit(
        project,
        Some("s1"),
        None,
        "src/auth/session.py",
    ))
}

fn codex_patch(project: &Path) -> String {
    let patch = "*** Begin Patch\n*** Update File: src/app.py\n@@\n-a\n+b\n*** Update File: src/auth/session.py\n*** Add File: docs/new.md\n+x\n*** Update File: big/src/app.py\n*** Update File: src/app.py\n*** End Patch\n";
    json!({
        "session_id": "c1",
        "cwd": project.display().to_string(),
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": ["apply_patch", patch]},
    })
    .to_string()
}

fn codex_string_patch(project: &Path) -> String {
    let patch =
        "*** Begin Patch\n*** Update File: auth/session.py\n*** Move to: app.py\n*** End Patch";
    json!({
        "session_id": "c2",
        "cwd": project.join("src").display().to_string(),
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch},
    })
    .to_string()
}

fn edit_actions() -> Vec<Action<'static>> {
    vec![
        Action::EditHook(Box::new(first_edit)),
        edit(Some("s1"), None, "src/auth/session.py"),
        edit(Some("s1"), Some("agent-7"), "src/auth/session.py"),
        edit(Some("s1"), Some("agent-7"), "src/auth/session.py"),
        lifecycle("SessionStart", json!({"source": "compact"})),
        edit(Some("s1"), None, "src/auth/session.py"),
        edit(Some("s1"), None, "src/app.py"),
        lifecycle("PostCompact", json!({"trigger": "auto"})),
        edit(Some("s1"), None, "src/app.py"),
        Action::EditHook(Box::new(|project: &Path| Ok(codex_patch(project)))),
        Action::EditHook(Box::new(|project: &Path| Ok(codex_string_patch(project)))),
        edit(Some("s2"), None, ".claude/worktrees/x/src/app.py"),
        edit(Some("s2"), None, "src/unrelated.py"),
        edit(Some("s2"), None, ".dejafix/summary.md"),
        edit(Some("s3"), None, "big/src/auth/session.py"),
        edit(Some("s3"), None, "big/web/components/Login.tsx"),
        edit(None, None, "src/app.py"),
        edit(None, None, "src/app.py"),
        raw("not json"),
        raw("[]"),
        raw(r#"{"tool_input": "x"}"#),
        raw(""),
    ]
}

#[test]
fn edit_hook_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    edit_fixture(fixture.path())?;
    let actions = edit_actions();
    let (python, differences) = compare(fixture.path(), &actions, Ok)?;
    assert_same("edit hook", &differences);
    for (index, step) in python.steps.iter().enumerate() {
        assert_eq!(step.code, Some(0), "step {index}: {step:?}");
        assert!(step.stderr.is_empty(), "step {index}: {step:?}");
    }
    for index in [0, 2, 5, 6, 8, 9, 11, 14, 16, 17] {
        assert!(
            stdout_of(&python, index).contains("dejafix lessons for "),
            "step {index}: {:?}",
            python.steps.get(index)
        );
    }
    for index in [1, 3, 4, 7, 12, 13, 18, 19, 20, 21] {
        assert_eq!(stdout_of(&python, index), "", "step {index}");
    }
    Ok(())
}
