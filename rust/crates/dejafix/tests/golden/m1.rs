use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use dejafix_core::testing::{
    FIXTURE_SEED, FIXTURE_SIZES, is_python_decimal, is_python_space, python_lines, write_project,
};
use dejafix_core::{
    Clock, EventId, EventLog, FsResolver, IssueId, PathContext, is_memory_path, location_to_file,
    normalize_timestamp, path_context, project_relative,
};
use jiff::tz::TimeZone;
use serde_json::{Value, json};

use crate::corpus::{EDGE_LOG, issue_ids, locations, paths, timestamps};
use crate::harness::{isolated, python, repository};

type TestResult = Result<(), Box<dyn Error>>;

const ZONE: &str = "America/Denver";

fn oracle(args: &[&str], cwd: &Path, stdin: &str) -> Result<String, Box<dyn Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/oracle.py");
    let repository = repository()?;
    let home = tempfile::tempdir()?;
    let mut command = isolated(python(&repository), home.path(), &repository.join("src"));
    command
        .env("TZ", ZONE)
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped());
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(stdin.as_bytes())?;
    let finished = child.wait_with_output()?;
    if !finished.status.success() {
        return Err(String::from_utf8_lossy(&finished.stderr)
            .into_owned()
            .into());
    }
    Ok(String::from_utf8(finished.stdout)?)
}

fn clock() -> Result<Clock, Box<dyn Error>> {
    Ok(Clock {
        now: jiff::Timestamp::now(),
        zone: TimeZone::get(ZONE)?,
    })
}

fn context(root: &Path) -> Result<PathContext, Box<dyn Error>> {
    let mut context = path_context(root)?;
    context.cwd.clone_from(&context.root);
    Ok(context)
}

fn rust_rewrite(root: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(root.join(".dejafix/events.jsonl"))?;
    let log = EventLog::parse(&bytes, &clock()?, &context(root)?, &FsResolver);
    let mut out = Vec::new();
    for event in log.events() {
        event.write_line(&mut out)?;
    }
    for skipped in log.skipped() {
        writeln!(out, "SKIP {}", skipped.line)?;
    }
    Ok(String::from_utf8(out)?)
}

fn rust_index(root: &Path) -> Result<Value, Box<dyn Error>> {
    let bytes = fs::read(root.join(".dejafix/events.jsonl"))?;
    let log = EventLog::parse(&bytes, &clock()?, &context(root)?, &FsResolver);
    let issues: BTreeSet<&str> = log
        .events()
        .iter()
        .filter_map(|e| e.issue.as_ref().map(IssueId::as_str))
        .collect();
    let references: BTreeSet<_> = log
        .events()
        .iter()
        .filter_map(|e| e.supersedes.as_ref())
        .collect();
    let files: BTreeMap<&str, Vec<&str>> = log
        .indexed_files()
        .map(|path| {
            (
                path.as_str(),
                log.live_events_for_file(path.as_str())
                    .map(|e| e.id.as_str())
                    .collect(),
            )
        })
        .collect();
    Ok(json!({
        "open": log.open_issues().map(IssueId::as_str).collect::<Vec<_>>(),
        "closed": issues.into_iter().filter(|issue| log.is_closed(&IssueId::from(*issue))).collect::<Vec<_>>(),
        "retired": references.into_iter().filter(|id| log.is_retired(id)).map(EventId::as_str).collect::<Vec<_>>(),
        "files": files,
    }))
}

fn first_difference(left: &str, right: &str) -> Option<String> {
    let mut l = left.lines();
    let mut r = right.lines();
    (1..)
        .find_map(|number| match (l.next(), r.next()) {
            (None, None) => Some(None),
            (a, b) if a == b => None,
            (a, b) => Some(Some(format!("line {number}: python {a:?} != rust {b:?}"))),
        })
        .flatten()
}

fn project(log: &str) -> Result<tempfile::TempDir, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    fs::create_dir_all(dir.path().join(".dejafix"))?;
    fs::write(dir.path().join(".dejafix/events.jsonl"), log)?;
    Ok(dir)
}

#[test]
fn every_fixture_round_trips_byte_identically_to_python() -> TestResult {
    for size in FIXTURE_SIZES {
        let dir = tempfile::tempdir()?;
        write_project(dir.path(), size, FIXTURE_SEED)?;
        let log = dir.path().join(".dejafix/events.jsonl");
        let original = fs::read_to_string(&log)?;
        let python = oracle(&["rewrite", log.to_str().ok_or("path")?], dir.path(), "")?;
        let rust = rust_rewrite(dir.path())?;
        assert_eq!(first_difference(&python, &rust), None, "{size} events");
        assert_eq!(
            first_difference(&original, &rust),
            None,
            "{size} events against the file"
        );
    }
    Ok(())
}

#[test]
fn edge_lines_round_trip_like_python() -> TestResult {
    let dir = project(EDGE_LOG)?;
    let log = dir.path().join(".dejafix/events.jsonl");
    let python = oracle(&["rewrite", log.to_str().ok_or("path")?], dir.path(), "")?;
    let rust = rust_rewrite(dir.path())?;
    assert_eq!(first_difference(&python, &rust), None);
    assert!(
        rust.contains("SKIP"),
        "the corpus must exercise skipped lines"
    );
    Ok(())
}

