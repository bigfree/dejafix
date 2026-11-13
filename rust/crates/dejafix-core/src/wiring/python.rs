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

pub(crate) fn python_io_error(error: &io::Error, path: &Path) -> (&'static str, String) {
    let shown = error.to_string();
    let text = shown.split(" (os error").next().unwrap_or(shown.as_str());
    let path = python_repr(&path.to_string_lossy());
    let message = match error.raw_os_error() {
        Some(code) => format!("[Errno {code}] {text}: {path}"),
        None => format!("{text}: {path}"),
    };
    (class(error), message)
}
