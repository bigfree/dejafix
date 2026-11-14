use std::fs;

use crate::support::{Repo, TestResult};

#[test]
fn log_attempt_and_fix_write_events() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "Login fails", "--at", "auth.py:42"])
        .assert()
        .success()
        .stdout("Logged issue #0001\n");
    repo.djf(&["attempt", "Clear cookies", "--failed"])
        .assert()
        .success()
        .stdout("Recorded failed attempt on #0001\n");
    repo.djf(&["fix", "Reset session", "--at", "auth.py:50"])
        .assert()
        .success()
        .stdout("Fixed issue #0001\n");
    let events = repo.events()?;
    let kinds: Vec<&str> = events
        .iter()
        .filter_map(|event| event["type"].as_str())
        .collect();
    assert_eq!(kinds, ["issue", "attempt", "fix"]);
    assert!(events.iter().all(|event| {
        event["git_commit"]
            .as_str()
            .is_some_and(|commit| commit.len() >= 7)
    }));
    assert_eq!(events[0]["location"], "auth.py:42");
    assert!(!repo.mem().join(".current_issue").exists());
    assert!(
        fs::read_to_string(repo.mem().join("summary.md"))?
            .contains("[DONE] #0001 Login fails [auth.py:42] (fixed)")
    );
    Ok(())
}

#[test]
fn fix_issue_closes_target_without_clearing_newer_active_issue() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "first"]).assert().success();
    repo.djf(&["log", "second"]).assert().success();
    repo.djf(&["fix", "closed first", "--issue", "1"])
        .assert()
        .success()
        .stdout("Fixed issue #0001\n");
    assert_eq!(
        fs::read_to_string(repo.mem().join(".current_issue"))?,
        "0002"
    );
    repo.djf(&["attempt", "for second", "--worked"])
        .assert()
        .success()
        .stdout("Recorded worked attempt on #0002\n");
    Ok(())
}

#[test]
fn plain_fix_still_closes_active_issue() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "only"]).assert().success();
    repo.djf(&["fix", "done"])
        .assert()
        .success()
        .stdout("Fixed issue #0001\n");
    assert!(!repo.mem().join(".current_issue").exists());
    Ok(())
}

#[test]
fn fix_issue_rejects_unknown_issue() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "only"]).assert().success();
    repo.djf(&["fix", "x", "--issue", "9"]).assert().code(1).stderr(
        "Error: Issue #0009 was not found. Run `djf search <query>` or `djf brief` to find the issue ID.\n",
    );
    assert_eq!(repo.events()?.len(), 1);
    Ok(())
}

#[test]
fn attempt_without_open_issue_fails() -> TestResult {
    let repo = Repo::new()?;
    let output = repo.djf(&["attempt", "x", "--failed"]).assert().code(1);
    let stderr = String::from_utf8(output.get_output().stderr.clone())?;
    assert!(stderr.starts_with("Error: No active issue."));
    assert_eq!(repo.events()?.len(), 0);
    Ok(())
}

#[test]
fn attempt_auto_issue_creates_parent() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["attempt", "flaky test", "--worked", "--auto-issue"])
        .assert()
        .success()
        .stdout("Recorded worked attempt on #0001\n");
    let events = repo.events()?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "issue");
    assert_eq!(events[0]["summary"], "flaky test");
    assert_eq!(
        fs::read_to_string(repo.mem().join(".current_issue"))?,
        "0001"
    );
    Ok(())
}

#[test]
fn attempt_after_fix_does_not_misattribute() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "first"]).assert().success();
    repo.djf(&["fix", "done"]).assert().success();
    repo.djf(&["attempt", "later", "--failed"]).assert().code(1);
    assert_eq!(repo.events()?.len(), 2);
    Ok(())
}

