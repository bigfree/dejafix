use std::fs;
use std::process::Command;

use crate::support::{Repo, TestResult};

fn git(repo: &Repo, args: &[&str]) -> TestResult {
    let status = Command::new("git")
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(args)
        .current_dir(&repo.root)
        .status()?;
    assert!(status.success());
    Ok(())
}

fn with_lessons() -> Result<Repo, Box<dyn std::error::Error>> {
    let repo = Repo::new()?;
    repo.djf(&["log", "form jumps"]).assert().success();
    for text in [
        "tried contain layout",
        "debounced the handler",
        "pinned the height",
    ] {
        repo.djf(&["attempt", text, "--failed", "--at", "payment.py"])
            .assert()
            .success();
    }
    Ok(repo)
}

#[test]
fn named_files_list_what_already_failed() -> TestResult {
    let repo = with_lessons()?;
    let out = repo.stdout(&["precheck", "payment.py"])?;
    assert!(out.starts_with("\ndejafix: Pre-Commit Check\n"));
    assert!(out.contains("    BLOCK  What already failed here (3 attempts):\n"));
    assert!(out.contains("           x tried contain layout (just now)\n"));
    assert!(out.contains("─".repeat(60).as_str()));
    assert!(!out.contains('\u{1b}'));
    assert!(
        out.ends_with("1 warning(s). Review before committing.\n\n"),
        "{out}"
    );
    let blocked = repo
        .djf(&["precheck", "payment.py", "--level", "block"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8(blocked)?
            .contains("Blocked: 1 critical warning(s).\n  Bypass once: git commit --no-verify\n")
    );
    Ok(())
}

#[test]
fn staged_and_working_files_and_quiet_runs() -> TestResult {
    let repo = with_lessons()?;
    repo.djf(&["precheck"])
        .assert()
        .success()
        .stdout("dejafix: No files to check.\n");
    repo.djf(&["precheck", "-q"]).assert().success().stdout("");
    fs::write(repo.root.join("payment.py"), "x\n")?;
    git(&repo, &["add", "payment.py"])?;
    let staged = repo.stdout(&["precheck"])?;
    assert!(staged.contains("  payment.py\n"));
    git(&repo, &["commit", "-qm", "add payment", "--no-verify"])?;
    fs::write(repo.root.join("payment.py"), "y\n")?;
    assert!(
        repo.stdout(&["precheck", "--working"])?
            .contains("  payment.py\n")
    );
    repo.djf(&["precheck", "README.md"])
        .assert()
        .success()
        .stdout("dejafix: no warnings — looking good!\n");
    repo.djf(&["precheck", "README.md", "--quiet"])
        .assert()
        .success()
        .stdout("");
    repo.djf(&["precheck", ".dejafix/summary.md", "."])
        .assert()
        .success()
        .stdout("dejafix: no warnings — looking good!\n");
    Ok(())
}

#[test]
fn snooze_silences_logs_and_unsnooze_restores() -> TestResult {
    let repo = with_lessons()?;
    let summary_before = repo.read(".dejafix/summary.md")?;
    let out = repo.stdout(&["precheck", "--snooze", "2h"])?;
    assert!(out.starts_with("dejafix: precheck warnings snoozed until "));
    assert!(
        out.ends_with(" UTC (logged to memory). Re-enable early with `djf precheck --unsnooze`.\n")
    );
    let marker = repo.mem().join("precheck.snooze");
    assert!(fs::read_to_string(&marker)?.ends_with("+00:00"));
    let events = repo.events()?;
    let note = events.last().ok_or("no note")?;
    assert_eq!(note["type"], "note");
    assert!(
        note["summary"]
            .as_str()
            .unwrap_or_default()
            .starts_with("precheck warnings snoozed for 2h (until ")
    );
    assert_eq!(repo.read(".dejafix/summary.md")?, summary_before);
    let snoozed = repo.stdout(&["precheck", "payment.py"])?;
    assert_eq!(
        snoozed,
        "dejafix: warnings snoozed (1h59m left) — `djf precheck --unsnooze` to re-enable.\n"
    );
    repo.djf(&["precheck", "--unsnooze"])
        .assert()
        .success()
        .stdout("dejafix: precheck warnings re-enabled.\n");
    assert!(!marker.exists());
    repo.djf(&["precheck", "--unsnooze"])
        .assert()
        .success()
        .stdout("dejafix: no active snooze.\n");
    repo.djf(&["precheck", "--snooze", "soon"])
        .assert()
        .code(1)
        .stderr("Error: Invalid duration 'soon' — use forms like 30m, 2h, or 1d.\n");
    repo.djf(&["precheck", "--snooze", "0h"])
        .assert()
        .code(1)
        .stderr("Error: Snooze duration must be positive.\n");
    Ok(())
}

#[test]
fn an_expired_or_broken_snooze_cleans_itself_up() -> TestResult {
    let repo = with_lessons()?;
    let marker = repo.mem().join("precheck.snooze");
    for text in ["2020-01-01T00:00:00+00:00", "garbage"] {
        fs::write(&marker, text)?;
        assert!(
            repo.stdout(&["precheck", "payment.py"])?
                .contains("What already failed here")
        );
        assert!(!marker.exists());
    }
    Ok(())
}

#[test]
fn outside_a_project_precheck_is_silent() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["precheck", "--snooze", "2h"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
    repo.djf(&["precheck", "a.py"])
        .assert()
        .success()
        .stdout("");
    Ok(())
}
