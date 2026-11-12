use std::io;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

use super::types::EditHookRun;
use crate::adapters::fs::{
    FsResolver, MEMORY_DIR, events_path, forget_session, posix, read_events_unlocked,
    report_is_new, resolve_path,
};
use crate::core::lessons::{Memory, edited_paths, hook_output, is_skipped, main_checkout, records};

const RESET_EVENTS: [&str; 2] = ["SessionStart", "PostCompact"];
const ERROR_CHARS: usize = 300;

#[must_use]
pub fn edit_hook(raw: &[u8], cwd: &Path, temp: &Path) -> EditHookRun {
    let payload = payload(raw);
    let text = |key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut run = EditHookRun {
        event: text("hook_event_name"),
        tool: text("tool_name"),
        session: text("session_id"),
        agent: text("agent_id"),
        cwd: text("cwd"),
        ..EditHookRun::default()
    };
    let paths = edited_paths(payload.get("tool_input"));
    run.files = paths.len();
    if RESET_EVENTS.contains(&run.event.as_str()) {
        forget_session(temp, &run.session);
        return run;
    }
    let base = if run.cwd.is_empty() {
        cwd.to_path_buf()
    } else {
        PathBuf::from(&run.cwd)
    };
    match lessons(&paths, &base, temp, &run.session, &run.agent) {
        Ok(context) if !context.is_empty() => run.output = hook_output(&context),
        Ok(_) => {}
        Err(error) => run.error = error.chars().take(ERROR_CHARS).collect(),
    }
    run
}

fn payload(raw: &[u8]) -> Map<String, Value> {
    let body = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(raw);
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn lessons(
    paths: &[String],
    cwd: &Path,
    temp: &Path,
    session: &str,
    agent: &str,
) -> Result<String, String> {
    let mut reports = Vec::new();
    for (root, relatives) in by_project(paths, cwd)? {
        let memory_dir = root.join(MEMORY_DIR);
        let bytes = read_events_unlocked(&memory_dir).map_err(|error| {
            let path = events_path(&memory_dir);
            std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<io::Error>())
                .map_or_else(
                    || format!("OSError: {error}"),
                    |source| python_error(source, &path),
                )
        })?;
        let text = String::from_utf8_lossy(&bytes);
        let found = records(&text);
        let root_posix = posix(&root);
        let memory = Memory::new(&found, &root_posix, &FsResolver);
        for relative in relatives {
            let report = memory.report(&relative);
            if !report.is_empty()
                && report_is_new(
                    temp,
                    session,
                    agent,
                    &format!("{root_posix}/{relative}"),
                    &report,
                )
            {
                reports.push(report);
            }
        }
    }
    Ok(reports.join("\n\n"))
}

fn by_project(paths: &[String], cwd: &Path) -> Result<Vec<(PathBuf, Vec<String>)>, String> {
    let mut grouped: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for raw in paths {
        if raw.contains('\0') || cwd.as_os_str().to_string_lossy().contains('\0') {
            return Err("ValueError: embedded null byte".to_owned());
        }
        let Some(resolved) = resolve_path(raw, None, cwd) else {
            continue;
        };
        let Some((root, relative)) = locate(&resolved) else {
            continue;
        };
        if is_skipped(&relative) {
            continue;
        }
        match grouped.iter_mut().find(|(known, _)| *known == root) {
            Some((_, relatives)) => {
                if !relatives.contains(&relative) {
                    relatives.push(relative);
                }
            }
            None => grouped.push((root, vec![relative])),
        }
    }
    Ok(grouped)
}

fn locate(path: &Path) -> Option<(PathBuf, String)> {
    let main = PathBuf::from(main_checkout(&posix(path)));
    let candidates = if main == path {
        vec![main]
    } else {
        vec![main, path.to_path_buf()]
    };
    candidates.into_iter().find_map(|candidate| {
        let root = candidate
            .ancestors()
            .skip(1)
            .find(|ancestor| events_path(&ancestor.join(MEMORY_DIR)).is_file())?
            .to_path_buf();
        let relative = candidate
            .strip_prefix(&root)
            .ok()?
            .components()
            .filter_map(|component| match component {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        Some((root, relative))
    })
}

fn python_error(error: &io::Error, path: &Path) -> String {
    let name = match error.kind() {
        io::ErrorKind::PermissionDenied => "PermissionError",
        io::ErrorKind::NotFound => "FileNotFoundError",
        io::ErrorKind::IsADirectory => "IsADirectoryError",
        io::ErrorKind::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    };
    let message = error.to_string();
    let message = message
        .split(" (os error")
        .next()
        .unwrap_or(message.as_str());
    match error.raw_os_error() {
        Some(code) => format!("{name}: [Errno {code}] {message}: '{}'", path.display()),
        None => format!("{name}: {message}: '{}'", path.display()),
    }
}
