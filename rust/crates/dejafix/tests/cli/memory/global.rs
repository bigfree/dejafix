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
Library Gotchas (5)
────────────────────────────────────────────────────────────
  got_000000000001 (not upgraded)  [sqlalchemy] always close async sessions
    from: other
  got_000000000002 (not upgraded)  [fastapi] fastapi deps are cached
    version: >=0.100 · from: other
  got_000000000003 (not upgraded)  [gin] unrelated lib thing
  got_000000000004 (not upgraded)  [pydantic] pydantic v2 strictness
    version: 2.x
  got_000000000005 (not upgraded)  [sqlalchemy] with text key

";

const STATUS: &str = "
  ================================================
    dejafix Global Memory
  ================================================
  Location: <GLOBAL>

    Library gotchas:     5
    Stack preferences:   yes
    Unique tags:         2
    Libraries tracked:   4
    Source projects:     1
    Not upgraded:        5

  Tags: python, web
  Libraries: fastapi, gin, pydantic, sqlalchemy
  Projects: other
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
    4 library gotchas

";

const NO_ENTRIES_HINT: &str = "  Add gotchas:   djf global add-gotcha \"gotcha text\" --library <library> --keyword <phrase> --keyword <phrase>\n";

const SKIPPED: &str = "cross-project pattern(s); dejafix no longer keeps them.\n";

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

const UNKNOWN: &str = "Unknown action: frob\nAvailable: status, list, add, add-gotcha, query, edit, upgrade, remove, export, import, prune, detect\n";

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
fn status_counts_version_one_records_and_creates_the_directory() -> TestResult {
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
    assert!(out.contains("Library gotchas:     0\n"));
    assert!(out.contains("Stack preferences:   not set\n"));
    assert!(!out.contains("Tags:"));
    assert_eq!(read(&repo, "patterns.jsonl")?, PATTERNS);
    Ok(())
}

#[test]
fn list_marks_version_one_records_and_filters() -> TestResult {
    let repo = seeded()?;
    assert_eq!(run(&repo, &["list"])?, LIST);
    let by_library = run(&repo, &["list", "--library", "sql"])?;
    assert!(by_library.contains("Library Gotchas (2)"));
    assert!(by_library.contains("[sqlalchemy] with text key"));
    assert!(!by_library.contains("[gin]"));
    let by_tag = run(&repo, &["list", "--tag", "PYTHON"])?;
    assert!(by_tag.contains("Library Gotchas (3)"));
    let none = run(&repo, &["list", "--tag", "zzz", "--library", "qqq"])?;
    assert_eq!(
        none,
        format!("No entries found. (filter: tag=zzz, library=qqq)\n{NO_ENTRIES_HINT}")
    );
    let empty = run(&Repo::new()?, &["list"])?;
    assert_eq!(empty, format!("No entries found.\n{NO_ENTRIES_HINT}"));
    Ok(())
}

#[test]
fn a_version_two_record_is_listed_without_the_marker() -> TestResult {
    let repo = Repo::new()?;
    let dir = global_dir(&repo);
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("library_gotchas.jsonl"),
        concat!(
            r#"{"v": 2, "id": "got_000000000009", "library": "bullmq", "keywords": ["shutdown", "worker close"], "versions": ">=6.2.0 <6.3", "gotcha": "Worker.close() never resolves against an unreachable Redis", "source_project": "project-a", "discovered": "2026-08-27", "reviewed": "2026-11-14"}"#,
            "\n"
        ),
    )?;
    let list = run(&repo, &["list"])?;
    assert!(list.contains(
        "  got_000000000009  [bullmq] Worker.close() never resolves against an unreachable Redis\n    keywords: shutdown, worker close · version: >=6.2.0 <6.3 · from: project-a\n"
    ));
    assert!(!run(&repo, &["status"])?.contains("Not upgraded"));
    Ok(())
}

