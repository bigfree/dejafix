use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use super::types::GitFailure;

const TIMEOUT: Duration = Duration::from_secs(5);
const FIRST_POLL: Duration = Duration::from_micros(500);
const LAST_POLL: Duration = Duration::from_millis(5);

pub(crate) fn git<S: AsRef<OsStr>>(root: &Path, args: &[S]) -> Option<Vec<u8>> {
    git_status(root, args).ok()
}

pub(crate) fn git_status<S: AsRef<OsStr>>(root: &Path, args: &[S]) -> Result<Vec<u8>, GitFailure> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| GitFailure::Unavailable)?;
    let started = Instant::now();
    let mut poll = FIRST_POLL;
    let mut stdout = child.stdout.take().ok_or(GitFailure::Unavailable)?;
    let reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut stdout, &mut buffer).map(|_| buffer)
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = reader
                    .join()
                    .ok()
                    .and_then(Result::ok)
                    .ok_or(GitFailure::Unavailable)?;
                return if status.success() {
                    Ok(output)
                } else {
                    Err(GitFailure::Exit)
                };
            }
            Ok(None) if started.elapsed() < TIMEOUT => {
                sleep(poll);
                poll = (poll * 2).min(LAST_POLL);
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GitFailure::Unavailable);
            }
        }
    }
}
