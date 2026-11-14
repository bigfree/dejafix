use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::Arc;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use super::memo::CHURN_LIFETIME;
use super::types::{GitMemo, MemoGit};
use super::{McpSettings, McpState, ToolCall, ToolReply, WriteCall};
use crate::core::event::Outcome;
use crate::core::stale::FileHistory;
use crate::core::write::OsIds;
use crate::testing::fixed_clock;
use crate::wiring::hooks::GitAnswers;
use crate::wiring::registry::register_project;
use crate::wiring::types::Settings;

const LONG: &str = "gotcha: word0 word1 word2";

struct World {
    _temp: tempfile::TempDir,
    base: PathBuf,
    home: PathBuf,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = crate::canonical_path(temp.path()).unwrap();
        let home = base.join("home");
        fs::create_dir_all(&home).unwrap();
        Self {
            _temp: temp,
            base,
            home,
        }
    }

    fn project(&self, name: &str) -> PathBuf {
        let root = self.base.join(name);
        fs::create_dir_all(root.join(".dejafix/issues")).unwrap();
        fs::write(root.join(".dejafix/config.toml"), "").unwrap();
        fs::write(root.join(".dejafix/events.jsonl"), "").unwrap();
        root
    }

    fn register(&self, root: &Path) {
        let settings = Settings {
            dejafix_home: self.home.join(".dejafix"),
            redact: true,
            clock: fixed_clock(),
        };
        register_project(
            &settings,
            &self.base,
            Some(&self.home),
            &root.to_string_lossy(),
            None,
        )
        .unwrap();
    }

    fn state(&self, pinned: Option<&Path>, cwd: &Path) -> McpState {
        McpState::new(McpSettings {
            pinned: pinned.map(Path::to_path_buf),
            cwd: cwd.to_path_buf(),
            home: Some(self.home.clone()),
            dejafix_home: self.home.join(".dejafix"),
            redact: true,
            zone: jiff::tz::TimeZone::UTC,
        })
    }
}

fn call(state: &McpState, call: ToolCall, project: Option<&str>) -> ToolReply {
    state.call(call, project, fixed_clock().now, &mut OsIds)
}

fn text(state: &McpState, tool: ToolCall, project: Option<&str>) -> String {
    let reply = call(state, tool, project);
    assert!(!reply.error, "{}", reply.text);
    reply.text
}

fn note(summary: &str) -> ToolCall {
    ToolCall::Write(WriteCall::AddNote {
        summary: summary.to_owned(),
        location: None,
    })
}

fn events(root: &Path) -> String {
    fs::read_to_string(root.join(".dejafix/events.jsonl")).unwrap()
}

#[test]
fn a_pinned_server_writes_and_names_the_project() {
    let world = World::new();
    let root = world.project("alpha");
    let state = world.state(Some(&root), &world.base);
    let logged = text(
        &state,
        ToolCall::Write(WriteCall::LogIssue {
            summary: "Login fails".into(),
            location: Some("src/auth.py:3".into()),
        }),
        None,
    );
    assert_eq!(logged, "Logged issue #0001 → alpha");
    let attempted = text(
        &state,
        ToolCall::Write(WriteCall::RecordAttempt {
            summary: LONG.into(),
            outcome: Outcome::Failed,
            location: None,
            issue_id: None,
        }),
        None,
    );
    assert_eq!(attempted, "Recorded failed attempt on #0001 → alpha");
    let fixed = text(
        &state,
        ToolCall::Write(WriteCall::RecordFix {
            summary: "guarded".into(),
            location: None,
            issue_id: Some("1".into()),
        }),
        None,
    );
    assert_eq!(fixed, "Fixed issue #0001 → alpha");
    let noted = text(&state, note("models live in src"), None);
    let id = events(&root)
        .lines()
        .last()
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .and_then(|event| event["id"].as_str().map(str::to_owned))
        .unwrap();
    assert_eq!(noted, format!("Recorded note {id} → alpha"));
    let decided = text(
        &state,
        ToolCall::Write(WriteCall::AddDecision {
            summary: "use argon2".into(),
            location: None,
            supersedes: None,
        }),
        None,
    );
    assert!(decided.starts_with("Recorded decision evt_"), "{decided}");
    let old = decided.split(' ').nth(2).unwrap().to_owned();
    let retired = text(
        &state,
        ToolCall::Write(WriteCall::AddDecision {
            summary: "use scrypt".into(),
            location: None,
            supersedes: Some(old.get(4..).unwrap().to_owned()),
        }),
        None,
    );
    assert!(
        retired.ends_with(&format!(" (supersedes {old})")),
        "{retired}"
    );
    let found = text(
        &state,
        ToolCall::SearchEvents {
            query: "use ".into(),
            limit: 10,
            full: false,
        },
        None,
    );
    assert!(found.contains("use argon2 (superseded)"), "{found}");
}