#[test]
fn list_json_keeps_python_layout() -> TestResult {
    let repo = seeded()?;
    let out = run(&repo, &["list", "--tag", "python", "-f", "json"])?;
    let value: Value = serde_json::from_str(&out)?;
    assert!(value.get("patterns").is_none());
    assert_eq!(value["gotchas"].as_array().map(Vec::len), Some(3));
    assert!(out.starts_with(
        "{\n  \"gotchas\": [\n    {\n      \"discovered\": \"2099-01-01T00:00:00Z\","
    ));
    assert!(out.contains(
        "      \"source_project\": \"other\",\n      \"tags\": [\n        \"python\"\n      ],\n"
    ));
    assert!(out.contains("      \"version_range\": null\n"));
    assert!(out.ends_with("    }\n  ]\n}\n"));
    assert_eq!(
        out,
        run(&repo, &["list", "--tag", "python", "--format", "json"])?
    );
    assert_eq!(
        run(&Repo::new()?, &["list", "-f", "json"])?,
        "{\n  \"gotchas\": []\n}\n"
    );
    Ok(())
}

#[test]
fn add_without_a_library_asks_for_a_gotcha() -> TestResult {
    let repo = Repo::new()?;
    let gotcha = "Usage: djf global add-gotcha \"<text>\" --library <name> --keyword <phrase> --keyword <phrase> [--versions <range>]\nWrite one lesson in English, 30 to 600 characters, without project details; give 2 to 8 keywords.\n";
    assert_eq!(
        refused(&repo, &["add", "prefer small PRs", "--tag", "process"])?,
        gotcha
    );
    assert!(!global_dir(&repo).join("patterns.jsonl").exists());
    Ok(())
}

const LESSON: &str =
    "SQLAlchemy async sessions stay open after a request unless the handler closes them.";

fn refused(repo: &Repo, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let mut full = vec!["global"];
    full.extend_from_slice(args);
    let assert = repo.djf(&full).assert().code(1);
    assert_eq!(assert.get_output().stdout, b"");
    Ok(String::from_utf8(assert.get_output().stderr.clone())?)
}

#[test]
fn add_gotcha_writes_a_version_two_record_and_the_library() -> TestResult {
    let repo = Repo::new()?;
    let out = run(
        &repo,
        &[
            "add",
            LESSON,
            "--library",
            "SQLAlchemy",
            "--keyword",
            "async session",
            "--keyword",
            "request  cleanup",
            "--versions",
            ">=2.0",
        ],
    )?;
    assert_eq!(
        ids(&out),
        format!("[global] Gotcha added: got_X\n  [sqlalchemy] {LESSON}\n")
    );
    assert!(!global_dir(&repo).join("patterns.jsonl").exists());
    let line = read(&repo, "library_gotchas.jsonl")?;
    let shape = Regex::new(&format!(
        r#"^\{{"v": 2, "id": "got_[0-9a-f]{{12}}", "library": "sqlalchemy", "keywords": \["async session", "request cleanup"\], "versions": ">=2.0", "gotcha": "{}", "source_project": "repo", "discovered": "\d{{4}}-\d\d-\d\d"\}}\n$"#,
        regex::escape(LESSON)
    ))?;
    assert!(shape.is_match(&line), "{line}");
    assert_eq!(
        read(&repo, "libraries.json")?,
        "{\n  \"sqlalchemy\": []\n}\n"
    );
    let again = refused(
        &repo,
        &[
            "add-gotcha",
            LESSON,
            "--library",
            "sqlalchemy",
            "--keyword",
            "a",
            "--keyword",
            "b",
        ],
    )?;
    assert!(
        ids(&again).starts_with("Not added: it repeats got_X, which already holds this lesson.\n"),
        "{again}"
    );
    Ok(())
}

