use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_POLL: Duration = Duration::from_micros(500);
const LAST_POLL: Duration = Duration::from_millis(5);

#[must_use]
pub fn head_short(root: &Path) -> Option<String> {
    let mut child = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    let mut poll = FIRST_POLL;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < TIMEOUT => {
                sleep(poll);
                poll = (poll * 2).min(LAST_POLL);
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let commit = text.trim();
    (!commit.is_empty()).then(|| commit.to_owned())
}
