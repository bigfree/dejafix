use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::store::read_python_text;
use crate::core::scaffold::ManifestInfo;
use crate::core::text::{PYTHON_SPACE_CLASS, python_strip};

const MAX_ENTRY_POINTS: usize = 8;
const MAX_FOLDERS: usize = 15;
const NPM_SCRIPTS: [&str; 4] = ["start", "dev", "build", "test"];
const SCRIPTS_HEADING: &str = "[project.scripts]";

const COMMON_FOLDERS: [(&str, &str); 12] = [
    ("src", "package or application code"),
    ("lib", "library code"),
    ("app", "application code"),
    ("tests", "test suite"),
    ("test", "test suite"),
    ("docs", "documentation"),
    ("examples", "examples / demos"),
    ("scripts", "scripts and utilities"),
    ("public", "static public assets"),
    ("assets", "images / fonts / static assets"),
    ("components", "UI components"),
    ("pages", "route pages"),
];

const IGNORE_FOLDERS: [&str; 26] = [
    ".git",
    ".hg",
    ".svn",
    ".dejafix",
    ".idea",
    ".vscode",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "__pycache__",
    "node_modules",
    "venv",
    "env",
    "virtualenv",
    ".tox",
    ".eggs",
    "build",
    "dist",
    "target",
    ".gradle",
    "htmlcov",
    "coverage",
    ".next",
    ".nuxt",
    ".cache",
    "site-packages",
];

static DESCRIPTION: LazyLock<Option<Regex>> = LazyLock::new(|| {
    let s = PYTHON_SPACE_CLASS;
    Regex::new(&format!(r#"(?m)^{s}*description{s}*={s}*["'](.+?)["']"#)).ok()
});

static SCRIPT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    let s = PYTHON_SPACE_CLASS;
    Regex::new(&format!(r#"\A{s}*([a-zA-Z0-9_-]+){s}*={s}*["'](.+?)["']"#)).ok()
});

fn toml_description(path: &Path) -> Option<String> {
    let text = read_python_text(path, false).ok()?;
    let found = DESCRIPTION.as_ref()?.captures(&text)?.get(1)?;
    Some(python_strip(found.as_str()).to_owned())
}

fn package_json(root: &Path) -> Option<Value> {
    let text = read_python_text(&root.join("package.json"), true).ok()?;
    serde_json::from_str(&text).ok()
}

fn description(root: &Path, package: Option<&Value>) -> Option<String> {
    if let Some(found) = toml_description(&root.join("pyproject.toml")) {
        return Some(found);
    }
    if let Some(Value::String(text)) = package
        .and_then(Value::as_object)
        .and_then(|data| data.get("description"))
        && !python_strip(text).is_empty()
    {
        return Some(python_strip(text).to_owned());
    }
    toml_description(&root.join("Cargo.toml"))
}

fn pyproject_scripts(root: &Path, entries: &mut Vec<String>) {
    let Ok(text) = read_python_text(&root.join("pyproject.toml"), false) else {
        return;
    };
    let Some(script) = SCRIPT.as_ref() else {
        return;
    };
    let mut inside = false;
    for line in text.split('\n') {
        let stripped = python_strip(line);
        if stripped == SCRIPTS_HEADING {
            inside = true;
            continue;
        }
        if inside && stripped.starts_with('[') {
            inside = false;
            continue;
        }
        if inside
            && stripped.contains('=')
            && let Some(found) = script.captures(line)
            && let (Some(name), Some(target)) = (found.get(1), found.get(2))
        {
            entries.push(format!("`{}` → `{}`", name.as_str(), target.as_str()));
        }
    }
}

fn npm_scripts(package: Option<&Value>, entries: &mut Vec<String>) {
    let Some(data) = package.and_then(Value::as_object) else {
        return;
    };
    let empty = Value::Object(serde_json::Map::new());
    let scripts = data.get("scripts").unwrap_or(&empty);
    for key in NPM_SCRIPTS {
        let present = match scripts {
            Value::Object(map) => map.contains_key(key),
            Value::Array(items) => items.iter().any(|item| item.as_str() == Some(key)),
            Value::String(text) => text.contains(key),
            Value::Null | Value::Bool(_) | Value::Number(_) => return,
        };
        if !present {
            continue;
        }
        match scripts.get(key) {
            Some(Value::String(command)) => entries.push(format!("`npm run {key}` → `{command}`")),
            Some(_) => {}
            None => return,
        }
    }
}

fn sort_key(name: &str) -> String {
    if cfg!(windows) {
        name.to_lowercase()
    } else {
        name.to_owned()
    }
}

fn main_folders(root: &Path) -> Vec<(String, &'static str)> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by_cached_key(|name| sort_key(name));
    names
        .into_iter()
        .filter(|name| !name.starts_with('.') && !IGNORE_FOLDERS.contains(&name.as_str()))
        .take(MAX_FOLDERS)
        .map(|name| {
            let description = COMMON_FOLDERS
                .iter()
                .find(|(folder, _)| *folder == name)
                .map_or("", |(_, description)| *description);
            (name, description)
        })
        .collect()
}

#[must_use]
pub fn manifest_info(root: &Path) -> ManifestInfo {
    let package = package_json(root);
    let mut entry_points = Vec::new();
    pyproject_scripts(root, &mut entry_points);
    npm_scripts(package.as_ref(), &mut entry_points);
    entry_points.truncate(MAX_ENTRY_POINTS);
    ManifestInfo {
        description: description(root, package.as_ref()),
        entry_points,
        main_folders: main_folders(root),
    }
}