#[test]
fn indexes_match_python_precheck() -> TestResult {
    let readable_by_precheck = EDGE_LOG.replace(r#", "location": {"a": 1}"#, "");
    let mut roots = vec![project(&readable_by_precheck)?];
    for size in FIXTURE_SIZES {
        let dir = tempfile::tempdir()?;
        write_project(dir.path(), size, FIXTURE_SEED)?;
        roots.push(dir);
    }
    for root in &roots {
        let python: Value = serde_json::from_str(&oracle(
            &["index", root.path().to_str().ok_or("path")?],
            root.path(),
            "",
        )?)?;
        let rust = rust_index(root.path())?;
        for key in ["open", "closed", "retired"] {
            assert_eq!(python[key], rust[key], "{key} in {}", root.path().display());
        }
        let (python_files, rust_files) = (
            python["files"].as_object().ok_or("files")?,
            rust["files"].as_object().ok_or("files")?,
        );
        let keys: BTreeSet<&String> = python_files.keys().chain(rust_files.keys()).collect();
        let differing: Vec<String> = keys
            .into_iter()
            .filter(|key| python_files.get(*key) != rust_files.get(*key))
            .map(|key| {
                format!(
                    "{key:?}: python {:?} != rust {:?}",
                    python_files.get(key),
                    rust_files.get(key)
                )
            })
            .collect();
        assert!(differing.is_empty(), "{}", differing.join("\n"));
    }
    Ok(())
}

fn compare_scalars(
    kind: &[&str],
    cwd: &Path,
    inputs: &[String],
    rust: impl Fn(&str) -> Value,
) -> TestResult {
    let stdin = inputs
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let python = oracle(&[&["scalars"], kind].concat(), cwd, &stdin)?;
    let mut mismatches = Vec::new();
    for (input, line) in inputs.iter().zip(python.lines()) {
        let expected: Value = serde_json::from_str(line)?;
        let actual = rust(input);
        if expected != actual {
            mismatches.push(format!("{input:?}: python {expected} != rust {actual}"));
        }
    }
    assert_eq!(python.lines().count(), inputs.len());
    assert!(
        mismatches.is_empty(),
        "{} {}:\n{}",
        kind.first().unwrap_or(&""),
        mismatches.len(),
        mismatches.join("\n")
    );
    Ok(())
}

#[test]
fn scalars_match_python() -> TestResult {
    let dir = project("")?;
    let root = dir.path();
    let context = context(root)?;
    let zone = TimeZone::get(ZONE)?;
    compare_scalars(&["location"], root, &locations(), |input| {
        json!(location_to_file(input))
    })?;
    compare_scalars(&["issue_id"], root, &issue_ids(), |input| {
        json!(IssueId::normalize(input).as_ref().map(IssueId::as_str))
    })?;
    compare_scalars(&["timestamp"], root, &timestamps(), |input| {
        normalize_timestamp(Cow::Borrowed(input), &zone)
            .map_or(json!("!!"), |stamp| json!(stamp.as_str()))
    })?;
    let root_text = root.to_str().ok_or("path")?;
    let inputs = paths(&context.root);
    compare_scalars(&["relative", root_text], root, &inputs, |input| {
        json!(project_relative(input, &context, &FsResolver).as_str())
    })?;
    compare_scalars(&["memory", root_text], root, &inputs, |input| {
        json!(is_memory_path(input, &context, &FsResolver))
    })?;
    Ok(())
}

#[test]
fn character_classes_match_python() -> TestResult {
    let dir = tempfile::tempdir()?;
    let python = oracle(&["chars"], dir.path(), "")?;
    let mut lines = python.lines().map(serde_json::from_str::<Vec<u32>>);
    let every = || (0..=0x10_ffff_u32).filter_map(char::from_u32);
    let space: Vec<u32> = every()
        .filter(|c| is_python_space(*c))
        .map(u32::from)
        .collect();
    let decimal: Vec<u32> = every()
        .filter(|c| is_python_decimal(c.encode_utf8(&mut [0; 4])))
        .map(u32::from)
        .collect();
    let breaks: Vec<u32> = every()
        .filter(|c| python_lines(format!("a{c}b").as_bytes()).count() == 2)
        .map(u32::from)
        .collect();
    let differ = |python: Vec<u32>, rust: Vec<u32>| -> Vec<u32> {
        let python: BTreeSet<u32> = python.into_iter().collect();
        let rust: BTreeSet<u32> = rust.into_iter().collect();
        python.symmetric_difference(&rust).copied().collect()
    };
    assert_eq!(
        differ(lines.next().ok_or("isspace")??, space),
        Vec::<u32>::new()
    );
    assert_eq!(
        differ(lines.next().ok_or("isdecimal")??, decimal),
        (0x11de0..=0x11de9).collect::<Vec<u32>>(),
        "only the digits Unicode 17 added, which Python 3.15 knows and regex 1.13 does not"
    );
    assert_eq!(
        differ(lines.next().ok_or("splitlines")??, breaks),
        Vec::<u32>::new()
    );
    Ok(())
}
