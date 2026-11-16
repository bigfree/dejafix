use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use criterion::{Criterion, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, gotcha_lines, write_project};

const CITED: &str = "src/app.py";

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

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn edit_hook(temp: &Path, home: &Path, payload: &str) -> std::process::ExitStatus {
    let mut child = Command::new(env!("CARGO_BIN_EXE_djf-edit-hook"))
        .env("TMPDIR", temp)
        .env("DEJAFIX_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("edit hook starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload written");
    child.wait().expect("edit hook ends")
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn link_projects(base: &Path, root: &Path, home: &Path) -> String {
    let mut registered = vec![root.display().to_string()];
    let mut links = String::new();
    for name in ["inventory-a", "inventory-b"] {
        let linked = base.join(name);
        write_project(&linked, 3_000, FIXTURE_SEED).expect("linked fixture");
        fs::create_dir_all(linked.join(".git")).expect("git folder");
        fs::write(
            linked.join(".git/config"),
            format!("[remote \"origin\"]\n\turl = git@github.com:example/{name}.git\n"),
        )
        .expect("git config");
        let config = linked.join(".dejafix/config.toml");
        let mut text = fs::read_to_string(&config).expect("linked config");
        text.push_str("boundary = [\"src/**\"]\n");
        fs::write(&config, text).expect("linked boundary");
        writeln!(
            links,
            "\n[[links]]\nproject = \"github.com/example/{name}\""
        )
        .expect("link written");
        registered.push(linked.display().to_string());
    }
    let config = root.join(".dejafix/config.toml");
    let mut text = fs::read_to_string(&config).expect("config");
    text.push_str("boundary = [\"src/api/**\"]\n");
    text.push_str(&links);
    fs::write(&config, text).expect("boundary");
    fs::create_dir_all(home).expect("home folder");
    fs::write(
        home.join("projects.json"),
        serde_json::to_string(&registered).expect("registry list"),
    )
    .expect("registry");
    format!(
        r#"{{"tool_name": "Edit", "tool_input": {{"file_path": "{}"}}}}"#,
        root.join("src/api/session.ts").display()
    )
}

#[expect(
    clippy::expect_used,
    reason = "a benchmark without its fixture must fail loudly"
)]
fn hooks(criterion: &mut Criterion) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let root = fs::canonicalize(dir.path())
        .expect("canonical root")
        .join("repo");
    fs::create_dir_all(root.join("src")).expect("source folder");
    write_project(&root, 3_000, FIXTURE_SEED).expect("fixture");
    fs::write(root.join(CITED), "print('app')\n").expect("cited file");
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fixture", "--no-verify"]);
    fs::write(root.join(CITED), "print('changed')\n").expect("changed file");
    git(&root, &["add", CITED]);
    let temp = dir.path().join("tmp");
    fs::create_dir_all(&temp).expect("temp folder");
    let home = dir.path().join("home");
    let boundary_payload = link_projects(dir.path(), &root, &home);
    let payload = format!(
        r#"{{"session_id": "bench", "tool_name": "Edit", "tool_input": {{"file_path": "{}"}}}}"#,
        root.join(CITED).display()
    );
    let mut group = criterion.benchmark_group("process");
    group.bench_function("edit_hook/3000", |bencher| {
        bencher.iter(|| edit_hook(&temp, &home, &payload));
    });
    group.bench_function("edit_hook_boundary/3000x2", |bencher| {
        bencher.iter(|| edit_hook(&temp, &home, &boundary_payload));
    });
    group.bench_function("precheck/3000", |bencher| {
        bencher.iter(|| {
            Command::new(env!("CARGO_BIN_EXE_djf"))
                .args(["precheck", "--level", "warn"])
                .current_dir(&root)
                .env("DEJAFIX_HOME", dir.path().join("home"))
                .stdout(Stdio::null())
                .status()
                .expect("precheck runs")
        });
    });
    group.bench_function("search/3000", |bencher| {
        bencher.iter(|| {
            Command::new(env!("CARGO_BIN_EXE_djf"))
                .args(["search", "token refresh login loop"])
                .current_dir(&root)
                .env("DEJAFIX_HOME", dir.path().join("home"))
                .stdout(Stdio::null())
                .status()
                .expect("search runs")
        });
    });
    let global = dir.path().join("home/global");
    fs::create_dir_all(&global).expect("global folder");
    let lines: String = gotcha_lines(500, FIXTURE_SEED)
        .into_iter()
        .map(|line| line + "\n")
        .collect();
    fs::write(global.join("library_gotchas.jsonl"), lines).expect("global store");
    group.bench_function("global_query/500", |bencher| {
        bencher.iter(|| {
            Command::new(env!("CARGO_BIN_EXE_djf"))
                .args(["global", "query", "worker never exits on shutdown"])
                .current_dir(&root)
                .env("DEJAFIX_HOME", dir.path().join("home"))
                .stdout(Stdio::null())
                .status()
                .expect("global query runs")
        });
    });
    group.bench_function("git_diff_cached", |bencher| {
        bencher.iter(|| {
            Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .current_dir(&root)
                .stdout(Stdio::null())
                .status()
                .expect("git runs")
        });
    });
    group.bench_function("precheck_named/3000", |bencher| {
        bencher.iter(|| {
            Command::new(env!("CARGO_BIN_EXE_djf"))
                .args(["precheck", CITED])
                .current_dir(&root)
                .env("DEJAFIX_HOME", dir.path().join("home"))
                .stdout(Stdio::null())
                .status()
                .expect("precheck runs")
        });
    });
    group.finish();
}

criterion_group!(benches, hooks);
criterion_main!(benches);
