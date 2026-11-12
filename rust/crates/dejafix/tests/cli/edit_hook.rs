use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::support::TestResult;

const ISSUE: &str = r#"{"type": "issue", "issue_id": "0001", "summary": "token refresh race", "location": "src/a.py:12"}"#;
const FAILED: &str = r#"{"type": "attempt", "issue_id": "0001", "outcome": "failed", "summary": "wrapped refresh in a lock", "timestamp": "2026-10-02T10:00:00"}"#;
const DECISION: &str = r#"{"type": "decision", "id": "d1", "summary": "tokens rotate per user", "files": ["src/a.py"], "timestamp": "2026-09-12T10:00:00"}"#;

struct Hook {
    _dir: TempDir,
    root: PathBuf,
    base: PathBuf,
    temp: PathBuf,
    log: Option<PathBuf>,
}

fn write_events(root: &Path, lines: &[&str]) -> TestResult {
    let file = root.join(".dejafix/events.jsonl");
    fs::create_dir_all(file.parent().ok_or("no parent")?)?;
    let mut text = fs::read_to_string(&file).unwrap_or_default();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(file, text)?;
    Ok(())
}

impl Hook {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let base = fs::canonicalize(dir.path())?;
        let root = base.join("repo");
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/a.py"), "x = 1\n")?;
        fs::write(root.join("src/b.py"), "y = 1\n")?;
        write_events(&root, &[ISSUE, FAILED, DECISION])?;
        let temp = base.join("tmp");
        fs::create_dir_all(&temp)?;
        Ok(Self {
            _dir: dir,
            root,
            base,
            temp,
            log: None,
        })
    }

    fn raw(&self, stdin: &[u8]) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let mut command = assert_cmd::Command::new(env!("CARGO_BIN_EXE_djf-edit-hook"));
        command.env_clear();
        for name in ["PATH", "LLVM_PROFILE_FILE", "SYSTEMROOT"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in ["TMPDIR", "TEMP", "TMP"] {
            command.env(name, &self.temp);
        }
        if let Some(log) = &self.log {
            command.env("DJF_EDIT_HOOK_LOG", log);
        }
        let output = command
            .current_dir(&self.base)
            .write_stdin(stdin)
            .output()?;
        assert!(output.status.success());
        assert_eq!(output.stderr, b"");
        Ok(output)
    }

    fn context(&self, payload: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let output = self.raw(payload.to_string().as_bytes())?;
        let text = String::from_utf8(output.stdout)?;
        if text.is_empty() {
            return Ok(text);
        }
        assert!(text.ends_with('\n') && text.matches('\n').count() == 1);
        let parsed: Value = serde_json::from_str(&text)?;
        assert_eq!(
            parsed.pointer("/hookSpecificOutput/hookEventName"),
            Some(&json!("PreToolUse"))
        );
        Ok(parsed
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    fn edit(&self, relative: &str, session: &str, agent: &str) -> Value {
        let mut payload = json!({
            "session_id": session,
            "tool_name": "Edit",
            "tool_input": {"file_path": self.root.join(relative).to_string_lossy()},
        });
        if !agent.is_empty()
            && let Some(object) = payload.as_object_mut()
        {
            object.insert("agent_id".to_owned(), json!(agent));
        }
        payload
    }
}

fn patch(cwd: &Path, headers: &[&str]) -> Value {
    let mut lines = vec!["*** Begin Patch"];
    lines.extend(headers);
    lines.extend(["@@", "-x", "+y", "*** End Patch"]);
    json!({"session_id": "s1", "cwd": cwd.to_string_lossy(), "tool_name": "apply_patch", "tool_input": {"command": lines.join("\n")}})
}

#[test]
fn claude_code_edits_get_lessons_once_per_session() -> TestResult {
    let hook = Hook::new()?;
    let edit = hook.edit("src/a.py", "s1", "");
    assert_eq!(
        hook.context(&edit)?.lines().collect::<Vec<_>>(),
        [
            "dejafix lessons for src/a.py:",
            "  Open issues on this file:",
            "    - #0001 token refresh race",
            "  Earlier attempts (1 failed, 0 partial), newest first:",
            "    x wrapped refresh in a lock (#0001, 2026-10-02)",
            "  Decisions on this file:",
            "    - tokens rotate per user (2026-09-12)",
        ]
    );
    assert_eq!(hook.context(&edit)?, "");
    assert_ne!(hook.context(&hook.edit("src/a.py", "s2", ""))?, "");
    assert_ne!(hook.context(&hook.edit("src/a.py", "s1", "agent-1"))?, "");
    assert_eq!(hook.context(&hook.edit("src/a.py", "s1", "agent-1"))?, "");
    write_events(
        &hook.root,
        &[
            r#"{"type": "attempt", "issue_id": "0001", "outcome": "partial", "summary": "retry halves failures"}"#,
        ],
    )?;
    assert!(hook.context(&edit)?.contains("retry halves failures"));
    assert_eq!(hook.context(&hook.edit("src/b.py", "s1", ""))?, "");
    let mut anonymous = hook.edit("src/a.py", "", "");
    anonymous
        .as_object_mut()
        .ok_or("not an object")?
        .remove("session_id");
    assert_ne!(hook.context(&anonymous)?, "");
    assert_ne!(hook.context(&anonymous)?, "");
    Ok(())
}

#[test]
fn compaction_resets_one_session_only() -> TestResult {
    let hook = Hook::new()?;
    for reset in [
        json!({"session_id": "s1", "hook_event_name": "SessionStart", "source": "compact"}),
        json!({"session_id": "s1", "hook_event_name": "PostCompact"}),
    ] {
        let edit = hook.edit("src/a.py", "s1", "");
        let other = hook.edit("src/a.py", "s2", "");
        hook.context(&edit)?;
        hook.context(&other)?;
        assert_eq!(hook.context(&edit)?, "");
        assert_eq!(hook.context(&reset)?, "");
        assert_ne!(hook.context(&edit)?, "");
        assert_eq!(hook.context(&other)?, "");
    }
    assert_eq!(
        hook.context(&json!({"session_id": "", "hook_event_name": "PostCompact"}))?,
        ""
    );
    Ok(())
}

#[test]
fn codex_patches_and_several_projects() -> TestResult {
    let hook = Hook::new()?;
    write_events(
        &hook.root,
        &[
            r#"{"type": "issue", "issue_id": "0002", "summary": "b drifts", "location": "src/b.py"}"#,
        ],
    )?;
    let context = hook.context(&patch(
        &hook.root,
        &[
            "*** Update File: src/a.py",
            "*** Add File: src/new.py",
            "*** Update File: src/b.py",
        ],
    ))?;
    assert!(context.starts_with("dejafix lessons for src/a.py:"));
    assert!(context.contains("\n\ndejafix lessons for src/b.py:"));
    assert!(!context.contains("src/new.py"));
    let other = hook.base.join("other");
    write_events(
        &other,
        &[r#"{"type": "issue", "issue_id": "0009", "summary": "other bug", "location": "c.py"}"#],
    )?;
    let mut two = patch(
        &hook.base,
        &[
            "*** Update File: repo/src/a.py",
            "*** Update File: other/c.py",
        ],
    );
    two["session_id"] = json!("s2");
    let both = hook.context(&two)?;
    assert!(both.contains("#0001 token refresh race") && both.contains("#0009 other bug"));
    Ok(())
}

#[test]
fn worktrees_memory_files_and_strays() -> TestResult {
    let hook = Hook::new()?;
    let worktree = hook.root.join(".claude/worktrees/feature");
    fs::create_dir_all(worktree.join("src"))?;
    write_events(
        &worktree,
        &[r#"{"type": "note", "summary": "worktree-local note"}"#],
    )?;
    let edit = json!({"session_id": "w", "tool_input": {"file_path": worktree.join("src/a.py").to_string_lossy()}});
    assert!(hook.context(&edit)?.contains("#0001 token refresh race"));
    let plain = hook.base.join("plain/.claude/worktrees/feature");
    write_events(&plain, &[ISSUE])?;
    let edit = json!({"session_id": "w", "tool_input": {"file_path": plain.join("src/a.py").to_string_lossy()}});
    assert!(hook.context(&edit)?.contains("#0001 token refresh race"));
    write_events(
        &hook.root,
        &[
            r#"{"type": "decision", "id": "d9", "summary": "keep config tidy", "files": [".dejafix/events.jsonl", ".claude/settings.json"]}"#,
        ],
    )?;
    for edited in [".dejafix/events.jsonl", ".claude/settings.json"] {
        assert_eq!(hook.context(&hook.edit(edited, "s9", ""))?, "");
    }
    let stray = json!({"session_id": "s", "tool_input": {"file_path": hook.base.join("stray.py").to_string_lossy()}});
    assert_eq!(hook.context(&stray)?, "");
    Ok(())
}

#[test]
fn malformed_input_and_unreadable_logs_are_silent() -> TestResult {
    let hook = Hook::new()?;
    for stdin in [
        &b""[..],
        b"not json",
        b"[]",
        b"{}",
        br#"{"tool_input": "x"}"#,
        b"\xff\xfe",
        br#"{"session_id": "s", "tool_input": {"file_path": "a\u0000b"}}"#,
    ] {
        assert_eq!(hook.raw(stdin)?.stdout, b"");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let events = hook.root.join(".dejafix/events.jsonl");
        fs::set_permissions(&events, fs::Permissions::from_mode(0o000))?;
        let mut logged = Hook::new()?;
        logged.root.clone_from(&hook.root);
        logged.base.clone_from(&hook.base);
        logged.log = Some(hook.base.join("calls.jsonl"));
        let silent = logged.context(&hook.edit("src/a.py", "s1", ""));
        fs::set_permissions(&events, fs::Permissions::from_mode(0o644))?;
        assert_eq!(silent?, "");
        let record: Value =
            serde_json::from_str(fs::read_to_string(hook.base.join("calls.jsonl"))?.trim())?;
        assert!(
            record["error"]
                .as_str()
                .unwrap_or_default()
                .starts_with("PermissionError: ")
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn shared_or_symlinked_dedup_directories_are_not_trusted() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let hook = Hook::new()?;
    let owner = fs::metadata(&hook.temp).map(|meta| std::os::unix::fs::MetadataExt::uid(&meta))?;
    let shared = hook.temp.join(format!("djf-edit-hook-{owner}"));
    fs::create_dir(&shared)?;
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o777))?;
    fs::create_dir(shared.join("s1"))?;
    let edit = hook.edit("src/a.py", "s1", "");
    assert_ne!(hook.context(&edit)?, "");
    assert_ne!(hook.context(&edit)?, "");
    hook.context(
        &json!({"session_id": "s1", "hook_event_name": "SessionStart", "source": "compact"}),
    )?;
    assert!(shared.join("s1").is_dir());
    assert_eq!(fs::read_dir(shared.join("s1"))?.count(), 0);
    fs::remove_dir_all(&shared)?;
    let target = hook.base.join("elsewhere");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
    std::os::unix::fs::symlink(&target, &shared)?;
    assert_ne!(hook.context(&edit)?, "");
    assert_ne!(hook.context(&edit)?, "");
    assert_eq!(fs::read_dir(&target)?.count(), 0);
    Ok(())
}

#[test]
fn broken_or_unusable_dedup_storage_still_shows_lessons() -> TestResult {
    let mut hook = Hook::new()?;
    let edit = hook.edit("src/a.py", "s1", "");
    assert_ne!(hook.context(&edit)?, "");
    let session = fs::read_dir(&hook.temp)?
        .next()
        .ok_or("no dedup directory")??
        .path()
        .join("s1");
    fs::remove_dir_all(&session)?;
    fs::create_dir(&session)?;
    fs::write(session.join("main"), "a file where a directory belongs")?;
    assert_ne!(hook.context(&edit)?, "");
    assert_ne!(hook.context(&edit)?, "");
    let not_a_directory = hook.base.join("tmp-file");
    fs::write(&not_a_directory, "")?;
    hook.temp = not_a_directory;
    assert_ne!(hook.context(&edit)?, "");
    assert_ne!(hook.context(&edit)?, "");
    assert_eq!(
        hook.context(&json!({"session_id": "s1", "hook_event_name": "PostCompact"}))?,
        ""
    );
    Ok(())
}

#[test]
fn the_call_log_records_calls_without_lesson_text() -> TestResult {
    let mut hook = Hook::new()?;
    let log = hook.base.join("calls.jsonl");
    hook.log = Some(log.clone());
    hook.context(&patch(
        &hook.root,
        &["*** Update File: src/a.py", "*** Update File: src/b.py"],
    ))?;
    hook.context(&json!({"session_id": "s1", "hook_event_name": "PostCompact"}))?;
    let text = fs::read_to_string(&log)?;
    assert!(!text.contains("token refresh race"));
    let records: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let first = records.first().ok_or("no record")?;
    assert_eq!(first["tool"], "apply_patch");
    assert_eq!(first["session"], "s1");
    assert_eq!(first["cwd"], hook.root.to_string_lossy().as_ref());
    assert_eq!(first["files"], 2);
    assert_eq!(first["reported"], 1);
    assert_eq!(first["error"], "");
    assert!(first["chars"].as_u64().unwrap_or_default() > 0);
    assert!(first["ms"].is_f64());
    let second = records.get(1).ok_or("no second record")?;
    assert_eq!(
        (
            second["event"].as_str(),
            second["files"].as_u64(),
            second["reported"].as_u64()
        ),
        (Some("PostCompact"), Some(0), Some(0))
    );
    hook.log = Some(hook.base.join("missing-dir/calls.jsonl"));
    assert_ne!(hook.context(&hook.edit("src/a.py", "s5", ""))?, "");
    hook.log = None;
    let before: Vec<_> = fs::read_dir(&hook.base)?.collect();
    hook.context(&hook.edit("src/a.py", "s6", ""))?;
    assert_eq!(fs::read_dir(&hook.base)?.count(), before.len());
    Ok(())
}

#[test]
fn a_writer_holding_the_log_lock_never_blocks_an_edit() -> TestResult {
    let hook = Hook::new()?;
    let log = fs::OpenOptions::new()
        .read(true)
        .append(true)
        .open(hook.root.join(".dejafix/events.jsonl"))?;
    log.lock()?;
    assert!(
        hook.context(&hook.edit("src/a.py", "locked", ""))?
            .contains("token refresh race")
    );
    log.unlock()?;
    Ok(())
}