#[test]
fn attempt_on_an_unknown_issue_is_refused_and_the_project_still_works() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "real issue"]).assert().success();
    let output = repo
        .djf(&["attempt", "--issue", "0099", "--failed", "typo id"])
        .assert()
        .code(1);
    assert!(String::from_utf8(output.get_output().stderr.clone())?.contains("#0099 was not found"));
    assert!(
        repo.events()?
            .iter()
            .all(|event| event["issue_id"] != "0099")
    );
    repo.djf(&["note", "still works"])
        .assert()
        .success()
        .stdout("Recorded note\n");
    Ok(())
}

#[test]
fn attempt_issue_id_is_normalised_like_fix() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "real issue"]).assert().success();
    repo.djf(&["attempt", "--issue", "1", "--failed", "short id"])
        .assert()
        .success()
        .stdout("Recorded failed attempt on #0001\n");
    Ok(())
}

#[test]
fn a_stale_current_issue_marker_is_ignored() -> TestResult {
    let repo = Repo::new()?;
    fs::write(repo.mem().join(".current_issue"), "0042")?;
    let output = repo
        .djf(&["attempt", "--failed", "no real issue yet"])
        .assert()
        .code(1);
    assert!(String::from_utf8(output.get_output().stderr.clone())?.contains("No active issue"));
    assert_eq!(repo.events()?.len(), 0);
    assert!(!repo.mem().join(".current_issue").exists());
    Ok(())
}

#[test]
fn fix_ignores_and_clears_a_stale_marker() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "real issue"]).assert().success();
    fs::write(repo.mem().join(".current_issue"), "0777")?;
    repo.djf(&["fix", "fixed the real one"])
        .assert()
        .success()
        .stdout("Fixed issue #0001\n");
    assert!(
        repo.events()?
            .iter()
            .all(|event| event["issue_id"] != "0777")
    );
    assert!(!repo.mem().join(".current_issue").exists());
    Ok(())
}

#[test]
fn a_bare_hash_is_reported_readably() -> TestResult {
    let repo = Repo::new()?;
    let output = repo
        .djf(&["attempt", "--issue", "#", "--failed", "x"])
        .assert()
        .code(1);
    let stderr = String::from_utf8(output.get_output().stderr.clone())?;
    assert!(stderr.contains("Issue #? was not found") && !stderr.contains("##"));
    Ok(())
}

#[test]
fn two_outcome_flags_are_refused() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "x"]).assert().success();
    repo.djf(&["attempt", "y", "--failed", "--worked"])
        .assert()
        .code(1)
        .stderr("Error: Use only one of --worked, --failed, or --partial.\n");
    Ok(())
}

#[test]
fn an_empty_summary_is_a_user_error_not_a_crash() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["note", "   "])
        .assert()
        .code(1)
        .stderr("Error: Event summary cannot be empty\n");
    Ok(())
}

#[test]
fn an_expected_error_prints_no_traceback() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["fix", "nothing open"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("Error: No open issue found. Run `djf log <text>` first.\n");
    Ok(())
}

#[test]
fn supersede_hides_old_decision_from_summary_and_accepts_a_prefix() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["decision", "use bcrypt rounds=12"])
        .assert()
        .success()
        .stdout("Recorded decision\n");
    let old = repo.events()?[0]["id"].as_str().ok_or("id")?.to_owned();
    let prefix = &old["evt_".len().."evt_".len() + 8];
    repo.djf(&["decision", "switch to argon2", "--supersedes", prefix])
        .assert()
        .success()
        .stdout(format!(
            "Recorded decision (supersedes {old}: \"use bcrypt rounds=12\")\n"
        ));
    let summary = fs::read_to_string(repo.mem().join("summary.md"))?;
    assert!(summary.contains("switch to argon2") && !summary.contains("use bcrypt rounds=12"));
    assert_eq!(repo.events()?.len(), 2);
    Ok(())
}

#[test]
fn supersede_bad_ref_fails_without_writing() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["decision", "x", "--supersedes", "evt_nope"]).assert().code(1).stderr(
        "Error: No event matches 'evt_nope'. Use `djf search <text>` to find the event id (shown as evt_...).\n",
    );
    assert_eq!(repo.events()?.len(), 0);
    Ok(())
}

