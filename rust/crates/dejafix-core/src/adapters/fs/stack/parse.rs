use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::super::store::read_python_text;
use super::types::{Found, Parser};
use crate::core::text::{
    PYTHON_SPACE_CLASS, contains_word, python_lines, python_split, python_strip,
};

static DEPENDENCY_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    let space = PYTHON_SPACE_CLASS;
    Regex::new(&format!(
        r#"\A{space}*"?([a-zA-Z0-9_-]+)(?:\[.*?\])?(?:{space}*[><=!~]+.*)?"?{space}*,?{space}*\z"#
    ))
    .ok()
});

static DEPENDENCY_HEADER: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?i)\A\[.*dependencies.*\]").ok());

const KNOWN_PYTHON_LIBRARIES: [(&str, &str); 17] = [
    ("sqlalchemy", "sqlalchemy"),
    ("pydantic", "pydantic"),
    ("fastapi", "fastapi"),
    ("flask", "flask"),
    ("django", "django"),
    ("pytest", "pytest"),
    ("typer", "typer"),
    ("click", "click"),
    ("celery", "celery"),
    ("redis", "redis"),
    ("numpy", "numpy"),
    ("pandas", "pandas"),
    ("torch", "pytorch"),
    ("tensorflow", "tensorflow"),
    ("aiohttp", "aiohttp"),
    ("httpx", "httpx"),
    ("alembic", "alembic"),
];

const PACKAGE_TAGS: [(&str, &str); 7] = [
    ("react", "react"),
    ("vue", "vue"),
    ("next", "nextjs"),
    ("express", "express"),
    ("vite", "vite"),
    ("tailwindcss", "tailwind"),
    ("typescript", "typescript"),
];

fn read_text(path: &Path, strip_bom: bool) -> Option<String> {
    read_python_text(path, strip_bom).ok()
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    python_lines(text.as_bytes()).filter_map(|(_, line)| std::str::from_utf8(line).ok())
}

pub(super) fn parse(parser: Parser, path: &Path) -> Found {
    let mut found = Found::default();
    match parser {
        Parser::Pyproject => pyproject(path, &mut found),
        Parser::Requirements => requirements(path, &mut found),
        Parser::PackageJson => package_json(path, &mut found),
        Parser::Cargo => cargo(path, &mut found),
        Parser::GoMod => go_mod(path, &mut found),
    }
    found
}

fn pyproject(path: &Path, found: &mut Found) {
    let (Some(text), Some(line_pattern), Some(header)) = (
        read_text(path, false),
        DEPENDENCY_LINE.as_ref(),
        DEPENDENCY_HEADER.as_ref(),
    ) else {
        return;
    };
    let mut in_dependencies = false;
    for line in text.split('\n') {
        if header.is_match(line) {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && line.starts_with('[') {
            in_dependencies = false;
            continue;
        }
        if in_dependencies
            && let Some(name) = line_pattern
                .captures(line)
                .and_then(|captures| captures.get(1))
        {
            let library = name.as_str().to_lowercase();
            let library = library.trim_matches('"').trim_matches('\'');
            if !library.is_empty() && !matches!(library, "python" | "dev" | "test" | "docs" | "all")
            {
                found.libraries.push(library.to_owned());
            }
        }
    }
    let lowered = text.to_lowercase();
    for (key, tag) in KNOWN_PYTHON_LIBRARIES {
        if contains_word(&lowered, key) {
            found.tags.push(tag.to_owned());
            found.libraries.push(key.to_owned());
        }
    }
}

fn requirements(path: &Path, found: &mut Found) {
    let Some(text) = read_text(path, false) else {
        return;
    };
    for line in lines(&text) {
        let line = python_strip(line);
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let head = line
            .split(['>', '<', '=', '!', '~', '[', ';', '@'])
            .next()
            .unwrap_or(line);
        let library = python_strip(head).to_lowercase();
        if !library.is_empty() {
            found.libraries.push(library);
        }
    }
}

fn package_names(value: &Value) -> Result<Vec<String>, ()> {
    match value {
        Value::Object(map) => Ok(map.keys().cloned().collect()),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or(()))
            .collect(),
        Value::String(text) => Ok(text.chars().map(String::from).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) => Err(()),
    }
}

fn package_json(path: &Path, found: &mut Found) {
    let Some(text) = read_text(path, true) else {
        return;
    };
    let Ok(Value::Object(data)) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    for key in ["dependencies", "devDependencies"] {
        let names = match data.get(key) {
            None => Ok(Vec::new()),
            Some(value) => package_names(value),
        };
        let Ok(names) = names else {
            return;
        };
        for name in names {
            found.libraries.push(name.to_lowercase());
            if let Some((_, tag)) = PACKAGE_TAGS.iter().find(|(package, _)| *package == name) {
                found.tags.push((*tag).to_owned());
            }
        }
    }
}

fn cargo(path: &Path, found: &mut Found) {
    let Some(text) = read_text(path, false) else {
        return;
    };
    let mut in_dependencies = false;
    for line in lines(&text) {
        if line.contains("[dependencies]") || line.contains("[dev-dependencies]") {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && line.starts_with('[') {
            in_dependencies = false;
            continue;
        }
        if in_dependencies && line.contains('=') {
            let library = python_strip(line.split('=').next().unwrap_or(""));
            if !library.is_empty() {
                found.libraries.push(library.to_lowercase());
            }
        }
    }
}

fn go_mod(path: &Path, found: &mut Found) {
    let Some(text) = read_text(path, false) else {
        return;
    };
    for line in lines(&text) {
        let line = python_strip(line);
        if line.is_empty() || line.starts_with("module") || line.starts_with("go ") {
            continue;
        }
        if line.starts_with("require") || line.starts_with(')') {
            continue;
        }
        if let Some(first) = python_split(line).next() {
            let library = first.rsplit('/').next().unwrap_or(first);
            found.libraries.push(library.to_lowercase());
        }
    }
}
