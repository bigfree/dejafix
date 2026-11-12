use super::{
    Change, capture_event, capture_files, classify, first_line, location_rank, meets_threshold,
    merge_event, parse_name_status, pick_location,
};
use crate::core::event::{CaptureSource, Confidence, EventKind, Outcome};
use crate::core::time::Timestamp;
use crate::testing::{SeqIds, fixed_clock};

fn is_memory(path: &str) -> bool {
    path.split('/').next() == Some(".dejafix")
}

fn changes(entries: &[(char, &str)]) -> Vec<Change> {
    entries
        .iter()
        .map(|(status, path)| Change {
            status: *status,
            path: (*path).to_owned(),
        })
        .collect()
}

fn located(entries: &[(char, &str)]) -> Option<String> {
    let list = changes(entries);
    pick_location(&list, is_memory).map(str::to_owned)
}

fn added(paths: &[&str]) -> Vec<Change> {
    paths
        .iter()
        .map(|path| Change {
            status: 'A',
            path: (*path).to_owned(),
        })
        .collect()
}

fn location_of(paths: &[&str]) -> Option<String> {
    let list = added(paths);
    pick_location(&list, is_memory).map(str::to_owned)
}

#[test]
fn parse_name_status_reads_every_field_shape() {
    let parsed = parse_name_status(b"M\0src/a.py\0R100\0old.py\0new.py\0A\0src/caf\xc3\xa9.py\0");
    assert_eq!(
        parsed,
        changes(&[
            ('M', "src/a.py"),
            ('R', "new.py"),
            ('A', "src/caf\u{e9}.py")
        ])
    );
}

#[test]
fn parse_name_status_copies_take_the_new_path_too() {
    assert_eq!(
        parse_name_status(b"C075\0a.py\0b.py\0D\0gone.py\0"),
        changes(&[('C', "b.py"), ('D', "gone.py")])
    );
}

#[test]
fn parse_name_status_stops_where_python_stops() {
    assert_eq!(parse_name_status(b""), Vec::new());
    assert_eq!(parse_name_status(b"M\0"), changes(&[('M', "")]));
    assert_eq!(parse_name_status(b"M\0a.py"), changes(&[('M', "a.py")]));
    assert_eq!(parse_name_status(b"R100\0old.py\0"), changes(&[('R', "")]));
    assert_eq!(
        parse_name_status(b"M\0a.py\0\0b.py\0"),
        changes(&[('M', "a.py")])
    );
}

#[test]
fn parse_name_status_replaces_invalid_utf8() {
    let parsed = parse_name_status(b"A\0src/caf\xe9.py\0");
    assert_eq!(parsed, changes(&[('A', "src/caf\u{fffd}.py")]));
}

#[test]
fn classify_names_each_of_the_seven_kinds() {
    let revert = classify("Revert \"x\"").unwrap();
    assert_eq!(revert.kind, EventKind::Attempt);
    assert_eq!(revert.outcome, Some(Outcome::Failed));
    assert_eq!(revert.prefix, "Reverted");
    assert_eq!(revert.confidence, Confidence::High);

    let fix = classify("fix: guard the login").unwrap();
    assert_eq!(
        (fix.kind, fix.outcome, fix.prefix, fix.confidence),
        (EventKind::Fix, None, "Fix", Confidence::High)
    );

    let breaking = classify("tidy things\n\nBREAKING CHANGE: the api moved").unwrap();
    assert_eq!(
        (breaking.kind, breaking.prefix, breaking.confidence),
        (EventKind::Decision, "Breaking change", Confidence::High)
    );

    let feature = classify("feat(ui): add a panel").unwrap();
    assert_eq!(
        (feature.kind, feature.prefix, feature.confidence),
        (EventKind::Note, "New feature", Confidence::Medium)
    );

    let refactor = classify("refactor: split the loader").unwrap();
    assert_eq!(
        (refactor.kind, refactor.prefix, refactor.confidence),
        (EventKind::Decision, "Refactor", Confidence::Medium)
    );

    let docs = classify("docs: explain").unwrap();
    assert_eq!(
        (docs.kind, docs.prefix, docs.confidence),
        (EventKind::Note, "Documentation", Confidence::Low)
    );

    let tests = classify("test: cover it").unwrap();
    assert_eq!(
        (tests.kind, tests.prefix, tests.confidence),
        (EventKind::Note, "Tests", Confidence::Low)
    );
}