#[test]
fn a_refused_gotcha_lists_every_failure_and_writes_nothing() -> TestResult {
    let repo = Repo::new()?;
    let err = refused(
        &repo,
        &[
            "add-gotcha",
            "See SHOP-3233: the config in apps/heating/ breaks",
            "--library",
            "nx",
            "--keyword",
            "only one",
        ],
    )?;
    assert_eq!(
        err,
        "Not added: give 2 to 8 keywords, not 1; it names a project path (apps/heating/); it names a ticket number (SHOP-3233).\n"
    );
    assert!(!global_dir(&repo).join("library_gotchas.jsonl").exists());
    assert!(!global_dir(&repo).join("libraries.json").exists());
    let warned = run(
        &repo,
        &[
            "add-gotcha",
            "Retry of the nx affected command failed because the daemon cached the graph.",
            "--library",
            "nx",
            "--keyword",
            "daemon",
            "--keyword",
            "project graph",
        ],
    )?;
    assert!(
        warned.ends_with("\n  Warning: it reads like a run status ('retry of')\n"),
        "{warned}"
    );
    Ok(())
}

#[test]
fn usage_lines_when_arguments_are_missing() -> TestResult {
    let repo = Repo::new()?;
    let gotcha = "Usage: djf global add-gotcha \"<text>\" --library <name> --keyword <phrase> --keyword <phrase> [--versions <range>]\nWrite one lesson in English, 30 to 600 characters, without project details; give 2 to 8 keywords.\n";
    assert_eq!(refused(&repo, &["add"])?, gotcha);
    assert_eq!(refused(&repo, &["add", ""])?, gotcha);
    assert_eq!(refused(&repo, &["add-gotcha"])?, gotcha);
    assert_eq!(refused(&repo, &["add-gotcha", "only text"])?, gotcha);
    assert_eq!(refused(&repo, &["add-gotcha", "--library", "x"])?, gotcha);
    assert_eq!(refused(&repo, &["add", "--library", "x"])?, gotcha);
    assert_eq!(
        refused(
            &repo,
            &["add-gotcha", "text without keywords", "--library", "x"]
        )?,
        gotcha
    );
    assert_eq!(run(&repo, &["remove"])?, "Usage: djf global remove <id>\n");
    assert_eq!(
        run(&repo, &["import"])?,
        "Usage: djf global import <file.json>\n"
    );
    assert!(!global_dir(&repo).exists());
    Ok(())
}

