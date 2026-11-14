use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::support::{Repo, TestResult};

fn git(repo: &Repo, args: &[&str]) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    Ok(Command::new("git")
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(args)
        .current_dir(&repo.root)
        .env("HOME", &repo.home)
        .env("DEJAFIX_HOME", repo.home.join(".dejafix"))
        .output()?)
}

fn commit(repo: &Repo, message: &str, files: &[(&str, &str)]) -> TestResult {
    for (path, text) in files {
        let target = repo.root.join(path);
        fs::create_dir_all(target.parent().ok_or("no parent")?)?;
        fs::write(target, text)?;
        git(repo, &["add", path])?;
    }
    let output = git(repo, &["commit", "-q", "--no-verify", "-m", message])?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

fn hook(repo: &Repo, name: &str) -> String {
    fs::read_to_string(repo.root.join(".git/hooks").join(name)).unwrap_or_default()
}

#[test]
fn init_installs_three_executable_hooks_baked_with_this_binary() -> TestResult {
    let repo = Repo::bare()?;
    let out = repo.stdout(&["init", "--no-mcp-config"])?;
    assert!(out.contains(
        "dejafix git hooks installed: post-commit, post-merge, pre-commit\n  Auto-captures: commits, reverts, merges, fixes, features, breaking changes.\n  Pre-commit: warns about repeating failed approaches and high-churn files.\n  Auto-capture active — events will be logged automatically.\n"
    ));
    let binary = Path::new(env!("CARGO_BIN_EXE_djf"));
    let baked = binary
        .parent()
        .ok_or("no parent")?
        .join(format!("djf{}", std::env::consts::EXE_SUFFIX));
    let baked = dejafix_core::shell_path(&baked.to_string_lossy());
    for name in ["post-commit", "post-merge", "pre-commit"] {
        let text = hook(&repo, name);
        assert!(
            text.starts_with("#!/bin/sh\n# >>> dejafix auto-capture >>>\n"),
            "{name}"
        );
        assert!(text.contains(&format!("DJF_BIN=\"{baked}\"")), "{name}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(repo.root.join(".git/hooks").join(name))?
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "{name}");
        }
    }
    assert!(hook(&repo, "post-merge").contains("_auto-capture \"merge\" >/dev/null 2>&1 &"));
    Ok(())
}

#[test]
fn a_hook_written_with_crlf_is_rewritten_with_lf() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["hooks", "install"]).assert().success();
    let path = repo.root.join(".git/hooks/post-commit");
    let lf = hook(&repo, "post-commit");
    fs::write(&path, lf.replace('\n', "\r\n"))?;
    repo.djf(&["hooks", "install"])
        .assert()
        .success()
        .stdout("dejafix git hooks refreshed: post-commit\n");
    assert_eq!(hook(&repo, "post-commit"), lf);
    repo.djf(&["hooks", "install"])
        .assert()
        .success()
        .stdout("dejafix git hooks already installed.\n");
    Ok(())
}

#[test]
fn init_outside_git_prints_the_note() -> TestResult {
    let repo = Repo::bare()?;
    fs::remove_dir_all(repo.root.join(".git"))?;
    let out = repo.stdout(&["init", "--no-mcp-config"])?;
    assert!(
        out.contains(
            "  Note: No .git directory found. Run `djf hooks install` after `git init`.\n"
        )
    );
    Ok(())
}

