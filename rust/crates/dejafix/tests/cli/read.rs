use std::fs;

use crate::support::{Repo, TestResult};

#[test]
fn show_prints_the_summary_with_a_trailing_newline() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["note", "startup is slow"]).assert().success();
    let summary = fs::read_to_string(repo.mem().join("summary.md"))?;
    repo.djf(&["show"])
        .assert()
        .success()
        .stdout(format!("{summary}\n"));
    Ok(())
}

#[test]
fn show_without_a_summary_is_an_io_error() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["show"]).assert().code(3);
    Ok(())
}

#[test]
fn search_finds_matching_events() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "token expiry bug"]).assert().success();
    repo.djf(&["note", "startup is slow"]).assert().success();
    let out = repo.stdout(&["search", "token"])?;
    assert!(out.contains("token expiry bug") && !out.contains("startup is slow"));
    assert!(out.contains("] issue #0001: token expiry bug\n"));
    Ok(())
}

#[test]
fn search_tags_superseded_and_shows_ids() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["decision", "use bcrypt everywhere"])
        .assert()
        .success();
    let old = repo.events()?[0]["id"].as_str().ok_or("id")?.to_owned();
    repo.djf(&["decision", "use argon2", "--supersedes", &old])
        .assert()
        .success();
    let out = repo.stdout(&["search", "bcrypt"])?;
    assert!(out.contains("(superseded)") && out.contains(&old));
    Ok(())
}

#[test]
fn search_matches_location_and_filters_failed_attempts() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log", "form jumps"]).assert().success();
    repo.djf(&[
        "attempt",
        "tried contain layout",
        "--failed",
        "--at",
        "payment.py",
    ])
    .assert()
    .success();
    repo.djf(&["attempt", "rewrote the carousel", "--worked"])
        .assert()
        .success();
    assert!(
        repo.stdout(&["search", "payment.py", "--failed-only"])?
            .contains("tried contain layout")
    );
    assert_eq!(
        repo.stdout(&["search", "carousel", "--failed-only"])?,
        "No matches.\n"
    );
    assert!(
        repo.stdout(&["search", "contain", "--failed-only"])?
            .contains("tried contain layout")
    );
    assert!(
        repo.stdout(&["search", "carousel|form"])?
            .starts_with("No matches. (Tip:")
    );
    assert_eq!(
        repo.stdout(&["search", "-r", "carousel|form"])?
            .lines()
            .count(),
        2
    );
    Ok(())
}

#[test]
fn regenerate_reports_the_summary_path() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["regenerate"]).assert().success().stdout(format!(
        "Regenerated {}\n",
        repo.mem().join("summary.md").display()
    ));
    Ok(())
}

#[test]
fn a_subdirectory_finds_the_project_but_the_global_store_is_never_a_project() -> TestResult {
    let repo = Repo::new()?;
    let deep = repo.root.join("src/deep");
    fs::create_dir_all(&deep)?;
    repo.djf_in(&deep, &["note", "from below"])
        .assert()
        .success();
    assert_eq!(repo.events()?.len(), 1);
    let scratch = repo.home.join("Documents/scratch");
    fs::create_dir_all(&scratch)?;
    fs::create_dir_all(repo.home.join(".dejafix/global"))?;
    let output = repo.djf_in(&scratch, &["note", "x"]).assert().code(1);
    let stderr = String::from_utf8(output.get_output().stderr.clone())?;
    assert!(stderr.starts_with("Error: No .dejafix directory found in "));
    assert!(!repo.home.join(".dejafix/events.jsonl").exists());
    Ok(())
}

#[test]
fn output_is_utf8_without_replacement_characters() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["note", "žluťoučký kůň — gotcha"])
        .assert()
        .success();
    let out = repo.stdout(&["show"])?;
    assert!(out.contains("žluťoučký kůň — gotcha") && !out.contains('\u{fffd}'));
    Ok(())
}

#[test]
fn bad_arguments_exit_two_and_help_exits_zero() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["log"]).assert().code(2);
    repo.djf(&["--bogus"]).assert().code(2);
    repo.djf(&["--help"]).assert().success();
    Ok(())
}
