use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, gotcha_lines, write_project};
use dejafix_core::{GotchaInput, McpSettings, McpState, OsIds, ToolCall};

const CITED: &str = "src/app.py";
const UNCITED: &str = "src/never_logged.py";

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.email=b@example.invalid", "-c", "user.name=b"])
        .args(args)
        .current_dir(root)
        .stdout(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn state(root: &Path, home: &Path) -> McpState {
    McpState::new(McpSettings {
        pinned: Some(root.to_path_buf()),
        cwd: root.to_path_buf(),
        home: Some(home.to_path_buf()),
        dejafix_home: home.join(".dejafix"),
        redact: true,
        zone: jiff::tz::TimeZone::UTC,
    })
}

fn reply(state: &McpState, call: ToolCall) -> String {
    state
        .call(call, None, jiff::Timestamp::now(), &mut OsIds)
        .text
}

fn global_query() -> ToolCall {
    ToolCall::GlobalQuery {
        query: "worker never exits on shutdown".to_owned(),
        library: None,
        budget: 2_000,
    }
}

fn new_gotcha() -> ToolCall {
    ToolCall::AddGlobalGotcha(GotchaInput {
        library: "bullmq".to_owned(),
        keywords: vec!["benchmark".to_owned(), "unique lesson".to_owned()],
        gotcha: "bullmq benchmark lesson that no generated record repeats, about queue draining."
            .to_owned(),
        versions: None,
    })
}

fn precheck(state: &McpState, file: &str) -> bool {
    state
        .call(
            ToolCall::PrecheckFile(file.to_owned()),
            None,
            jiff::Timestamp::now(),
            &mut OsIds,
        )
        .error
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn mcp(criterion: &mut Criterion) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let base = fs::canonicalize(dir.path()).expect("canonical root");
    let root = base.join("repo");
    let home = base.join("home");
    fs::create_dir_all(root.join("src")).expect("source folder");
    fs::create_dir_all(&home).expect("home folder");
    write_project(&root, 3_000, FIXTURE_SEED).expect("fixture");
    fs::write(root.join(CITED), "print('app')\n").expect("cited file");
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fixture", "--no-verify"]);
    let warm = state(&root, &home);
    assert!(!precheck(&warm, CITED), "precheck_file answers");
    let mut group = criterion.benchmark_group("mcp");
    group.bench_function("precheck_file/warm/cited/3000", |bencher| {
        bencher.iter(|| precheck(&warm, CITED));
    });
    group.bench_function("precheck_file/warm/uncited/3000", |bencher| {
        bencher.iter(|| precheck(&warm, UNCITED));
    });
    group.bench_function("precheck_file/cold/uncited/3000", |bencher| {
        bencher.iter(|| precheck(&state(&root, &home), UNCITED));
    });
    let global = home.join(".dejafix/global");
    fs::create_dir_all(&global).expect("global folder");
    let store = global.join("library_gotchas.jsonl");
    let lines: String = gotcha_lines(500, FIXTURE_SEED)
        .into_iter()
        .map(|line| line + "\n")
        .collect();
    fs::write(&store, &lines).expect("global store");
    assert!(reply(&warm, global_query()).starts_with("Global gotchas for"));
    group.bench_function("global_query/warm/500", |bencher| {
        bencher.iter(|| reply(&warm, global_query()));
    });
    group.bench_function("global_query/cold/500", |bencher| {
        bencher.iter(|| reply(&state(&root, &home), global_query()));
    });
    assert!(reply(&warm, new_gotcha()).starts_with("Added global gotcha"));
    group.bench_function("add_global_gotcha/500", |bencher| {
        bencher.iter_batched(
            || fs::write(&store, &lines).expect("reset store"),
            |()| reply(&warm, new_gotcha()),
            BatchSize::PerIteration,
        );
    });
    fs::write(&store, &lines).expect("reset store");
    group.bench_function("git_log_since", |bencher| {
        bencher.iter(|| {
            Command::new("git")
                .args(["log", "--since=30.days.ago", "--oneline", "--", CITED])
                .current_dir(&root)
                .stdout(Stdio::null())
                .status()
                .expect("git runs")
        });
    });
    group.finish();
}

criterion_group!(benches, mcp);
criterion_main!(benches);