#[test]
fn hooks_install_refresh_and_uninstall() -> TestResult {
    let repo = Repo::new()?;
    repo.djf(&["hooks", "install"]).assert().success().stdout(
        "dejafix git hooks installed: post-commit, post-merge, pre-commit\n  Auto-captures: commits, reverts, merges, fixes, features, breaking changes.\n  Pre-commit: warns about repeating failed approaches and high-churn files.\n",
    );
    repo.djf(&["hooks"])
        .assert()
        .success()
        .stdout("dejafix git hooks already installed.\n");
    let pre_commit = repo.root.join(".git/hooks/pre-commit");
    let text = hook(&repo, "pre-commit").replace("#!/bin/sh\n", "#!/bin/sh\necho mine\n");
    fs::write(&pre_commit, text.replace("DJF_BIN=\"", "DJF_BIN=\"/old"))?;
    let broken = hook(&repo, "post-merge").replace("# <<< dejafix auto-capture <<<\n", "");
    fs::write(repo.root.join(".git/hooks/post-merge"), &broken)?;
    repo.djf(&["hooks", "install"])
        .assert()
        .success()
        .stdout("dejafix git hooks refreshed: pre-commit\n")
        .stderr("Warning: .git/hooks/post-merge has a dejafix start marker but no end marker, so it was left alone. Fix or delete that block, then re-run `djf hooks install`.\n");
    assert!(hook(&repo, "pre-commit").starts_with("#!/bin/sh\necho mine\n# >>> dejafix"));
    assert_eq!(hook(&repo, "post-merge"), broken);
    repo.djf(&["hooks", "uninstall"])
        .assert()
        .success()
        .stdout("dejafix hooks removed from: post-commit, post-merge, pre-commit\n");
    assert_eq!(hook(&repo, "pre-commit"), "#!/bin/sh\necho mine\n");
    assert!(!repo.root.join(".git/hooks/post-commit").exists());
    repo.djf(&["hooks", "uninstall"])
        .assert()
        .success()
        .stdout("No dejafix hooks found to remove.\n");
    repo.djf(&["hooks", "bogus"])
        .assert()
        .success()
        .stdout("Unknown action: bogus\n");
    fs::remove_dir_all(repo.root.join(".git"))?;
    repo.djf(&["hooks"])
        .assert()
        .success()
        .stdout("")
        .stderr("Error: .git/hooks directory not found. Is this a git repository?\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlinked_hook_is_written_through() -> TestResult {
    let repo = Repo::new()?;
    let shared = repo.home.join("shared-pre-commit");
    fs::write(&shared, "#!/bin/sh\necho shared\n")?;
    let link = repo.root.join(".git/hooks/pre-commit");
    std::os::unix::fs::symlink(&shared, &link)?;
    repo.djf(&["hooks", "install"]).assert().success();
    assert!(fs::symlink_metadata(&link)?.file_type().is_symlink());
    assert!(fs::read_to_string(&shared)?.contains("precheck --level warn"));
    Ok(())
}

fn wait_for_events(
    repo: &Repo,
    count: usize,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let started = Instant::now();
    loop {
        let events = repo.events()?;
        if events.len() >= count || started.elapsed() > Duration::from_secs(10) {
            return Ok(events);
        }
        sleep(Duration::from_millis(50));
    }
}

fn path_without_djf() -> Result<std::ffi::OsString, std::env::JoinPathsError> {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::env::split_paths(&inherited).filter(|directory| {
        ["djf", "dejafix"].iter().all(|name| {
            !directory
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
                .exists()
        })
    }))
}

#[test]
fn a_real_commit_records_the_event_and_is_never_blocked() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["init", "--no-mcp-config"]).assert().success();
    fs::create_dir_all(repo.root.join("src"))?;
    fs::write(repo.root.join("src/login.py"), "print('login')\n")?;
    git(&repo, &["add", "src/login.py"])?;
    let output = Command::new("git")
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(["commit", "-q", "-m", "fix: guard the login submit"])
        .current_dir(&repo.root)
        .env("HOME", &repo.home)
        .env("DEJAFIX_HOME", repo.home.join(".dejafix"))
        .env("PATH", path_without_djf()?)
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let events = wait_for_events(&repo, 1)?;
    let event = events.last().ok_or("no event")?;
    assert_eq!(event["type"], "fix");
    assert_eq!(event["summary"], "Fix: fix: guard the login submit");
    assert_eq!(event["location"], "src/login.py");
    assert_eq!(event["capture_source"], "git_post_commit");
    assert_eq!(event["auto_captured"], true);
    Ok(())
}