#[test]
fn classify_matches_prefixes_and_words_as_python_does() {
    for (message, prefix) in [
        ("revert: x", "Reverted"),
        ("Revert-ing x", "Reverted"),
        ("REVERT x", "Reverted"),
        ("hotfix(core): x", "Fix"),
        ("BugFix x", "Fix"),
        ("patch: x", "Fix"),
        ("fix\u{a0}x", "Fix"),
        ("break: x", "Breaking change"),
        ("note about BREAKING_CHANGE here", "Breaking change"),
        ("note about breaking-change here", "Breaking change"),
        ("note about breaking change here", "Breaking change"),
        ("note about breakingchange here", "Breaking change"),
        ("feature: x", "New feature"),
        ("Add: x", "New feature"),
        ("cleanup x", "Refactor"),
        ("reorganize modules", "Refactor"),
        ("restructure", "Refactor"),
        ("doc: x", "Documentation"),
        ("README tweaks", "Documentation"),
        ("changelog entry", "Documentation"),
        ("tests: x", "Tests"),
        ("spec(api): x", "Tests"),
    ] {
        assert_eq!(classify(message).map(|found| found.prefix), Some(prefix));
    }
}

#[test]
fn classify_anchors_at_the_start_of_the_whole_message() {
    assert!(classify("tidy\nfix: later line").is_none());
    assert!(classify("fixture: x").is_none());
    assert!(classify("fixed: x").is_none());
    assert!(classify("address: x").is_none());
    assert!(classify("update dependencies").is_none());
    assert!(classify("").is_none());
    assert!(classify("tidy\nBREAKING CHANGE: x").is_some());
}

#[test]
fn classify_takes_the_first_matching_pattern() {
    assert_eq!(
        classify("revert: fix: BREAKING CHANGE").map(|found| found.prefix),
        Some("Reverted")
    );
    assert_eq!(
        classify("fix: BREAKING CHANGE").map(|found| found.prefix),
        Some("Fix")
    );
    assert_eq!(
        classify("docs: BREAKING CHANGE").map(|found| found.prefix),
        Some("Breaking change")
    );
}

#[test]
fn only_medium_and_high_confidence_is_logged() {
    assert!(meets_threshold(Confidence::High));
    assert!(meets_threshold(Confidence::Medium));
    assert!(!meets_threshold(Confidence::Low));
}

#[test]
fn first_line_strips_splits_and_clips_by_characters() {
    assert_eq!(first_line("  fix: x \n\nbody"), "fix: x ");
    assert_eq!(first_line("\u{a0}\n fix: x\ny"), "fix: x");
    assert_eq!(first_line(""), "");
    assert_eq!(first_line("a\r\nb"), "a\r");
    let long = "\u{e9}".repeat(130);
    assert_eq!(first_line(&long).chars().count(), 120);
}

#[test]
fn location_skips_memory_files_but_files_keep_them() {
    let list = changes(&[('A', ".dejafix/summary.md"), ('A', "src/login.py")]);
    let location = pick_location(&list, is_memory);
    assert_eq!(location, Some("src/login.py"));
    assert_eq!(
        capture_files(&list, location, is_memory),
        ["src/login.py", ".dejafix/summary.md"]
    );
}

#[test]
fn a_memory_only_commit_has_no_location() {
    let list = added(&[".dejafix/summary.md"]);
    assert_eq!(pick_location(&list, is_memory), None);
    assert_eq!(
        capture_files(&list, None, is_memory),
        [".dejafix/summary.md"]
    );
    assert_eq!(pick_location(&[], is_memory), None);
}

#[test]
fn a_deleted_file_is_never_the_location() {
    let list = changes(&[('D', "src/aaa_old.py"), ('A', "src/login.py")]);
    let location = pick_location(&list, is_memory);
    assert_eq!(location, Some("src/login.py"));
    assert_eq!(
        capture_files(&list, location, is_memory),
        ["src/login.py", "src/aaa_old.py"]
    );
    assert_eq!(located(&[('D', "src/a.py")]), None);
}