#[test]
fn several_open_issues_need_an_issue_id() {
    let world = World::new();
    let root = world.project("alpha");
    let first = world.state(Some(&root), &world.base);
    let second = world.state(Some(&root), &world.base);
    let log = |state: &McpState, summary: &str| {
        text(
            state,
            ToolCall::Write(WriteCall::LogIssue {
                summary: summary.into(),
                location: None,
            }),
            None,
        )
    };
    assert_eq!(
        log(&first, "first session bug"),
        "Logged issue #0001 → alpha"
    );
    assert_eq!(
        log(&second, "second session bug"),
        "Logged issue #0002 → alpha"
    );
    let attempt = |issue_id: Option<&str>| {
        ToolCall::Write(WriteCall::RecordAttempt {
            summary: "tried it".into(),
            outcome: Outcome::Failed,
            location: None,
            issue_id: issue_id.map(str::to_owned),
        })
    };
    let refused = call(&first, attempt(None), None);
    assert!(refused.error);
    assert!(
        refused
            .text
            .starts_with("dejafix tool error: DejafixError: 2 issues are open, so pass issue_id"),
        "{}",
        refused.text
    );
    assert_eq!(
        text(&first, attempt(Some("0001")), None),
        "Recorded failed attempt on #0001 → alpha"
    );
}

#[test]
fn one_server_writes_to_the_project_it_was_told() {
    let world = World::new();
    let alpha = world.project("alpha");
    let beta = world.project("beta");
    world.register(&alpha);
    world.register(&beta);
    let state = world.state(None, &world.base);
    assert!(state.global_mode());
    assert!(text(&state, note("alpha bug"), Some("alpha")).ends_with("→ alpha"));
    assert!(text(&state, note("beta bug"), Some("beta")).ends_with("→ beta"));
    assert!(events(&alpha).contains("alpha bug") && !events(&alpha).contains("beta bug"));
    assert!(events(&beta).contains("beta bug") && !events(&beta).contains("alpha bug"));
    let unrouted = call(&state, note("unrouted"), None);
    assert!(unrouted.error);
    assert!(
        unrouted
            .text
            .starts_with("dejafix tool error: ResolutionError: No project selected.")
    );
    assert!(!events(&alpha).contains("unrouted"));
    let listing = text(&state, ToolCall::ListProjects, None);
    assert!(listing.starts_with("2 project(s)"), "{listing}");
    assert_eq!(
        text(&state, ToolCall::CurrentProject, Some("beta")),
        format!("beta — {}\nchosen by: explicit", beta.display())
    );
}

#[test]
fn a_pinned_server_refuses_another_project() {
    let world = World::new();
    let alpha = world.project("alpha");
    let beta = world.project("beta");
    world.register(&alpha);
    world.register(&beta);
    let state = world.state(Some(&alpha), &world.base);
    let reply = call(&state, note("x"), Some("beta"));
    assert!(reply.error);
    assert!(reply.text.contains("is pinned to 'alpha'"));
    assert!(text(&state, ToolCall::ListProjects, None).starts_with("This server is pinned"));
}

#[test]
fn the_working_folder_pins_the_server_as_python_does() {
    let world = World::new();
    let alpha = world.project("alpha");
    let nested = alpha.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    let state = world.state(None, &nested);
    assert!(!state.global_mode());
    assert_eq!(state.pinned(), Some(alpha.as_path()));
}

#[test]
fn read_tools_answer_on_a_bare_project() {
    let world = World::new();
    let root = world.project("alpha");
    let state = world.state(Some(&root), &world.base);
    let fallbacks = [
        (
            ToolCall::GetInstructions,
            "No instructions found. Run `djf init` first.",
        ),
        (
            ToolCall::GetSummary,
            "No summary found. Run `djf init` first.",
        ),
        (ToolCall::GetProjectMap, "No project map found."),
        (ToolCall::GetPlan, "No plan found."),
        (ToolCall::GetIssue("1".into()), "No issue found with ID 1."),
        (ToolCall::GetIssue("*".into()), "No issue found with ID *."),
        (ToolCall::GetGlobalGotchas(None), "No global gotchas found."),
        (
            ToolCall::SearchEvents {
                query: "x".into(),
                limit: 10,
                full: false,
            },
            "No events match 'x'.",
        ),
    ];
    for (tool, expected) in fallbacks {
        assert_eq!(text(&state, tool, None), expected);
    }
    let start = text(&state, ToolCall::StartSession, None);
    assert!(start.ends_with("call get_project_map() only when the file layout matters."));
    assert!(
        text(&state, ToolCall::GetScore, None).starts_with("dejafix Prevention Score: F (0/100)")
    );
    assert!(
        text(
            &state,
            ToolCall::GetContext {
                tokens: 2000,
                focus: None
            },
            None
        )
        .starts_with("## dejafix context (budget: 2000 tokens)")
    );
}

