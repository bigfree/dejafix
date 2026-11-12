use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::time::Instant;

use dejafix_core::{EditHookRun, edit_hook, python_json};
use serde_json::{Value, json};

use crate::config::HookConfig;

const LESSON_HEADER: &str = "dejafix lessons for ";

#[must_use]
pub fn main() -> ExitCode {
    let started = Instant::now();
    let config = HookConfig::from_env();
    let mut raw = Vec::new();
    let _ = io::stdin().lock().read_to_end(&mut raw);
    let run = edit_hook(&raw, &config.cwd, &config.temp);
    if !run.output.is_empty() {
        let mut stdout = io::stdout().lock();
        let _ = writeln!(stdout, "{}", run.output);
        let _ = stdout.flush();
    }
    if let Some(log) = &config.log {
        let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
        let at = jiff::Timestamp::now()
            .to_zoned(HookConfig::zone())
            .strftime("%Y-%m-%dT%H:%M:%S%z")
            .to_string();
        let line = call_record(&run, &at, elapsed);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log) {
            let _ = writeln!(file, "{line}");
        }
    }
    ExitCode::SUCCESS
}

#[must_use]
pub fn call_record(run: &EditHookRun, at: &str, elapsed_ms: f64) -> String {
    let rounded = (elapsed_ms * 10.0).round() / 10.0;
    let record: Value = json!({
        "at": at,
        "event": run.event,
        "tool": run.tool,
        "session": run.session,
        "agent": run.agent,
        "cwd": run.cwd,
        "files": run.files,
        "reported": run.output.matches(LESSON_HEADER).count(),
        "chars": run.output.chars().count(),
        "ms": rounded,
        "error": run.error,
    });
    python_json(&record)
}