#[test]
fn a_rename_is_located_at_the_new_path() {
    let parsed = parse_name_status(b"R100\0src/old_name.py\0src/new_name.py\0");
    let location = pick_location(&parsed, is_memory);
    assert_eq!(location, Some("src/new_name.py"));
    assert_eq!(
        capture_files(&parsed, location, is_memory),
        ["src/new_name.py"]
    );
}

#[test]
fn the_first_commit_lists_its_files() {
    let parsed = parse_name_status(b"A\0src/app.py\0");
    assert_eq!(pick_location(&parsed, is_memory), Some("src/app.py"));
}

#[test]
fn a_non_ascii_path_is_kept_as_written() {
    let parsed = parse_name_status("A\0src/caf\u{e9}.py\0".as_bytes());
    let location = pick_location(&parsed, is_memory);
    assert_eq!(location, Some("src/caf\u{e9}.py"));
    assert_eq!(
        capture_files(&parsed, location, is_memory),
        ["src/caf\u{e9}.py"]
    );
}

#[test]
fn code_outranks_docs_and_ci_config() {
    let list = added(&[".github/workflows/ci.yml", "README.md", "src/app.py"]);
    let location = pick_location(&list, is_memory);
    assert_eq!(location, Some("src/app.py"));
    let mut files = capture_files(&list, location, is_memory);
    files.sort();
    assert_eq!(
        files,
        [".github/workflows/ci.yml", "README.md", "src/app.py"]
    );
}

#[test]
fn docs_outrank_lockfiles_and_a_tie_keeps_git_order() {
    assert_eq!(
        location_of(&["Cargo.lock", "docs/upgrade.md"]).as_deref(),
        Some("docs/upgrade.md")
    );
    assert_eq!(
        location_of(&["src/a.py", "src/b.py"]).as_deref(),
        Some("src/a.py")
    );
}

#[test]
fn manifests_and_build_files_never_beat_source() {
    for (manifest, code) in [
        ("package.json", "src/app.py"),
        ("Dockerfile", "src/app.py"),
        ("pyproject.toml", "src/app.py"),
        ("Makefile", "src/main.c"),
        ("requirements.txt", "src/app.py"),
        ("CMakeLists.txt", "src/main.c"),
    ] {
        assert_eq!(
            location_of(&[manifest, code]).as_deref(),
            Some(code),
            "{manifest}"
        );
    }
}

#[test]
fn the_first_commit_with_manifests_is_located_at_the_code() {
    assert_eq!(
        location_of(&["Dockerfile", "package.json", "pyproject.toml", "src/app.py"]).as_deref(),
        Some("src/app.py")
    );
}

#[test]
fn source_outranks_tests_which_outrank_other_files() {
    assert_eq!(
        location_of(&["tests/test_app.py", "web/ui.js"]).as_deref(),
        Some("web/ui.js")
    );
    assert_eq!(
        location_of(&["src/data/words.txt", "tests/test_words.py"]).as_deref(),
        Some("tests/test_words.py")
    );
    assert_eq!(
        location_of(&["docs/conf.py", "docs/guide.md"]).as_deref(),
        Some("docs/conf.py")
    );
}

#[test]
fn only_a_top_level_dot_dir_or_dot_file_counts_as_config() {
    assert_eq!(
        location_of(&["README.md", "src/.hidden/x.py"]).as_deref(),
        Some("src/.hidden/x.py")
    );
    assert_eq!(
        location_of(&["src/.well-known/foo.ts", "src/zzz.ts"]).as_deref(),
        Some("src/.well-known/foo.ts")
    );
    assert_eq!(
        location_of(&[".env.example", ".github/workflows/ci.yml", "src/app.py"]).as_deref(),
        Some("src/app.py")
    );
}

#[test]
fn a_merge_commit_lists_only_what_git_reports() {
    let parsed = parse_name_status(b"A\0src/feature.py\0");
    let location = pick_location(&parsed, is_memory);
    assert_eq!(location, Some("src/feature.py"));
    assert_eq!(
        capture_files(&parsed, location, is_memory),
        ["src/feature.py"]
    );
}

