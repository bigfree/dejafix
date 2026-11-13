use std::io;
use std::path::Path;

fn class(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "FileNotFoundError",
        io::ErrorKind::PermissionDenied => "PermissionError",
        io::ErrorKind::IsADirectory => "IsADirectoryError",
        io::ErrorKind::NotADirectory => "NotADirectoryError",
        io::ErrorKind::InvalidData => "UnicodeDecodeError",
        _ => "OSError",
    }
}

fn python_repr(text: &str) -> String {
    if text.contains('\'') && !text.contains('"') {
        return format!("\"{text}\"");
    }
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn windows_errno(error: &io::Error) -> Option<(&'static str, i32, &'static str)> {
    if !cfg!(windows) {
        return None;
    }
    match (error.raw_os_error(), error.kind()) {
        (_, io::ErrorKind::NotFound) => Some(("FileNotFoundError", 2, "No such file or directory")),
        (Some(32 | 33), _) | (_, io::ErrorKind::PermissionDenied) => {
            Some(("PermissionError", 13, "Permission denied"))
        }
        _ => None,
    }
}

pub(crate) fn python_io_error(error: &io::Error, path: &Path) -> (&'static str, String) {
    let shown = error.to_string();
    let text = shown.split(" (os error").next().unwrap_or(shown.as_str());
    let path = python_repr(&path.to_string_lossy());
    match (windows_errno(error), error.raw_os_error()) {
        (Some((class, code, text)), _) => (class, format!("[Errno {code}] {text}: {path}")),
        (None, Some(code)) => (class(error), format!("[Errno {code}] {text}: {path}")),
        (None, None) => (class(error), format!("{text}: {path}")),
    }
}
