use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use crate::core::text::{python_strip, python_strip_matches};

const SOURCE_SUFFIXES: [&str; 64] = [
    "bash",
    "c",
    "cc",
    "cfg",
    "cjs",
    "clj",
    "conf",
    "cpp",
    "cs",
    "css",
    "dart",
    "dockerfile",
    "erl",
    "ex",
    "exs",
    "go",
    "gradle",
    "graphql",
    "h",
    "hpp",
    "hs",
    "html",
    "ini",
    "java",
    "jl",
    "js",
    "json",
    "jsx",
    "kt",
    "kts",
    "lock",
    "lua",
    "m",
    "md",
    "mjs",
    "mm",
    "nim",
    "php",
    "proto",
    "ps1",
    "py",
    "pyi",
    "r",
    "rb",
    "rs",
    "rst",
    "sass",
    "scala",
    "scss",
    "sh",
    "sql",
    "svelte",
    "swift",
    "tf",
    "toml",
    "ts",
    "tsx",
    "txt",
    "vue",
    "xml",
    "yaml",
    "yml",
    "zig",
    "zsh",
];

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static LINE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":\d+(?::\d+)?(\n?)\z").expect("valid line suffix pattern"));

pub fn location_to_file(location: &str) -> Option<Cow<'_, str>> {
    let located = python_strip_matches(python_strip(location), &['"', '\'']);
    if located.is_empty() {
        return None;
    }
    let drive_length = if has_drive(located) { 2 } else { 0 };
    let rest = located.get(drive_length..)?;
    let (body, newline) = match LINE_SUFFIX.captures(rest) {
        Some(found) => (
            rest.get(..found.get(0)?.start())?,
            found.get(1).is_some_and(|line| !line.is_empty()),
        ),
        None => (rest, false),
    };
    if body.is_empty() && !newline {
        return None;
    }
    let is_file = body.contains(['/', '\\'])
        || (!newline
            && body
                .rsplit_once('.')
                .is_some_and(|(_, suffix)| is_source_suffix(suffix)));
    if !is_file {
        return None;
    }
    let end = drive_length + body.len();
    let file = located.get(..end)?;
    Some(if newline {
        Cow::Owned(format!("{file}\n"))
    } else {
        Cow::Borrowed(file)
    })
}

fn has_drive(text: &str) -> bool {
    matches!(text.as_bytes(), [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic())
}

fn is_source_suffix(suffix: &str) -> bool {
    let lowered: Cow<'_, str> = if suffix
        .bytes()
        .any(|b| b.is_ascii_uppercase() || !b.is_ascii())
    {
        Cow::Owned(suffix.to_lowercase())
    } else {
        Cow::Borrowed(suffix)
    };
    !lowered.is_empty() && SOURCE_SUFFIXES.binary_search(&lowered.as_ref()).is_ok()
}
