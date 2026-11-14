mod corpus;
mod harness;
mod m1;
mod m2;
mod m2b;
mod m2c;
mod m3;
mod m4;
mod m5;
mod normalize;
mod scenario;
mod types;

use std::error::Error;
use std::fs;
use std::path::Path;

use dejafix_core::testing::{FIXTURE_SEED, FIXTURE_SIZES, write_project};
use harness::{
    choose_python, console_text, diff, isolated, output, python, python_candidates, repository, run,
};
use types::{Implementation, Normalizer, Step};

type TestResult = Result<(), Box<dyn Error>>;

const SELF_CHECK: &[Step] = &[
    &[
        "log",
        "Golden harness self-check",
        "--at",
        "src/golden.py:1",
    ],
    &["attempt", "Retry the golden step", "--failed"],
    &["decision", "Keep the golden harness"],
    &["note", "Golden harness note"],
    &["fix", "Golden step fixed", "--issue", "0017"],
];

#[test]
fn every_fixture_is_read_by_python_read_events() -> TestResult {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/check_fixture.py");
    let repository = repository()?;
    let source = repository.join("src");
    let interpreter = python(&repository);
    for size in FIXTURE_SIZES {
        let dir = tempfile::tempdir()?;
        let home = tempfile::tempdir()?;
        write_project(dir.path(), size, FIXTURE_SEED)?;
        let mut command = isolated(&interpreter, home.path(), &source);
        let output = output(command.arg(&script).arg(dir.path()))?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(console_text(&output.stdout), format!("ok {size}\n"));
    }
    Ok(())
}

#[test]
fn python_matches_itself_after_normalisation() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), 100, FIXTURE_SEED)?;
    let first = run(Implementation::Python, dir.path(), SELF_CHECK)?;
    let second = run(Implementation::Python, dir.path(), SELF_CHECK)?;
    assert!(
        first.steps.iter().all(|step| step.code == Some(0)),
        "{:?}",
        first.steps
    );
    assert!(first.files.values().any(|text| text.contains("<ID 1>")));
    assert!(first.files.values().any(|text| text.contains("<NOW>")));
    let differences = diff(&first, &second);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    Ok(())
}

#[test]
fn rust_runs_on_a_fixture_copy_without_touching_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), 100, FIXTURE_SEED)?;
    let untouched = run(Implementation::Rust, dir.path(), &[])?;
    let searched = run(Implementation::Rust, dir.path(), &[&["search", "anything"]])?;
    assert_eq!(searched.steps.first().and_then(|step| step.code), Some(0));
    assert_eq!(
        diff(&untouched, &searched),
        [
            "step count 0 != 1",
            "home/.dejafix/projects.meta.json: only on the right"
        ]
    );
    Ok(())
}

#[test]
fn diff_names_the_first_differing_line() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), 10, FIXTURE_SEED)?;
    let base = run(Implementation::Rust, dir.path(), &[])?;
    let mut changed = run(Implementation::Rust, dir.path(), &[])?;
    changed.files.insert(
        "project/.dejafix/config.toml".into(),
        "recent_days = 7\n".into(),
    );
    changed.files.insert("project/extra".into(), String::new());
    changed.files.remove("project/.dejafix/events.jsonl");
    assert_eq!(
        diff(&base, &changed),
        [
            "project/.dejafix/config.toml:1: \"recent_days = 30\\n\" != \"recent_days = 7\\n\"",
            "project/.dejafix/events.jsonl: only on the left",
            "project/extra: only on the right",
        ]
    );
    Ok(())
}

#[test]
fn normalizer_numbers_ids_and_collapses_new_timestamps() -> TestResult {
    let root = Path::new("/tmp/golden-root");
    let mut normalizer = Normalizer::new(&[(root, "<PROJECT>")], "2026-11-12T10:00:00Z")?;
    let text = "evt_0123456789abcdef0123 evt_ffffffffffffffffffff evt_0123456789abcdef0123 \
                2024-01-01T00:00:00Z 2026-11-12T10:00:00Z 2026-11-12T11:30:00Z /tmp/golden-root/x \
                2025-03-04 2026-11-12 2026-11-13";
    assert_eq!(
        normalizer.apply(text),
        "<ID 1> <ID 2> <ID 1> 2024-01-01T00:00:00Z <NOW> <NOW> <PROJECT>/x \
         2025-03-04 <TODAY> <TODAY>"
    );
    Ok(())
}

#[test]
fn normalizer_handles_global_ids_fractions_minutes_and_versions() -> TestResult {
    let mut normalizer =
        Normalizer::new(&[], "2026-11-12T10:00:00Z")?.with_versions(&["0.4.0", "0.0.0"])?;
    let text = "got_0123456789ab evt_0123456789abcdef0123 got_0123456789ab \
                2026-11-12T10:00:01.123456Z 2026-11-12 10:31 2026-11-11 23:59 \
                \"last_version\": \"0.4.0\" dejafix 0.0.0 — checking";
    assert_eq!(
        normalizer.apply(text),
        "<ID 1> <ID 2> <ID 1> <NOW> <MINUTE> 2026-11-11 23:59 \
         \"last_version\": \"<VERSION>\" dejafix <VERSION> — checking"
    );
    Ok(())
}

#[test]
fn diff_shows_a_difference_in_line_endings() -> TestResult {
    let dir = tempfile::tempdir()?;
    write_project(dir.path(), 1, FIXTURE_SEED)?;
    let base = run(Implementation::Rust, dir.path(), &[])?;
    let mut changed = run(Implementation::Rust, dir.path(), &[])?;
    changed.files.insert(
        "project/.dejafix/config.toml".into(),
        "recent_days = 30\r\nproject_description = \"\"\r\n".into(),
    );
    assert_eq!(
        diff(&base, &changed),
        ["project/.dejafix/config.toml:1: \"recent_days = 30\\n\" != \"recent_days = 30\\r\\n\""]
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn the_harness_finds_the_main_checkout_s_virtualenv_from_a_worktree() -> TestResult {
    let dir = tempfile::tempdir()?;
    let main = fs::canonicalize(dir.path())?.join("main");
    let worktree = main.with_file_name("worktree");
    fs::create_dir_all(main.join(".venv/bin"))?;
    let fake = main.join(".venv/bin/python");
    fs::write(&fake, "#!/bin/sh\nexit 0\n")?;
    let git = |args: &[&str]| -> TestResult {
        let status = std::process::Command::new("git")
            .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
            .args(args)
            .current_dir(&main)
            .status()?;
        assert!(status.success(), "git {args:?}");
        Ok(())
    };
    git(&["init", "-q"])?;
    git(&["commit", "-q", "--allow-empty", "-m", "init"])?;
    git(&["worktree", "add", "-q", "../worktree"])?;
    assert_eq!(python_candidates(None, &worktree).first(), Some(&fake));
    assert_eq!(choose_python(None, &worktree, |_| true)?, fake);
    assert_eq!(
        python_candidates(Some("/opt/py".into()), &worktree),
        [std::path::PathBuf::from("/opt/py")]
    );
    let error = choose_python(None, &worktree, |_| false)
        .err()
        .ok_or("no error")?;
    assert!(error.contains("set DEJAFIX_PYTHON to one"), "{error}");
    assert!(error.contains(&fake.display().to_string()), "{error}");
    Ok(())
}