#[test]
fn secrets_are_redacted_with_a_notice_unless_disabled() -> TestResult {
    let repo = Repo::new()?;
    let secret = format!("sk-{}", "a".repeat(44));
    repo.djf(&["note", &format!("key {secret}")])
        .assert()
        .success()
        .stdout("Recorded note\n")
        .stderr("dejafix: redacted 1 secret(s) before write (openai_key)\n");
    repo.djf(&["note", &format!("raw {secret}")])
        .env("DEJAFIX_NO_REDACT", " yes ")
        .assert()
        .success()
        .stderr("");
    let events = repo.events()?;
    assert_eq!(events[0]["summary"], "key [REDACTED:openai_key]");
    assert_eq!(events[1]["summary"], format!("raw {secret}"));
    Ok(())
}

#[test]
fn appending_an_event_stays_inside_an_isolated_home() -> TestResult {
    let repo = Repo::new()?;
    fs::write(
        repo.root.join("package.json"),
        r#"{"dependencies": {"react": "18"}}"#,
    )?;
    repo.djf(&["note", "gotcha: react strict mode mounts twice"])
        .assert()
        .success();
    assert!(
        repo.home
            .join(".dejafix/global/library_gotchas.jsonl")
            .exists()
    );
    let elsewhere = repo.home.join("isolated");
    repo.djf(&["note", "gotcha: react portals lose context"])
        .env("DEJAFIX_HOME", &elsewhere)
        .assert()
        .success();
    assert!(
        fs::read_to_string(elsewhere.join("global/library_gotchas.jsonl"))?.contains("portals")
    );
    Ok(())
}

#[test]
fn global_memory_falls_back_to_home_without_the_env_var() -> TestResult {
    let repo = Repo::new()?;
    fs::write(
        repo.root.join("package.json"),
        r#"{"dependencies": {"vite": "5"}}"#,
    )?;
    repo.djf(&["note", "gotcha: vite caches env vars"])
        .env_remove("DEJAFIX_HOME")
        .assert()
        .success();
    assert!(
        repo.home
            .join(".dejafix/global/library_gotchas.jsonl")
            .exists()
    );
    Ok(())
}

#[test]
fn a_failed_regeneration_prints_the_outcome_then_the_error() -> TestResult {
    let repo = Repo::new()?;
    fs::create_dir_all(repo.mem().join("summary.md/blocked"))?;
    let output = repo
        .djf(&["log", "still logged"])
        .assert()
        .code(3)
        .stdout("Logged issue #0001\n");
    assert!(
        String::from_utf8(output.get_output().stderr.clone())?.starts_with("Error: cannot write")
    );
    assert_eq!(repo.events()?.len(), 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_memory_folder_linked_out_of_the_project_is_refused() -> TestResult {
    let repo = Repo::bare()?;
    let outside = repo.home.with_file_name("outside");
    fs::create_dir_all(&outside)?;
    fs::write(outside.join("config.toml"), "")?;
    fs::write(outside.join("events.jsonl"), "")?;
    std::os::unix::fs::symlink(&outside, repo.mem())?;
    let output = repo
        .djf(&["note", "x"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(output.stderr)?,
        format!(
            "Error: {} points outside the project; refusing to use it\n",
            repo.mem().display()
        )
    );
    assert_eq!(fs::read_to_string(outside.join("events.jsonl"))?, "");
    Ok(())
}

#[cfg(unix)]
#[test]
fn an_event_log_linked_out_of_the_project_is_never_appended_to() -> TestResult {
    let repo = Repo::new()?;
    let secret = repo.home.join("zshrc");
    fs::write(&secret, "export TOKEN=1\n")?;
    fs::remove_file(repo.mem().join("events.jsonl"))?;
    std::os::unix::fs::symlink(&secret, repo.mem().join("events.jsonl"))?;
    repo.djf(&["note", "x"]).assert().code(1);
    assert_eq!(fs::read_to_string(&secret)?, "export TOKEN=1\n");
    Ok(())
}