#[test]
fn remove_deletes_a_gotcha_and_leaves_patterns_alone() -> TestResult {
    let repo = seeded()?;
    assert_eq!(
        run(&repo, &["remove", "pat_000000000002"])?,
        "Entry not found: pat_000000000002\n"
    );
    assert_eq!(read(&repo, "patterns.jsonl")?, PATTERNS);
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
    let added = run(
        &repo,
        &[
            "add-gotcha",
            "--library",
            "redis",
            "Redis EXPIRE on a key that already has a TTL replaces the old TTL.",
            "--keyword",
            "ttl",
            "--keyword",
            "expire",
        ],
    )?;
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
fn prune_ignores_confidence_for_gotchas_then_prunes_by_age() -> TestResult {
    let repo = seeded()?;
    assert_eq!(
        run(
            &repo,
            &["prune", "--older-than", "365", "--confidence", "high"]
        )?,
        "No entries to prune.\n"
    );
    assert_eq!(
        run(&repo, &["prune"])?,
        "[global] Pruned 1 entries (older than 365 days)\n"
    );
    assert_eq!(read(&repo, "library_gotchas.jsonl")?.lines().count(), 4);
    assert!(!read(&repo, "library_gotchas.jsonl")?.contains("got_000000000002"));
    assert_eq!(read(&repo, "patterns.jsonl")?, PATTERNS);
    assert_eq!(run(&repo, &["prune"])?, "No entries to prune.\n");
    Ok(())
}

#[test]
fn prune_treats_a_missing_date_as_ancient() -> TestResult {
    let repo = Repo::new()?;
    let dir = global_dir(&repo);
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("library_gotchas.jsonl"),
        "{\"gotcha\": \"undated\", \"id\": \"got_a\", \"library\": \"x\"}\n{\"discovered\": \"junk\", \"gotcha\": \"junk\", \"id\": \"got_b\", \"library\": \"x\"}\n",
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
    assert!(value.get("patterns").is_none());
    assert_eq!(value["gotchas"].as_array().map(Vec::len), Some(5));
    let stamp = Regex::new(r#"^  "exported_at": "\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{6})?Z",$"#)?;
    assert!(
        out.lines().nth(2).is_some_and(|line| stamp.is_match(line)),
        "{out}"
    );
    let head = "{\n  \"version\": \"1.0\",\n";
    assert!(out.starts_with(head));
    assert!(out.ends_with("  \"preferences\": \"# prefs\\nuse uv\\n\"\n}\n"));
    let empty = run(&Repo::new()?, &["export"])?;
    assert!(empty.contains("  \"gotchas\": [],\n  \"preferences\": \"\"\n}\n"));
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
        r#"{"version": "1.0", "patterns": [{"id": "pat_000000000003", "pattern": "dup"}, {"id": "pat_imported0001", "pattern": "imported ščř", "tags": ["x"], "created": "2099-01-01T00:00:00Z"}], "gotchas": [{"id": "got_imported0001", "library": "zlib", "gotcha": "imp ščř", "discovered": "2099-01-01T00:00:00Z"}], "preferences": "new prefs"}"#.as_bytes(),
    )?;
    assert_eq!(
        run(&repo, &["import", "imp.json"])?,
        format!("[global] Imported: 1 gotchas\n  Skipped 2 {SKIPPED}")
    );
    assert_eq!(read(&repo, "patterns.jsonl")?, PATTERNS);
    assert!(read(&repo, "library_gotchas.jsonl")?.ends_with("{\"discovered\": \"2099-01-01T00:00:00Z\", \"gotcha\": \"imp \\u0161\\u010d\\u0159\", \"id\": \"got_imported0001\", \"library\": \"zlib\"}\n"));
    assert_eq!(read(&repo, "stack_preferences.md")?, "# prefs\nuse uv\n");
    assert_eq!(
        run(&repo, &["import", "imp.json"])?,
        format!("[global] Imported: 0 gotchas\n  Skipped 2 {SKIPPED}")
    );
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
        format!("[global] Imported: 0 gotchas\n  Skipped 1 {SKIPPED}")
    );
    assert_eq!(read(&repo, "stack_preferences.md")?, "use uv\n");
    assert!(!global_dir(&repo).join("patterns.jsonl").exists());
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
        "[global] Imported: 5 gotchas\n"
    );
    assert!(!global_dir(&target).join("patterns.jsonl").exists());
    assert_eq!(read(&target, "library_gotchas.jsonl")?, GOTCHAS);
    assert_eq!(read(&target, "stack_preferences.md")?, "# prefs\nuse uv\n");
    Ok(())
}