#[test]
fn get_issue_accepts_unpadded_and_legacy_ids_and_rejects_globs() {
    let world = World::new();
    let root = world.project("alpha");
    let issues = root.join(".dejafix/issues");
    fs::write(issues.join("0001-login.md"), "# login\n").unwrap();
    fs::write(issues.join("legacy_f436-old.md"), "# old\n").unwrap();
    let state = world.state(Some(&root), &world.base);
    for wanted in ["1", "#1", "0001", " 001 "] {
        assert_eq!(
            text(&state, ToolCall::GetIssue(wanted.into()), None),
            "# login\n"
        );
    }
    assert_eq!(
        text(&state, ToolCall::GetIssue("legacy_f436".into()), None),
        "# old\n"
    );
    for glob in ["*", "00?1", "../0001", "0001-*"] {
        assert_eq!(
            text(&state, ToolCall::GetIssue(glob.into()), None),
            format!("No issue found with ID {glob}.")
        );
    }
}

#[test]
fn precheck_matches_every_spelling_and_names_memory_files() {
    let world = World::new();
    let root = world.project("alpha");
    let state = world.state(Some(&root), &world.base);
    text(
        &state,
        ToolCall::Write(WriteCall::LogIssue {
            summary: "auth breaks".into(),
            location: Some(format!("{}/src/auth.py", root.display())),
        }),
        None,
    );
    for spelling in [
        "src/auth.py".to_owned(),
        "./src/auth.py".to_owned(),
        format!("{}/src/auth.py", root.display()),
    ] {
        let reply = text(&state, ToolCall::PrecheckFile(spelling.clone()), None);
        assert!(
            reply.starts_with(&format!("dejafix precheck: {spelling}")),
            "{reply}"
        );
        assert!(reply.contains("auth breaks"), "{reply}");
    }
    let both = text(
        &state,
        ToolCall::PrecheckFiles(vec!["src/auth.py".into(), "src/other.py".into()]),
        None,
    );
    let single = [
        text(&state, ToolCall::PrecheckFile("src/auth.py".into()), None),
        text(&state, ToolCall::PrecheckFile("src/other.py".into()), None),
    ]
    .join("\n\n");
    assert_eq!(both, single);
    let memory = text(
        &state,
        ToolCall::PrecheckFile(".dejafix/summary.md".into()),
        None,
    );
    assert!(memory.contains("memory file") && memory.contains("add_note"));
}

#[test]
fn the_cache_serves_until_the_log_changes() {
    let world = World::new();
    let root = world.project("alpha");
    let state = world.state(Some(&root), &world.base);
    let clock = fixed_clock();
    let first = state.cached_log(&root, &clock).unwrap();
    let again = state.cached_log(&root, &clock).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    fs::write(
        root.join(".dejafix/events.jsonl"),
        "{\"id\": \"evt_a\", \"summary\": \"s\", \"timestamp\": \"2026-01-01T00:00:00Z\", \"type\": \"note\"}\n{broken\n",
    )
    .unwrap();
    let reloaded = state.cached_log(&root, &clock).unwrap();
    assert!(!Arc::ptr_eq(&first, &reloaded));
    assert_eq!(reloaded.log.events().len(), 1);
    assert_eq!(reloaded.log.skipped().len(), 1);
    assert_eq!(state.cached_projects(), 1);
}

#[test]
fn the_cache_keeps_at_most_sixteen_projects() {
    let world = World::new();
    let state = world.state(Some(&world.project("pinned")), &world.base);
    let clock = fixed_clock();
    for index in 0..20 {
        let root = world.project(&format!("p{index}"));
        state.cached_log(&root, &clock).unwrap();
    }
    assert_eq!(state.cached_projects(), 16);
}

