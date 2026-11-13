use std::fs;
use std::path::Path;

use crate::core::text::python_strip;

const PID_FILE: &str = "watch.pid";

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    match rustix::process::test_kill_process(pid) {
        Ok(()) => true,
        Err(error) => error == rustix::io::Errno::PERM,
    }
}

#[cfg(windows)]
fn alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\"")))
}

#[must_use]
pub fn live_watcher(legacy: &Path) -> Option<u32> {
    let text = fs::read_to_string(legacy.join(PID_FILE)).ok()?;
    let digits = python_strip(&text);
    let pid: u32 = digits
        .strip_prefix('+')
        .unwrap_or(digits)
        .parse()
        .ok()
        .filter(|pid| *pid > 0)?;
    alive(pid).then_some(pid)
}
