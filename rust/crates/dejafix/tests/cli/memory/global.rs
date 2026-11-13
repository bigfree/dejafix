use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde_json::Value;

use crate::support::{Repo, TestResult, text};

const PATTERNS: &str = concat!(
    r#"{"confidence": "high", "created": "2020-01-01T00:00:00Z", "id": "pat_000000000001", "pattern": "old high pattern", "source_project": "alpha", "tags": ["python"], "times_encountered": 1}"#,
    "\n",
    r#"{"confidence": "medium", "created": "2020-01-01T00:00:00Z", "id": "pat_000000000002", "pattern": "old medium pattern", "source_project": null, "tags": [], "times_encountered": 1}"#,
    "\n",
    r#"{"confidence": "medium", "created": "2099-11-01T00:00:00Z", "id": "pat_000000000003", "pattern": "fresh unicode pattern \u0161\u010d\u0159", "source_project": "beta", "tags": ["rust", "web"], "times_encountered": 1}"#,
    "\n",
);

const GOTCHAS: &str = concat!(
    r#"{"discovered": "2099-01-01T00:00:00Z", "gotcha": "always close async sessions", "id": "got_000000000001", "library": "sqlalchemy", "source_project": "other", "tags": ["python"], "version_range": null}"#,
    "\n",
    r#"{"discovered": "2024-01-01T00:00:00Z", "gotcha": "fastapi deps are cached", "id": "got_000000000002", "library": "fastapi", "source_project": "other", "tags": ["python", "web"], "version_range": ">=0.100"}"#,
    "\n",
    r#"{"discovered": "2099-02-01T00:00:00Z", "gotcha": "unrelated lib thing", "id": "got_000000000003", "library": "gin", "source_project": "", "tags": [], "version_range": null}"#,
    "\n",
    r#"{"discovered": "2099-03-01T00:00:00Z", "gotcha": "pydantic v2 strictness", "id": "got_000000000004", "library": "pydantic", "tags": ["python"], "version_range": "2.x"}"#,
    "\n",
    r#"{"discovered": "2099-03-02T00:00:00Z", "gotcha": "with text key", "id": "got_000000000005", "library": "sqlalchemy", "summary": "summary wins when no text", "tags": [], "text": "a text key", "version_range": null}"#,
    "\n",
);

const LIST: &str = "
Patterns (3)
────────────────────────────────────────────────────────────
  pat_000000000001  old high pattern
    tags: python · from: alpha · confidence: high
  pat_000000000002  old medium pattern
    confidence: medium
  pat_000000000003  fresh unicode pattern ščř
    tags: rust, web · from: beta · confidence: medium

Library Gotchas (5)
────────────────────────────────────────────────────────────
  got_000000000001  [sqlalchemy] always close async sessions
    from: other
  got_000000000002  [fastapi] fastapi deps are cached
    version: >=0.100 · from: other
  got_000000000003  [gin] unrelated lib thing
  got_000000000004  [pydantic] pydantic v2 strictness
    version: 2.x
  got_000000000005  [sqlalchemy] with text key

";

const STATUS: &str = "
  ================================================
    dejafix Global Memory
  ================================================
  Location: <GLOBAL>

    Patterns:           3
    Library gotchas:     5
    Stack preferences:   yes
    Unique tags:         3
    Libraries tracked:   4
    Source projects:     3

  Tags: python, rust, web
  Libraries: fastapi, gin, pydantic, sqlalchemy
  Projects: alpha, beta, other
  ================================================

";

const DETECT: &str = "
Detected Stack: repo
──────────────────────────────────────────────────
  Tags:       fastapi, pydantic, python, sqlalchemy
  Libraries:  fastapi, pydantic, sqlalchemy
  Frameworks: fastapi
  Manifests:  pyproject.toml

  Would inherit:
    2 patterns, 4 library gotchas

";