#[test]
fn memory_files_come_last_so_the_cap_keeps_the_code() {
    let mut paths: Vec<String> = (0..12)
        .map(|index| format!(".dejafix/issues/{index:04}-x.md"))
        .collect();
    paths.push("src/app.py".to_owned());
    let list: Vec<Change> = paths
        .into_iter()
        .map(|path| Change { status: 'A', path })
        .collect();
    let location = pick_location(&list, is_memory);
    let files = capture_files(&list, location, is_memory);
    assert_eq!(location, Some("src/app.py"));
    assert_eq!(files.first().map(String::as_str), Some("src/app.py"));
    assert_eq!(files.len(), 10);
}

#[test]
fn the_location_is_always_first_in_files() {
    let mut paths: Vec<String> = (0..14).map(|index| format!("a{index:02}.md")).collect();
    paths.push("z/code.py".to_owned());
    let list: Vec<Change> = paths
        .into_iter()
        .map(|path| Change { status: 'A', path })
        .collect();
    let location = pick_location(&list, is_memory);
    let files = capture_files(&list, location, is_memory);
    assert_eq!(location, Some("z/code.py"));
    assert_eq!(files.first().map(String::as_str), Some("z/code.py"));
    assert_eq!(files.len(), 10);
    assert_eq!(files.get(1).map(String::as_str), Some("a00.md"));
}

#[test]
fn memory_files_keep_git_order_among_themselves_after_the_code() {
    let list = added(&[".dejafix/b.md", "a.md", ".dejafix/a.md", "c.md"]);
    assert_eq!(
        capture_files(&list, None, is_memory),
        ["a.md", "c.md", ".dejafix/b.md", ".dejafix/a.md"]
    );
}

#[test]
fn a_code_file_whose_name_starts_like_a_doc_or_manifest_is_code() {
    for (code, other) in [
        ("src/requirements_checker.py", "README.md"),
        ("src/tsconfig_utils.ts", "package.json"),
        ("src/NoticeService.java", "CHANGELOG.md"),
        ("src/license_check.py", "docs/guide.md"),
        ("src/readme_gen.py", "LICENSE"),
        ("src/dockerfile_parser.py", "Dockerfile"),
    ] {
        let mut paths = [other, code];
        paths.sort_unstable();
        assert_eq!(location_of(&paths).as_deref(), Some(code), "{code}");
    }
}

#[test]
fn explicit_build_code_files_are_still_demoted() {
    assert_eq!(
        location_of(&["conftest.py", "setup.py", "src/app.py", "vite.config.ts"]).as_deref(),
        Some("src/app.py")
    );
    assert_eq!(
        location_of(&["README.md", "webpack.config.js"]).as_deref(),
        Some("README.md")
    );
}

#[test]
fn binary_assets_do_not_beat_docs() {
    assert_eq!(
        location_of(&["README.md", "assets/logo.png"]).as_deref(),
        Some("README.md")
    );
}

#[test]
fn location_rank_covers_every_table() {
    for (path, rank) in [
        ("src/app.py", 0),
        ("Src/App.PY", 0),
        ("web/ui.js", 0),
        ("tests/test_app.py", 1),
        ("src/Tests/app.py", 1),
        ("src/test_app.py", 1),
        ("src/app_test.go", 1),
        ("src/app.spec.ts", 1),
        ("src/app.test.ts", 1),
        ("src/data/words.txt", 2),
        ("data.json", 2),
        ("README.md", 3),
        ("docs/guide.txt", 3),
        ("LICENSE", 3),
        ("LICENSE.txt", 3),
        ("notes.RST", 3),
        ("guide.adoc", 3),
        ("docs", 3),
        ("Dockerfile", 4),
        ("Dockerfile.dev", 4),
        ("app.dockerfile", 4),
        ("docker-compose.override.yml", 4),
        ("requirements-dev.txt", 4),
        ("tsconfig.build.json", 4),
        ("vite.config.ts", 4),
        (".gitignore", 4),
        (".github/workflows/ci.yml", 4),
        ("assets/logo.PNG", 4),
        ("Cargo.toml", 4),
        ("Cargo.lock", 5),
        ("package-lock.json", 5),
        ("yarn.lock", 5),
        ("go.sum", 5),
        ("src/thing.lock", 5),
        ("src/.well-known/foo.ts", 0),
        ("a.", 2),
        ("", 2),
    ] {
        assert_eq!(location_rank(path), rank, "{path}");
    }
}

