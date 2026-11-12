use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use criterion::{Criterion, criterion_group, criterion_main};
use dejafix_core::testing::{FIXTURE_SEED, write_project};

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
    let payload = format!(
        r#"{{"session_id": "bench", "tool_name": "Edit", "tool_input": {{"file_path": "{}"}}}}"#,
        root.join(CITED).display()
    );
    let mut group = criterion.benchmark_group("process");
    group.bench_function("edit_hook/3000", |bencher| {
        bencher.iter(|| {
            let mut child = Command::new(env!("CARGO_BIN_EXE_djf-edit-hook"))
                .env("TMPDIR", &temp)
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
        });
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