const DETECT_JSON: &str = r#"{
  "name": "repo",
  "tags": [
    "fastapi",
    "pydantic",
    "python",
    "sqlalchemy"
  ],
  "libraries": [
    "fastapi",
    "pydantic",
    "sqlalchemy"
  ],
  "frameworks": [
    "fastapi"
  ],
  "manifest_files": [
    "pyproject.toml"
  ]
}
"#;

const UNKNOWN: &str = "Unknown action: frob\nAvailable: status, list, add, add-gotcha, remove, export, import, prune, detect\n";

fn global_dir(repo: &Repo) -> PathBuf {
    repo.registry().join("global")
}

fn seeded() -> Result<Repo, Box<dyn std::error::Error>> {
    let repo = Repo::new()?;
    let dir = global_dir(&repo);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("patterns.jsonl"), PATTERNS)?;
    fs::write(dir.join("library_gotchas.jsonl"), GOTCHAS)?;
    fs::write(dir.join("stack_preferences.md"), "# prefs\nuse uv\n")?;
    Ok(repo)
}

fn with_stack(repo: &Repo) -> TestResult {
    fs::write(
        repo.root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\ndependencies = [\"sqlalchemy>=2\", \"fastapi\", \"pydantic\"]\n",
    )?;
    Ok(())
}

fn read(repo: &Repo, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    text(&global_dir(repo).join(name))
}

fn run(repo: &Repo, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let mut full = vec!["global"];
    full.extend_from_slice(args);
    let assert = repo.djf(&full).assert().code(0);
    Ok(String::from_utf8(assert.get_output().stdout.clone())?)
}

fn ids(text: &str) -> String {
    Regex::new("(pat|got)_[0-9a-f]{12}")
        .map(|pattern| pattern.replace_all(text, "${1}_X").into_owned())
        .unwrap_or_default()
}

#[test]
fn status_matches_python_and_creates_the_directory() -> TestResult {
    let repo = seeded()?;
    let out = run(&repo, &["status"])?;
    assert_eq!(
        out,
        STATUS.replace("<GLOBAL>", &global_dir(&repo).display().to_string())
    );
    assert_eq!(run(&repo, &[])?, out);
    let fresh = Repo::new()?;
    assert!(!global_dir(&fresh).exists());
    let out = run(&fresh, &["status"])?;
    assert!(global_dir(&fresh).is_dir());
    assert!(out.contains("Patterns:           0\n"));
    assert!(out.contains("Stack preferences:   not set\n"));
    assert!(!out.contains("Tags:"));
    Ok(())
}

#[test]
fn list_matches_python_and_filters() -> TestResult {
    let repo = seeded()?;
    assert_eq!(run(&repo, &["list"])?, LIST);
    let by_library = run(&repo, &["list", "--library", "sql"])?;
    assert!(by_library.contains("Patterns (3)"));
    assert!(by_library.contains("Library Gotchas (2)"));
    assert!(by_library.contains("[sqlalchemy] with text key"));
    assert!(!by_library.contains("[gin]"));
    let by_tag = run(&repo, &["list", "--tag", "PYTHON"])?;
    assert!(by_tag.contains("Patterns (1)"));
    assert!(by_tag.contains("Library Gotchas (3)"));
    let none = run(&repo, &["list", "--tag", "zzz", "--library", "qqq"])?;
    assert_eq!(
        none,
        "No entries found. (filter: tag=zzz, library=qqq)\n  Add patterns:  djf global add \"your pattern here\"\n  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library>\n"
    );
    let empty = run(&Repo::new()?, &["list"])?;
    assert_eq!(
        empty,
        "No entries found.\n  Add patterns:  djf global add \"your pattern here\"\n  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library>\n"
    );
    Ok(())
}

#[test]
fn list_json_keeps_python_layout() -> TestResult {
    let repo = seeded()?;
    let out = run(&repo, &["list", "--tag", "python", "-f", "json"])?;
    let value: Value = serde_json::from_str(&out)?;
    assert_eq!(value["patterns"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["gotchas"].as_array().map(Vec::len), Some(3));
    assert!(out.starts_with("{\n  \"patterns\": [\n    {\n      \"confidence\": \"high\","));
    assert!(out.contains(
        "      \"source_project\": \"alpha\",\n      \"tags\": [\n        \"python\"\n      ],\n"
    ));
    assert!(out.contains("      \"version_range\": null\n"));
    assert!(out.ends_with("    }\n  ]\n}\n"));
    assert_eq!(
        out,
        run(&repo, &["list", "--tag", "python", "--format", "json"])?
    );
    let unicode = run(&repo, &["list", "--format", "json"])?;
    assert!(unicode.contains("fresh unicode pattern \\u0161\\u010d\\u0159"));
    assert_eq!(
        run(&Repo::new()?, &["list", "-f", "json"])?,
        "{\n  \"patterns\": [],\n  \"gotchas\": []\n}\n"
    );
    Ok(())
}

#[test]
fn add_writes_a_sorted_key_line_and_prints_its_id() -> TestResult {
    let repo = Repo::new()?;
    let out = run(&repo, &["add", "prefer small PRs", "--tag", "process"])?;
    assert_eq!(
        ids(&out),
        "[global] Pattern added: pat_X\n  prefer small PRs\n"
    );
    let line = read(&repo, "patterns.jsonl")?;
    let shape = Regex::new(
        r#"^\{"confidence": "medium", "created": "\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{6})?Z", "id": "pat_[0-9a-f]{12}", "pattern": "prefer small PRs", "source_project": "repo", "tags": \["process"\], "times_encountered": 1\}\n$"#,
    )?;
    assert!(shape.is_match(&line), "{line}");
    let again = run(&repo, &["add", "second"])?;
    assert!(again.contains("Pattern added: pat_"));
    let lines = read(&repo, "patterns.jsonl")?;
    assert_eq!(lines.lines().count(), 2);
    assert!(
        lines
            .lines()
            .nth(1)
            .is_some_and(|line| line.contains(r#""tags": []"#))
    );
    Ok(())
}

#[test]
fn add_with_a_library_routes_to_add_gotcha() -> TestResult {
    let repo = Repo::new()?;
    let out = run(
        &repo,
        &[
            "add",
            "close sessions",
            "--library",
            "SQLAlchemy",
            "--tag",
            "web",
        ],
    )?;
    assert_eq!(
        ids(&out),
        "[global] Gotcha added: got_X\n  [SQLAlchemy] close sessions\n"
    );
    assert!(!global_dir(&repo).join("patterns.jsonl").exists());
    let line = read(&repo, "library_gotchas.jsonl")?;
    let shape = Regex::new(
        r#"^\{"discovered": "[^"]+", "gotcha": "close sessions", "id": "got_[0-9a-f]{12}", "library": "sqlalchemy", "source_project": "repo", "tags": \["web"\], "version_range": null\}\n$"#,
    )?;
    assert!(shape.is_match(&line), "{line}");
    let explicit = run(&repo, &["add-gotcha", "expire keys", "--library", "Redis"])?;
    assert_eq!(
        ids(&explicit),
        "[global] Gotcha added: got_X\n  [Redis] expire keys\n"
    );
    Ok(())
}

#[test]
fn usage_lines_when_arguments_are_missing() -> TestResult {
    let repo = Repo::new()?;
    assert_eq!(
        run(&repo, &["add"])?,
        "Usage: djf global add \"your pattern here\" [--tag python]\n"
    );
    assert_eq!(
        run(&repo, &["add", ""])?,
        "Usage: djf global add \"your pattern here\" [--tag python]\n"
    );
    let gotcha =
        "Usage: djf global add-gotcha --library sqlalchemy \"gotcha text\" [--tag python]\n";
    assert_eq!(run(&repo, &["add-gotcha"])?, gotcha);
    assert_eq!(run(&repo, &["add-gotcha", "only text"])?, gotcha);
    assert_eq!(run(&repo, &["add-gotcha", "--library", "x"])?, gotcha);
    assert_eq!(run(&repo, &["add", "--library", "x"])?, gotcha);
    assert_eq!(run(&repo, &["remove"])?, "Usage: djf global remove <id>\n");
    assert_eq!(
        run(&repo, &["import"])?,
        "Usage: djf global import <file.json>\n"
    );
    assert!(!global_dir(&repo).exists());
    Ok(())
}

#[test]
fn remove_deletes_a_pattern_or_a_gotcha_and_rewrites_sorted() -> TestResult {
    let repo = seeded()?;
    let out = run(&repo, &["remove", "pat_000000000002"])?;
    assert_eq!(out, "[global] Removed: pat_000000000002\n");
    let mut expected: Vec<&str> = PATTERNS.lines().collect();
    expected.remove(1);
    assert_eq!(read(&repo, "patterns.jsonl")?, expected.join("\n") + "\n");
    let out = run(&repo, &["remove", "got_000000000003"])?;
    assert_eq!(out, "[global] Removed: got_000000000003\n");
    let kept = read(&repo, "library_gotchas.jsonl")?;
    assert_eq!(kept.lines().count(), 4);
    assert!(!kept.contains("got_000000000003"));
    assert_eq!(run(&repo, &["remove", "nope"])?, "Entry not found: nope\n");
    Ok(())
}

#[test]
fn add_then_remove_round_trips() -> TestResult {
    let repo = Repo::new()?;
    let added = run(&repo, &["add", "temporary"])?;
    let id = Regex::new("pat_[0-9a-f]{12}")?
        .find(&added)
        .map(|found| found.as_str().to_owned())
        .ok_or("no id printed")?;
    assert_eq!(
        run(&repo, &["remove", &id])?,
        format!("[global] Removed: {id}\n")
    );
    assert_eq!(read(&repo, "patterns.jsonl")?, "");
    let added = run(&repo, &["add-gotcha", "--library", "redis", "ttl"])?;
    let id = Regex::new("got_[0-9a-f]{12}")?
        .find(&added)
        .map(|found| found.as_str().to_owned())
        .ok_or("no id printed")?;
    assert_eq!(
        run(&repo, &["remove", &id])?,
        format!("[global] Removed: {id}\n")
    );
    assert_eq!(read(&repo, "library_gotchas.jsonl")?, "");
    Ok(())
}

#[test]
fn prune_by_confidence_then_by_age() -> TestResult {
    let repo = seeded()?;
    assert_eq!(
        run(
            &repo,
            &["prune", "--older-than", "365", "--confidence", "low"]
        )?,
        "No entries to prune.\n"
    );
    assert_eq!(
        run(
            &repo,
            &["prune", "--older-than", "365", "--confidence", "high"]
        )?,
        "[global] Pruned 1 entries (older than 365 days)\n"
    );
    assert!(!read(&repo, "patterns.jsonl")?.contains("pat_000000000001"));
    assert_eq!(
        run(&repo, &["prune"])?,
        "[global] Pruned 2 entries (older than 365 days)\n"
    );
    assert_eq!(read(&repo, "patterns.jsonl")?.lines().count(), 1);
    assert_eq!(read(&repo, "library_gotchas.jsonl")?.lines().count(), 4);
    assert_eq!(run(&repo, &["prune"])?, "No entries to prune.\n");
    Ok(())
}

#[test]
fn prune_treats_a_missing_date_as_ancient() -> TestResult {
    let repo = Repo::new()?;
    let dir = global_dir(&repo);
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("patterns.jsonl"),
        "{\"id\": \"pat_a\", \"pattern\": \"undated\"}\n{\"created\": \"junk\", \"id\": \"pat_b\", \"pattern\": \"junk\"}\n",
    )?;
    assert_eq!(
        run(&repo, &["prune", "--older-than", "3650"])?,
        "[global] Pruned 2 entries (older than 3650 days)\n"
    );
    Ok(())
}

#[test]
fn export_matches_python() -> TestResult {
    let repo = seeded()?;
    let out = run(&repo, &["export"])?;
    let value: Value = serde_json::from_str(&out)?;
    assert_eq!(value["version"], "1.0");
    assert_eq!(value["preferences"], "# prefs\nuse uv\n");
    assert_eq!(value["patterns"].as_array().map(Vec::len), Some(3));
    assert_eq!(value["gotchas"].as_array().map(Vec::len), Some(5));
    let stamp = Regex::new(r#"^  "exported_at": "\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{6})?Z",$"#)?;
    assert!(
        out.lines().nth(2).is_some_and(|line| stamp.is_match(line)),
        "{out}"
    );
    let head = "{\n  \"version\": \"1.0\",\n";
    assert!(out.starts_with(head));
    assert!(out.contains("      \"pattern\": \"fresh unicode pattern \\u0161\\u010d\\u0159\",\n"));
    assert!(out.ends_with("  \"preferences\": \"# prefs\\nuse uv\\n\"\n}\n"));
    let empty = run(&Repo::new()?, &["export"])?;
    assert!(
        empty.contains("  \"patterns\": [],\n  \"gotchas\": [],\n  \"preferences\": \"\"\n}\n")
    );
    Ok(())
}

fn write_json(repo: &Repo, name: &str, bytes: &[u8]) -> TestResult {
    fs::write(repo.root.join(name), bytes)?;
    Ok(())
}

#[test]
fn import_merges_by_id_and_keeps_existing_preferences() -> TestResult {
    let repo = seeded()?;
    write_json(
        &repo,
        "imp.json",
        r#"{"version": "1.0", "patterns": [{"id": "pat_000000000003", "pattern": "dup"}, {"id": "pat_imported0001", "pattern": "imported ščř", "tags": ["x"], "created": "2099-01-01T00:00:00Z"}], "gotchas": [{"id": "got_imported0001", "library": "zlib", "gotcha": "imp", "discovered": "2099-01-01T00:00:00Z"}], "preferences": "new prefs"}"#.as_bytes(),
    )?;
    assert_eq!(
        run(&repo, &["import", "imp.json"])?,
        "[global] Imported: 1 patterns, 1 gotchas\n"
    );
    let patterns = read(&repo, "patterns.jsonl")?;
    assert_eq!(patterns.lines().count(), 4);
    assert!(patterns.ends_with("{\"created\": \"2099-01-01T00:00:00Z\", \"id\": \"pat_imported0001\", \"pattern\": \"imported \\u0161\\u010d\\u0159\", \"tags\": [\"x\"]}\n"));
    assert!(!patterns.contains("\"dup\""));
    assert!(read(&repo, "library_gotchas.jsonl")?.ends_with("{\"discovered\": \"2099-01-01T00:00:00Z\", \"gotcha\": \"imp\", \"id\": \"got_imported0001\", \"library\": \"zlib\"}\n"));
    assert_eq!(read(&repo, "stack_preferences.md")?, "# prefs\nuse uv\n");
    assert_eq!(
        run(&repo, &["import", "imp.json"])?,
        "[global] Imported: 0 patterns, 0 gotchas\n"
    );
    assert_eq!(read(&repo, "patterns.jsonl")?, patterns);
    Ok(())
}

#[test]
fn import_writes_preferences_when_none_exist_and_reads_a_bom() -> TestResult {
    let repo = Repo::new()?;
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(
        br#"{"patterns": [{"id": "pat_bom", "pattern": "from bom"}], "gotchas": [], "preferences": "use uv\n"}"#,
    );
    write_json(&repo, "bom.json", &bom)?;
    assert_eq!(
        run(&repo, &["import", "bom.json"])?,
        "[global] Imported: 1 patterns, 0 gotchas\n"
    );
    assert_eq!(read(&repo, "stack_preferences.md")?, "use uv\n");
    assert_eq!(
        read(&repo, "patterns.jsonl")?,
        "{\"id\": \"pat_bom\", \"pattern\": \"from bom\"}\n"
    );
    Ok(())
}

#[test]
fn import_reports_a_missing_or_invalid_file() -> TestResult {
    let repo = Repo::new()?;
    assert_eq!(
        run(&repo, &["import", "nowhere.json"])?,
        "File not found: nowhere.json\n"
    );
    write_json(&repo, "bad.json", b"{\"patterns\": [\n")?;
    let bad = run(&repo, &["import", "bad.json"])?;
    assert!(bad.starts_with("Invalid JSON: "), "{bad}");
    assert!(bad.ends_with("line 2 column 0\n"), "{bad}");
    write_json(&repo, "empty.json", b"")?;
    assert!(run(&repo, &["import", "empty.json"])?.starts_with("Invalid JSON: "));
    write_json(&repo, "arr.json", b"[1, 2]")?;
    assert_eq!(
        run(&repo, &["import", "arr.json"])?,
        "Invalid JSON: top-level value is not an object\n"
    );
    Ok(())
}

#[test]
fn export_then_import_into_a_fresh_home_restores_everything() -> TestResult {
    let source = seeded()?;
    let exported = run(&source, &["export"])?;
    let target = Repo::new()?;
    write_json(&target, "all.json", exported.as_bytes())?;
    assert_eq!(
        run(&target, &["import", "all.json"])?,
        "[global] Imported: 3 patterns, 5 gotchas\n"
    );
    assert_eq!(read(&target, "patterns.jsonl")?, PATTERNS);
    assert_eq!(read(&target, "library_gotchas.jsonl")?, GOTCHAS);
    assert_eq!(read(&target, "stack_preferences.md")?, "# prefs\nuse uv\n");
    Ok(())
}

#[test]
fn detect_matches_python() -> TestResult {
    let repo = seeded()?;
    with_stack(&repo)?;
    assert_eq!(run(&repo, &["detect"])?, DETECT);
    assert_eq!(run(&repo, &["detect", "-f", "json"])?, DETECT_JSON);
    let bare = run(&Repo::new()?, &["detect"])?;
    assert_eq!(
        bare,
        "\nDetected Stack: repo\n──────────────────────────────────────────────────\n  Tags:       (none detected)\n  Libraries:  (none detected)\n  Manifests:  \n\n  No matching global entries for this stack.\n\n"
    );
    Ok(())
}

#[test]
fn detect_records_the_libraries_it_saw() -> TestResult {
    let repo = Repo::new()?;
    with_stack(&repo)?;
    run(&repo, &["detect"])?;
    let cache: Value = serde_json::from_str(&read(&repo, ".promotable.json")?)?;
    let libraries = cache["libraries"].as_array().ok_or("no libraries")?;
    assert!(libraries.iter().any(|library| library == "pydantic"));
    Ok(())
}

#[test]
fn an_unknown_action_is_reported_with_exit_zero() -> TestResult {
    let repo = Repo::new()?;
    assert_eq!(run(&repo, &["frob"])?, UNKNOWN);
    Ok(())
}

#[test]
fn global_works_outside_a_project() -> TestResult {
    let repo = Repo::bare()?;
    let out = run(&repo, &["status"])?;
    assert!(out.contains("dejafix Global Memory"));
    Ok(())
}

#[test]
fn a_corrupt_store_is_an_error_not_a_silent_loss() -> TestResult {
    let repo = Repo::new()?;
    let dir = global_dir(&repo);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("patterns.jsonl"), "{not json\n")?;
    let assert = repo.djf(&["global", "remove", "x"]).assert().code(3);
    let err = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(err.starts_with("Error: cannot read "), "{err}");
    assert_eq!(
        fs::read_to_string(dir.join("patterns.jsonl"))?,
        "{not json\n"
    );
    Ok(())
}

#[test]
fn colour_is_not_written_to_a_pipe() -> TestResult {
    let repo = seeded()?;
    for args in [
        &["status"][..],
        &["list"],
        &["add", "x"],
        &["remove", "pat_000000000001"],
        &["prune"],
    ] {
        assert!(!run(&repo, args)?.contains('\u{1b}'));
    }
    Ok(())
}
