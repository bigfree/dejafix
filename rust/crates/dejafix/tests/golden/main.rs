mod corpus;
mod harness;
mod m1;
mod normalize;
mod types;

use std::error::Error;
use std::path::Path;

use dejafix_core::testing::{FIXTURE_SEED, FIXTURE_SIZES, write_project};
use harness::{diff, isolated, output, python, repository, run};
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
        assert_eq!(String::from_utf8(output.stdout)?, format!("ok {size}\n"));
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
    let stub = run(Implementation::Rust, dir.path(), &[&["show"]])?;
    assert_eq!(stub.steps.first().and_then(|step| step.code), Some(1));
    assert_eq!(diff(&untouched, &stub), ["step count 0 != 1"]);
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