#[test]
fn capture_event_builds_what_python_builds() {
    let clock = fixed_clock();
    let mut ids = SeqIds::default();
    let list = changes(&[('A', ".dejafix/summary.md"), ('M', "src/login.py")]);
    let event = capture_event(
        "fix: guard the login submit\n\nlonger body",
        &list,
        Some("abc1234".to_owned()),
        is_memory,
        &clock,
        &mut ids,
    )
    .unwrap();
    assert_eq!(event.id.as_str(), "evt_00000000000000000001");
    assert_eq!(event.at.as_str(), Timestamp::now(&clock).as_str());
    assert_eq!(event.kind, EventKind::Fix);
    assert_eq!(event.summary.as_str(), "Fix: fix: guard the login submit");
    assert_eq!(event.outcome, None);
    assert_eq!(event.files, ["src/login.py", ".dejafix/summary.md"]);
    assert_eq!(event.location.as_deref(), Some("src/login.py"));
    assert_eq!(event.command.as_deref(), Some("auto-capture"));
    assert_eq!(event.issue, None);
    assert_eq!(event.notes, None);
    assert!(event.origin.auto_captured);
    assert_eq!(event.origin.source, Some(CaptureSource::GitPostCommit));
    assert_eq!(event.origin.confidence, Some(Confidence::High));
    assert_eq!(event.origin.git_commit.as_deref(), Some("abc1234"));
    assert_eq!(
        event.origin.git_message.as_deref(),
        Some("fix: guard the login submit")
    );
}

#[test]
fn capture_event_records_a_revert_as_a_failed_attempt() {
    let clock = fixed_clock();
    let mut ids = SeqIds::default();
    let event = capture_event("Revert \"x\"", &[], None, is_memory, &clock, &mut ids).unwrap();
    assert_eq!(event.kind, EventKind::Attempt);
    assert_eq!(event.outcome, Some(Outcome::Failed));
    assert_eq!(event.summary.as_str(), "Reverted: Revert \"x\"");
    assert_eq!(event.files.len(), 0);
    assert_eq!(event.location, None);
    assert_eq!(event.origin.git_commit, None);
}

#[test]
fn capture_event_skips_low_confidence_and_unclassified_messages() {
    let clock = fixed_clock();
    let mut ids = SeqIds::default();
    let list = added(&["src/a.py"]);
    for message in ["docs: explain", "test: cover", "update stuff", ""] {
        assert!(capture_event(message, &list, None, is_memory, &clock, &mut ids).is_none());
    }
    let event = capture_event("feat: x", &list, None, is_memory, &clock, &mut ids).unwrap();
    assert_eq!(event.id.as_str(), "evt_00000000000000000001");
    assert_eq!(event.origin.confidence, Some(Confidence::Medium));
}

#[test]
fn capture_event_clips_the_first_line_to_120_characters() {
    let clock = fixed_clock();
    let mut ids = SeqIds::default();
    let message = format!("fix: {}", "x".repeat(200));
    let event = capture_event(&message, &[], None, is_memory, &clock, &mut ids).unwrap();
    let line = event.origin.git_message.unwrap();
    assert_eq!(line.chars().count(), 120);
    assert_eq!(event.summary.as_str(), format!("Fix: {line}"));
}

#[test]
fn merge_event_is_a_high_confidence_note() {
    let clock = fixed_clock();
    let mut ids = SeqIds::default();
    let event = merge_event(
        "Merge branch 'x' into main\n\nbody",
        Some("def5678".to_owned()),
        &clock,
        &mut ids,
    )
    .unwrap();
    assert_eq!(event.kind, EventKind::Note);
    assert_eq!(event.summary.as_str(), "Merge: Merge branch 'x' into main");
    assert_eq!(event.files.len(), 0);
    assert_eq!(event.location, None);
    assert_eq!(event.outcome, None);
    assert_eq!(event.command.as_deref(), Some("auto-capture"));
    assert!(event.origin.auto_captured);
    assert_eq!(event.origin.source, Some(CaptureSource::GitPostMerge));
    assert_eq!(event.origin.confidence, Some(Confidence::High));
    assert_eq!(event.origin.git_commit.as_deref(), Some("def5678"));
    assert_eq!(
        event.origin.git_message.as_deref(),
        Some("Merge branch 'x' into main")
    );
}