#[test]
fn auto_capture_classifies_deduplicates_and_respects_the_switch() -> TestResult {
    let repo = Repo::new()?;
    commit(
        &repo,
        "fix: guard the login submit",
        &[
            (".dejafix/summary.md", "# regenerated\n"),
            ("src/login.py", "print('login')\n"),
        ],
    )?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("[dejafix] Auto-captured: Fix: fix: guard the login submit\n");
    repo.djf(&["_auto-capture"]).assert().success().stdout("");
    let events = repo.events()?;
    assert_eq!(events.len(), 1);
    let event = events.first().ok_or("no event")?;
    assert_eq!(
        event["files"],
        serde_json::json!(["src/login.py", ".dejafix/summary.md"])
    );
    assert_eq!(event["command"], "auto-capture");
    assert_eq!(event["capture_confidence"], "high");
    assert_eq!(event["git_message"], "fix: guard the login submit");
    commit(&repo, "docs: explain the login", &[("README.md", "y\n")])?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("");
    commit(&repo, "chore: bump", &[("README.md", "z\n")])?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("");
    commit(
        &repo,
        "Revert \"fix: guard the login submit\"",
        &[("src/login.py", "x\n")],
    )?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("[dejafix] Auto-captured: Reverted: Revert \"fix: guard the login submit\"\n");
    repo.djf(&["_auto-capture", "merge"])
        .assert()
        .success()
        .stdout("");
    commit(
        &repo,
        "feat: add the profile page",
        &[("src/profile.py", "p\n")],
    )?;
    repo.djf(&["_auto-capture", "merge"])
        .assert()
        .success()
        .stdout("[dejafix] Auto-captured merge: feat: add the profile page\n");
    let events = repo.events()?;
    assert_eq!(events.len(), 3);
    assert_eq!(events.get(1).ok_or("no revert")?["outcome"], "failed");
    assert_eq!(
        events.get(2).ok_or("no merge")?["capture_source"],
        "git_post_merge"
    );
    let config = repo.mem().join("config.toml");
    let mut text = fs::read_to_string(&config)?;
    text.push_str("auto_capture = false\n");
    fs::write(&config, text)?;
    commit(&repo, "fix: one more", &[("src/login.py", "w\n")])?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("");
    assert_eq!(repo.events()?.len(), 3);
    assert!(
        repo.read(".dejafix/summary.md")?
            .contains("Merge: feat: add the profile page")
    );
    Ok(())
}

#[test]
fn auto_capture_outside_a_project_does_nothing() -> TestResult {
    let repo = Repo::bare()?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert!(!repo.mem().exists());
    Ok(())
}

#[test]
fn a_secret_in_a_commit_message_is_redacted() -> TestResult {
    let repo = Repo::new()?;
    let secret = format!("sk-{}", "a".repeat(48));
    commit(
        &repo,
        &format!("fix: rotate {secret}"),
        &[("src/a.py", "a\n")],
    )?;
    repo.djf(&["_auto-capture", "commit"])
        .assert()
        .success()
        .stdout("[dejafix] Auto-captured: Fix: fix: rotate [REDACTED:openai_key]\n")
        .stderr("dejafix: redacted 2 secret(s) before write (openai_key)\n");
    let text = repo.read(".dejafix/events.jsonl")?;
    assert!(!text.contains(&secret));
    assert!(text.contains("Fix: fix: rotate [REDACTED:openai_key]"));
    commit(
        &repo,
        &format!("feat: merge {secret}"),
        &[("src/b.py", "b\n")],
    )?;
    repo.djf(&["_auto-capture", "merge"])
        .assert()
        .success()
        .stdout("[dejafix] Auto-captured merge: feat: merge [REDACTED:openai_key]\n");
    Ok(())
}

#[test]
fn an_unreadable_hook_is_left_alone_with_its_own_warning() -> TestResult {
    let repo = Repo::new()?;
    let hook = repo.root.join(".git/hooks/pre-commit");
    fs::write(&hook, b"#!/bin/sh\necho caf\xe9\n")?;
    repo.djf(&["hooks", "install"]).assert().success().stderr(
        "Warning: .git/hooks/pre-commit could not be read as text, so it was left alone.\n",
    );
    assert_eq!(fs::read(&hook)?, b"#!/bin/sh\necho caf\xe9\n");
    Ok(())
}