#[test]
fn a_missing_log_is_python_s_file_not_found_error() {
    let world = World::new();
    let root = world.project("alpha");
    fs::remove_file(root.join(".dejafix/events.jsonl")).unwrap();
    let state = world.state(Some(&root), &world.base);
    let reply = call(&state, ToolCall::GetScore, None);
    assert!(reply.error);
    assert_eq!(
        reply.text,
        format!(
            "dejafix tool error: FileNotFoundError: [Errno 2] No such file or directory: '{}'",
            root.join(".dejafix")
                .join("events.jsonl")
                .display()
                .to_string()
                .replace('\\', "\\\\")
        )
    );
}

#[test]
fn precheck_and_context_use_the_resolved_project_s_git() {
    let world = World::new();
    let root = world.project("alpha");
    let git = |args: &[&str]| {
        let status = Process::new("git")
            .args(args)
            .current_dir(&root)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .status()
            .unwrap();
        assert!(status.success());
    };
    git(&["init", "-q"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/auth.py"), "a\n").unwrap();
    git(&["add", "src/auth.py"]);
    git(&["commit", "-q", "-m", "one"]);
    let elsewhere = world.base.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let state = world.state(Some(&root), &elsewhere);
    text(
        &state,
        ToolCall::Write(WriteCall::LogIssue {
            summary: "auth breaks".into(),
            location: Some("src/auth.py".into()),
        }),
        None,
    );
    let reply = text(&state, ToolCall::PrecheckFile("src/auth.py".into()), None);
    assert!(reply.contains("auth breaks"), "{reply}");
}

#[test]
fn a_pinned_server_resolves_paths_from_its_root_as_python_s_chdir_does() {
    let world = World::new();
    let root = world.project("alpha");
    let elsewhere = world.base.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let state = world.state(Some(&root), &elsewhere);
    let cached = state.cached_log(&root, &fixed_clock()).unwrap();
    let posix = if cfg!(windows) {
        root.to_string_lossy().replace('\\', "/")
    } else {
        root.to_string_lossy().into_owned()
    };
    assert_eq!(cached.context.cwd, posix);
}

#[derive(Default)]
struct Counting {
    churn: AtomicUsize,
    times: AtomicUsize,
    answer: AtomicUsize,
}

impl FileHistory for Counting {
    fn exists(&self, _relative: &str) -> bool {
        true
    }

    fn commit_times(&self, _relative: &str, _since: Option<&str>) -> Option<Vec<String>> {
        self.times.fetch_add(1, Ordering::SeqCst);
        Some(vec![self.answer.load(Ordering::SeqCst).to_string()])
    }
}

impl GitAnswers for Counting {
    fn churn(&self, _relative: &str) -> Option<usize> {
        self.churn.fetch_add(1, Ordering::SeqCst);
        Some(self.answer.load(Ordering::SeqCst))
    }
}

fn committed_repository(root: &Path, message: &str) {
    let status = Process::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["commit", "-q", "--allow-empty", "-m", message])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn churn_is_remembered_for_sixty_seconds_of_one_repository_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(
        Process::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
    committed_repository(root, "one");
    let memo = Mutex::new(GitMemo::default());
    let inner = Counting::default();
    let start = fixed_clock().now;
    let ask = |inner: &Counting, now: jiff::Timestamp| {
        MemoGit {
            inner,
            root,
            state: OnceLock::new(),
            memo: &memo,
            now,
        }
        .churn("src/a.py")
    };
    inner.answer.store(3, Ordering::SeqCst);
    assert_eq!(ask(&inner, start), Some(3));
    inner.answer.store(4, Ordering::SeqCst);
    let almost = start
        .checked_add(CHURN_LIFETIME - jiff::SignedDuration::from_secs(1))
        .unwrap();
    assert_eq!(
        ask(&inner, almost),
        Some(3),
        "a churn answer may lag git for up to 60 s"
    );
    assert_eq!(inner.churn.load(Ordering::SeqCst), 1);
    let expired = start.checked_add(CHURN_LIFETIME).unwrap();
    assert_eq!(ask(&inner, expired), Some(4));
    assert_eq!(inner.churn.load(Ordering::SeqCst), 2);
    inner.answer.store(5, Ordering::SeqCst);
    committed_repository(root, "two");
    assert_eq!(
        ask(&inner, expired),
        Some(5),
        "a new commit is seen at once"
    );
    assert_eq!(inner.churn.load(Ordering::SeqCst), 3);
}

#[test]
fn commit_times_are_remembered_until_the_repository_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(
        Process::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
    committed_repository(root, "one");
    let memo = Mutex::new(GitMemo::default());
    let inner = Counting::default();
    let ask = |now: jiff::Timestamp, since: Option<&str>| {
        MemoGit {
            inner: &inner,
            root,
            state: OnceLock::new(),
            memo: &memo,
            now,
        }
        .commit_times("src/a.py", since)
    };
    let start = fixed_clock().now;
    let much_later = start
        .checked_add(jiff::SignedDuration::from_hours(48))
        .unwrap();
    assert_eq!(ask(start, None), Some(vec!["0".to_owned()]));
    inner.answer.store(1, Ordering::SeqCst);
    assert_eq!(ask(much_later, None), Some(vec!["0".to_owned()]));
    assert_eq!(ask(start, Some("2026-01-01")), Some(vec!["1".to_owned()]));
    assert_eq!(inner.times.load(Ordering::SeqCst), 2);
    committed_repository(root, "two");
    assert_eq!(ask(much_later, None), Some(vec!["1".to_owned()]));
    assert_eq!(inner.times.load(Ordering::SeqCst), 3);
}

#[test]
fn precheck_sees_a_new_commit_at_once_through_the_memo() {
    let world = World::new();
    let root = world.project("alpha");
    let git = |args: &[&str]| {
        let status = Process::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(args)
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
    };
    git(&["init", "-q"]);
    let state = world.state(Some(&root), &world.base);
    text(
        &state,
        ToolCall::Write(WriteCall::LogIssue {
            summary: "auth breaks".into(),
            location: Some("src/auth.py".into()),
        }),
        None,
    );
    fs::create_dir_all(root.join("src")).unwrap();
    let churn_of = |state: &McpState| {
        let reply = state.call(
            ToolCall::PrecheckFile("src/auth.py".into()),
            None,
            jiff::Timestamp::now(),
            &mut OsIds,
        );
        reply.text.contains("HIGH CHURN")
    };
    for round in 0..3 {
        fs::write(root.join("src/auth.py"), format!("{round}\n")).unwrap();
        git(&["add", "src/auth.py"]);
        git(&["commit", "-q", "-m", &format!("change {round}")]);
    }
    assert!(!churn_of(&state));
    fs::write(root.join("src/auth.py"), "3\n").unwrap();
    git(&["add", "src/auth.py"]);
    git(&["commit", "-q", "-m", "change 3"]);
    assert!(
        churn_of(&state),
        "the fourth commit must not wait for the memo to expire"
    );
}

#[cfg(unix)]
#[test]
fn a_memory_folder_linked_out_of_the_project_is_refused_over_mcp() {
    let world = World::new();
    let outside = world.project("outside");
    let root = world.base.join("alpha");
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(outside.join(".dejafix"), root.join(".dejafix")).unwrap();
    let state = world.state(Some(&root), &world.base);
    let reply = call(&state, note("x"), None);
    assert!(reply.error, "{}", reply.text);
    assert!(
        reply
            .text
            .contains("points outside the project; refusing to use it"),
        "{}",
        reply.text
    );
    assert_eq!(events(&outside), "");
    assert!(call(&state, ToolCall::GetScore, None).error);
}

#[cfg(unix)]
#[test]
fn memory_files_linked_out_of_the_project_read_as_missing() {
    use std::os::unix::fs::symlink;
    let world = World::new();
    let root = world.project("alpha");
    let secret = world.base.join("secret.txt");
    fs::write(&secret, "SECRET\n").unwrap();
    for name in [
        "AI_INSTRUCTIONS.md",
        "summary.md",
        "PROJECT_MAP.md",
        "plan.md",
    ] {
        symlink(&secret, root.join(".dejafix").join(name)).unwrap();
    }
    symlink(&secret, root.join(".dejafix/issues/0001-leak.md")).unwrap();
    let state = world.state(Some(&root), &world.base);
    for (tool, expected) in [
        (
            ToolCall::GetInstructions,
            "No instructions found. Run `djf init` first.",
        ),
        (
            ToolCall::GetSummary,
            "No summary found. Run `djf init` first.",
        ),
        (ToolCall::GetProjectMap, "No project map found."),
        (ToolCall::GetPlan, "No plan found."),
        (ToolCall::GetIssue("1".into()), "No issue found with ID 1."),
    ] {
        assert_eq!(text(&state, tool, None), expected);
    }
    assert!(!text(&state, ToolCall::StartSession, None).contains("SECRET"));
    fs::remove_file(root.join(".dejafix/summary.md")).unwrap();
    fs::write(root.join(".dejafix/real.md"), "# Inside\n").unwrap();
    symlink(
        root.join(".dejafix/real.md"),
        root.join(".dejafix/summary.md"),
    )
    .unwrap();
    assert_eq!(text(&state, ToolCall::GetSummary, None), "# Inside\n");
}