#[test]
fn export_then_import_restores_library_aliases() -> TestResult {
    let source = seeded()?;
    fs::write(
        global_dir(&source).join("libraries.json"),
        "{\n  \"next\": [\n    \"nextjs\"\n  ]\n}\n",
    )?;
    let exported = run(&source, &["export"])?;
    assert!(
        exported.ends_with(
            "  \"preferences\": \"# prefs\\nuse uv\\n\",\n  \"libraries\": {\n    \"next\": [\n      \"nextjs\"\n    ]\n  }\n}\n"
        ),
        "{exported}"
    );
    let target = Repo::new()?;
    write_json(&target, "all.json", exported.as_bytes())?;
    assert_eq!(
        run(&target, &["import", "all.json"])?,
        "[global] Imported: 5 gotchas\n"
    );
    assert_eq!(
        read(&target, "libraries.json")?,
        "{\n  \"next\": [\n    \"nextjs\"\n  ]\n}\n"
    );
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
fn detect_no_longer_writes_a_promotion_cache() -> TestResult {
    let repo = Repo::new()?;
    with_stack(&repo)?;
    assert!(run(&repo, &["detect"])?.starts_with("\nDetected Stack: repo\n"));
    assert!(!global_dir(&repo).join(".promotable.json").exists());
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
    fs::write(dir.join("library_gotchas.jsonl"), "{not json\n")?;
    let assert = repo.djf(&["global", "remove", "x"]).assert().code(3);
    let err = String::from_utf8(assert.get_output().stderr.clone())?;
    assert!(err.starts_with("Error: cannot read "), "{err}");
    assert_eq!(
        fs::read_to_string(dir.join("library_gotchas.jsonl"))?,
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
        &["remove", "got_000000000001"],
        &["remove", "pat_000000000001"],
        &["prune"],
    ] {
        assert!(!run(&repo, args)?.contains('\u{1b}'));
    }
    Ok(())
}

#[test]
fn query_ranks_global_gotchas_and_checks_the_budget() -> TestResult {
    let repo = Repo::new()?;
    run(
        &repo,
        &[
            "add-gotcha",
            LESSON,
            "--library",
            "sqlalchemy",
            "--keyword",
            "async session",
            "--keyword",
            "request cleanup",
        ],
    )?;
    let found = run(&repo, &["query", "sessions left open after a request"])?;
    assert!(
        ids(&found).starts_with(
            "Global gotchas for \"sessions left open after a request\" (1 of 1 shown):\n- got_X · sqlalchemy · from repo\n"
        ),
        "{found}"
    );
    assert_eq!(
        run(&repo, &["query", "kubernetes ingress"])?,
        "No global gotcha matches this query.\nLibraries with records: sqlalchemy (1).\n"
    );
    let listed = run(
        &repo,
        &["query", "--library", "sqlalchemy", "--budget", "500"],
    )?;
    assert!(
        listed.starts_with("Global gotchas of sqlalchemy (1 of 1 shown):"),
        "{listed}"
    );
    repo.djf(&["global", "query", "x", "--budget", "100"])
        .assert()
        .code(2);
    Ok(())
}

const UPGRADE_STORE: &str = concat!(
    r#"{"discovered": "2026-08-01T09:00:00Z", "gotcha": "Session zůstává otevřená, pokud ji handler nezavře.", "id": "got_000000000001", "library": "SQLAlchemy", "source_project": "alpha", "tags": ["python"], "version_range": ">=2.0"}"#,
    "\n",
    r#"{"discovered": "2026-08-02T09:00:00Z", "gotcha": "Worker.close() hangs in apps/heating/queue.ts when Redis is down.", "id": "got_000000000002", "library": "bullmq", "source_project": "home-log", "tags": []}"#,
    "\n",
    r#"{"v": 2, "id": "got_000000000003", "library": "zod", "keywords": ["strict", "unknown keys"], "versions": null, "gotcha": "Zod parse results drop unknown keys unless strict is used.", "discovered": "2026-01-01", "reviewed": "2026-02-02"}"#,
    "\n",
);

fn upgrade_repo() -> Result<Repo, Box<dyn std::error::Error>> {
    let repo = Repo::new()?;
    fs::create_dir_all(global_dir(&repo))?;
    fs::write(
        global_dir(&repo).join("library_gotchas.jsonl"),
        UPGRADE_STORE,
    )?;
    Ok(repo)
}

fn drafts_path(repo: &Repo) -> PathBuf {
    repo.root.join("drafts.jsonl")
}

fn export_drafts(repo: &Repo) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    run(repo, &["upgrade", "--export", "drafts.jsonl"])?;
    text(&drafts_path(repo))?
        .lines()
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

fn write_drafts(repo: &Repo, lines: &[Value]) -> TestResult {
    let mut out = String::new();
    for line in lines {
        out.push_str(&serde_json::to_string(line)?);
        out.push('\n');
    }
    fs::write(drafts_path(repo), out)?;
    Ok(())
}

fn fill(drafts: &mut [Value], id: &str, fields: &Value) -> TestResult {
    let draft = drafts
        .iter_mut()
        .find(|draft| draft["id"] == id)
        .ok_or("no such draft")?;
    for (key, value) in fields.as_object().ok_or("fields are not an object")? {
        draft[key] = value.clone();
    }
    Ok(())
}

fn store_text(repo: &Repo) -> Result<String, Box<dyn std::error::Error>> {
    read(repo, "library_gotchas.jsonl")
}

#[test]
fn upgrade_export_lists_only_version_one_records_with_their_notes() -> TestResult {
    let repo = upgrade_repo()?;
    let out = run(&repo, &["upgrade", "--export", "drafts.jsonl"])?;
    assert_eq!(
        out,
        "[global] Drafts written: 2 records to drafts.jsonl\n  Let the agent fill them, review them, then: djf global upgrade --apply drafts.jsonl\n"
    );
    let lines = text(&drafts_path(&repo))?;
    assert!(lines.contains("\"original\":\"Worker.close() hangs"));
    let drafts = export_drafts_from(&lines)?;
    assert_eq!(drafts.len(), 3);
    assert!(
        drafts[0]["instructions"]
            .as_str()
            .is_some_and(|text| text.contains("English"))
    );
    assert_eq!(drafts[1]["id"], "got_000000000001");
    assert_eq!(drafts[1]["library"], "SQLAlchemy");
    assert_eq!(drafts[1]["source_project"], "alpha");
    assert_eq!(drafts[1]["discovered"], "2026-08-01T09:00:00Z");
    assert_eq!(drafts[1]["versions"], ">=2.0");
    assert_eq!(drafts[1]["gotcha"], "");
    assert_eq!(drafts[1]["keywords"], serde_json::json!([]));
    assert_eq!(drafts[1]["drop"], false);
    let checks = |draft: &Value| draft["checks"].to_string();
    assert!(checks(&drafts[1]).contains("not English"), "{drafts:?}");
    assert!(!checks(&drafts[1]).contains("keyword"));
    assert!(checks(&drafts[2]).contains("apps/heating/queue.ts"));
    assert!(!lines.contains("got_000000000003"));
    assert_eq!(store_text(&repo)?, UPGRADE_STORE);
    Ok(())
}

fn export_drafts_from(text: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    text.lines()
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

#[test]
fn upgrade_export_never_overwrites_an_existing_file() -> TestResult {
    let repo = upgrade_repo()?;
    fs::write(drafts_path(&repo), "filled by the agent\n")?;
    assert_eq!(
        refused(&repo, &["upgrade", "--export", "drafts.jsonl"])?,
        "Not written: drafts.jsonl already exists; move it away or choose another name.\n"
    );
    assert_eq!(text(&drafts_path(&repo))?, "filled by the agent\n");
    Ok(())
}

#[test]
fn upgrade_needs_exactly_one_of_export_and_apply() -> TestResult {
    let repo = upgrade_repo()?;
    let usage = "Usage: djf global upgrade --export <file> | djf global upgrade --apply <file>\n";
    assert_eq!(refused(&repo, &["upgrade"])?, usage);
    assert_eq!(
        refused(&repo, &["upgrade", "--export", "a", "--apply", "b"])?,
        usage
    );
    assert_eq!(
        refused(&repo, &["upgrade", "--apply", "missing.jsonl"])?,
        "File not found: missing.jsonl\n"
    );
    assert_eq!(store_text(&repo)?, UPGRADE_STORE);
    Ok(())
}

#[test]
fn upgrade_apply_replaces_drops_and_backs_up_the_version_one_store() -> TestResult {
    let repo = upgrade_repo()?;
    let mut drafts = export_drafts(&repo)?;
    fill(
        &mut drafts,
        "got_000000000001",
        &serde_json::json!({
            "library": "SQLAlchemy ",
            "gotcha": LESSON,
            "keywords": ["async session", "request cleanup"],
        }),
    )?;
    fill(
        &mut drafts,
        "got_000000000002",
        &serde_json::json!({"drop": true}),
    )?;
    write_drafts(&repo, &drafts)?;
    let out = run(&repo, &["upgrade", "--apply", "drafts.jsonl"])?;
    assert_eq!(out, "[global] Upgrade: 1 replaced, 1 dropped, 0 kept\n");
    let lines: Vec<String> = store_text(&repo)?.lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2);
    let pattern = Regex::new(&format!(
        r#"^\{{"v": 2, "id": "got_000000000001", "library": "sqlalchemy", "keywords": \["async session", "request cleanup"\], "versions": ">=2.0", "gotcha": "{}", "source_project": "alpha", "discovered": "2026-08-01T09:00:00Z", "reviewed": "\d{{4}}-\d\d-\d\d", "tags": \["python"\]\}}$"#,
        regex::escape(LESSON)
    ))?;
    assert!(pattern.is_match(&lines[0]), "{}", lines[0]);
    assert_eq!(
        format!("{}\n", lines[1]),
        UPGRADE_STORE
            .lines()
            .nth(2)
            .map(|line| format!("{line}\n"))
            .unwrap_or_default()
    );
    assert_eq!(read(&repo, "library_gotchas.v1.jsonl")?, UPGRADE_STORE);
    let libraries: Value = serde_json::from_str(&read(&repo, "libraries.json")?)?;
    assert_eq!(libraries["sqlalchemy"], serde_json::json!([]));
    Ok(())
}

#[test]
fn applying_the_same_file_again_changes_nothing_and_keeps_the_backup() -> TestResult {
    let repo = upgrade_repo()?;
    let mut drafts = export_drafts(&repo)?;
    fill(
        &mut drafts,
        "got_000000000001",
        &serde_json::json!({"gotcha": LESSON, "keywords": ["async session", "request cleanup"]}),
    )?;
    fill(
        &mut drafts,
        "got_000000000002",
        &serde_json::json!({"drop": true}),
    )?;
    write_drafts(&repo, &drafts)?;
    run(&repo, &["upgrade", "--apply", "drafts.jsonl"])?;
    let store = store_text(&repo)?;
    let libraries = read(&repo, "libraries.json")?;
    let backup = global_dir(&repo).join("library_gotchas.v1.jsonl");
    let stamp = fs::metadata(&backup)?.modified()?;
    let second = run(&repo, &["upgrade", "--apply", "drafts.jsonl"])?;
    assert_eq!(
        second,
        "[global] Upgrade: 0 replaced, 0 dropped, 2 kept\n  got_000000000001: the record is already version 2\n  got_000000000002: no record has this id\n"
    );
    assert_eq!(store_text(&repo)?, store);
    assert_eq!(read(&repo, "libraries.json")?, libraries);
    assert_eq!(read(&repo, "library_gotchas.v1.jsonl")?, UPGRADE_STORE);
    assert_eq!(fs::metadata(&backup)?.modified()?, stamp);
    Ok(())
}

#[test]
fn a_draft_failing_a_hard_check_keeps_the_record_and_is_listed() -> TestResult {
    let repo = upgrade_repo()?;
    let mut drafts = export_drafts(&repo)?;
    fill(
        &mut drafts,
        "got_000000000001",
        &serde_json::json!({"gotcha": LESSON, "keywords": ["async session", "request cleanup"]}),
    )?;
    fill(
        &mut drafts,
        "got_000000000002",
        &serde_json::json!({"gotcha": "Too short.", "keywords": ["only one"]}),
    )?;
    write_drafts(&repo, &drafts)?;
    let out = run(&repo, &["upgrade", "--apply", "drafts.jsonl"])?;
    assert_eq!(
        out,
        "[global] Upgrade: 1 replaced, 0 dropped, 1 kept\n  got_000000000002: give 2 to 8 keywords, not 1; the gotcha must be 30 to 600 characters long, not 10\n"
    );
    let lines: Vec<String> = store_text(&repo)?.lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with(r#"{"v": 2, "id": "got_000000000001""#));
    assert_eq!(
        format!("{}\n", lines[1]),
        UPGRADE_STORE
            .lines()
            .nth(1)
            .map(|line| format!("{line}\n"))
            .unwrap_or_default()
    );
    Ok(())
}

#[test]
fn a_malformed_draft_line_is_reported_and_nothing_is_written() -> TestResult {
    let repo = upgrade_repo()?;
    let mut drafts = export_drafts(&repo)?;
    fill(
        &mut drafts,
        "got_000000000001",
        &serde_json::json!({"gotcha": LESSON, "keywords": ["async session", "request cleanup"]}),
    )?;
    write_drafts(&repo, &drafts)?;
    let mut broken = text(&drafts_path(&repo))?;
    broken.push_str("{\"id\": \"got_000000000002\", \"keywords\": \"one\"}\n");
    broken.push_str("not json at all\n");
    fs::write(drafts_path(&repo), broken)?;
    let err = refused(&repo, &["upgrade", "--apply", "drafts.jsonl"])?;
    assert!(
        err.starts_with("Draft file not applied, nothing was written:\n"),
        "{err}"
    );
    assert!(
        err.contains("  line 4: the field 'keywords' has the wrong type\n"),
        "{err}"
    );
    assert!(err.contains("  line 5: not valid JSON"), "{err}");
    assert_eq!(store_text(&repo)?, UPGRADE_STORE);
    assert!(!global_dir(&repo).join("library_gotchas.v1.jsonl").exists());
    assert!(!global_dir(&repo).join("libraries.json").exists());
    Ok(())
}

fn added_id(repo: &Repo) -> Result<String, Box<dyn std::error::Error>> {
    let out = run(
        repo,
        &[
            "add-gotcha",
            LESSON,
            "--library",
            "sqlalchemy",
            "--keyword",
            "async session",
            "--keyword",
            "request cleanup",
        ],
    )?;
    Ok(Regex::new("got_[0-9a-f]{12}")?
        .find(&out)
        .map(|found| found.as_str().to_owned())
        .ok_or("no id printed")?)
}

#[test]
fn edit_changes_keywords_versions_and_library_and_rechecks() -> TestResult {
    let repo = Repo::new()?;
    let id = added_id(&repo)?;
    let out = run(
        &repo,
        &[
            "edit",
            &id,
            "--keyword",
            "session leak",
            "--keyword",
            "fastapi request",
            "--versions",
            ">=2.0",
            "--alias",
            "sqlalchemy=sqla",
        ],
    )?;
    assert_eq!(
        out,
        format!("[global] Gotcha edited: {id}\n[global] Alias added: sqlalchemy=sqla\n")
    );
    let line = read(&repo, "library_gotchas.jsonl")?;
    assert!(
        line.contains(r#""keywords": ["session leak", "fastapi request"], "versions": ">=2.0""#),
        "{line}"
    );
    assert!(line.contains(r#""reviewed": "#), "{line}");
    assert!(read(&repo, "libraries.json")?.contains("\"sqla\""));
    let found = run(&repo, &["query", "", "--library", "sqla"])?;
    assert!(
        found.starts_with("Global gotchas of sqla (1 of 1 shown):"),
        "{found}"
    );
    let rejected = refused(&repo, &["edit", &id, "--keyword", "only one"])?;
    assert_eq!(rejected, "Not changed: give 2 to 8 keywords, not 1.\n");
    assert_eq!(read(&repo, "library_gotchas.jsonl")?, line);
    assert_eq!(
        refused(&repo, &["edit", "got_000000000000", "--versions", "1"])?,
        "Not changed: no record has the id got_000000000000.\n"
    );
    assert_eq!(
        refused(&repo, &["edit", "--alias", "nonsense"])?,
        "Not changed: 'nonsense' is not <library>=<alias>.\n"
    );
    assert_eq!(
        refused(&repo, &["edit", "--alias", "redis=sqla"])?,
        "Not changed: 'sqla' is already an alias of sqlalchemy.\n"
    );
    assert!(refused(&repo, &["edit"])?.starts_with("Usage: djf global edit <id>"));
    Ok(())
}

#[test]
fn a_version_one_record_must_be_upgraded_before_an_edit() -> TestResult {
    let repo = seeded()?;
    assert_eq!(
        refused(&repo, &["edit", "got_000000000001", "--versions", "2"])?,
        "Not changed: got_000000000001 is not upgraded yet; run `djf global upgrade` first.\n"
    );
    Ok(())
}
