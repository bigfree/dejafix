use std::sync::LazyLock;

use regex::Regex;

use super::types::Change;

const RANK_SOURCE: u8 = 0;
const RANK_TEST: u8 = 1;
const RANK_OTHER: u8 = 2;
const RANK_DOC: u8 = 3;
const RANK_BUILD: u8 = 4;
const RANK_LOCK: u8 = 5;
const MAX_FILES: usize = 10;

const CODE_SUFFIXES: &[&str] = &[
    "py", "pyi", "js", "jsx", "ts", "tsx", "mjs", "cjs", "rs", "go", "rb", "php", "java", "kt",
    "kts", "swift", "c", "h", "cc", "cpp", "hpp", "cs", "m", "mm", "sh", "bash", "zsh", "ps1",
    "sql", "html", "css", "scss", "sass", "vue", "svelte", "gradle", "tf", "proto", "graphql",
    "ex", "exs", "erl", "hs", "scala", "clj", "lua", "r", "jl", "dart", "zig", "nim",
];
const TEST_DIRS: &[&str] = &["tests", "test", "__tests__", "spec"];
const DOC_SUFFIXES: &[&str] = &[".md", ".rst", ".adoc"];
const DOC_DIRS: &[&str] = &["docs", "doc"];
const DOC_STEMS: &[&str] = &[
    "readme",
    "changelog",
    "changes",
    "notice",
    "license",
    "licence",
    "copying",
    "authors",
    "contributing",
    "code_of_conduct",
    "security",
];
const BUILD_NAMES: &[&str] = &[
    "makefile",
    "cmakelists.txt",
    "justfile",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "tox.ini",
    "noxfile.py",
    "conftest.py",
    "manifest.in",
    "cargo.toml",
    "go.mod",
    "gemfile",
    "rakefile",
    "pipfile",
    "composer.json",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "pom.xml",
    "mix.exs",
    "deno.json",
    "procfile",
    "vagrantfile",
];
const ASSET_SUFFIXES: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "tiff", "psd", "ttf", "otf", "woff",
    "woff2", "eot", "mp3", "wav", "ogg", "flac", "mp4", "webm", "mov", "avi", "pdf", "zip", "gz",
    "tgz", "tar", "bz2", "xz", "7z", "rar", "jar", "whl", "bin", "dll", "so", "dylib", "exe",
];
const LOCK_NAMES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "uv.lock",
    "pipfile.lock",
    "cargo.lock",
    "gemfile.lock",
    "composer.lock",
    "go.sum",
];

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static TEST_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A(?:test_.*|.*_test\.[^.]+|.*\.(?:spec|test)\.[^.]+)\n?\z")
        .expect("valid test name pattern")
});

#[expect(
    clippy::expect_used,
    reason = "a literal pattern, exercised by the tests"
)]
static BUILD_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\A(?:dockerfile(?:\..+)?|.+\.dockerfile|docker-compose.*\.ya?ml",
        r"|requirements.*\.txt|constraints.*\.txt|tsconfig.*\.json|jsconfig.*\.json",
        r"|.+\.config\.(?:js|cjs|mjs|ts))\n?\z"
    ))
    .expect("valid build name pattern")
});

#[must_use]
pub fn parse_name_status(bytes: &[u8]) -> Vec<Change> {
    let text = String::from_utf8_lossy(bytes);
    let fields: Vec<&str> = text.split('\0').collect();
    let mut changes = Vec::new();
    let mut index = 0;
    while let (Some(code), Some(path)) = (fields.get(index), fields.get(index + 1)) {
        let Some(status) = code.chars().next() else {
            break;
        };
        index += 2;
        let path = if matches!(status, 'R' | 'C') {
            let Some(renamed) = fields.get(index) else {
                break;
            };
            index += 1;
            renamed
        } else {
            path
        };
        changes.push(Change {
            status,
            path: (*path).to_owned(),
        });
    }
    changes
}

#[must_use]
pub fn location_rank(path: &str) -> u8 {
    let (directories, name) = path.rsplit_once('/').unwrap_or(("", path));
    let first = path.split('/').next().unwrap_or(path);
    let lower = name.to_lowercase();
    let (stem, rest) = lower.split_once('.').unwrap_or((&lower, ""));
    let suffix = if rest.is_empty() {
        ""
    } else {
        rest.rsplit('.').next().unwrap_or("")
    };
    if LOCK_NAMES.contains(&lower.as_str()) || suffix == "lock" {
        return RANK_LOCK;
    }
    if first.starts_with('.')
        || lower.starts_with('.')
        || BUILD_NAMES.contains(&lower.as_str())
        || BUILD_NAME.is_match(&lower)
        || ASSET_SUFFIXES.contains(&suffix)
    {
        return RANK_BUILD;
    }
    if CODE_SUFFIXES.contains(&suffix) {
        let in_test_directory = directories
            .split('/')
            .any(|part| TEST_DIRS.contains(&part.to_lowercase().as_str()));
        return if in_test_directory || TEST_NAME.is_match(&lower) {
            RANK_TEST
        } else {
            RANK_SOURCE
        };
    }
    if DOC_SUFFIXES.iter().any(|ending| lower.ends_with(ending))
        || DOC_STEMS.contains(&stem)
        || DOC_DIRS.contains(&first.to_lowercase().as_str())
    {
        return RANK_DOC;
    }
    RANK_OTHER
}

pub fn pick_location(changes: &[Change], is_memory: impl Fn(&str) -> bool) -> Option<&str> {
    changes
        .iter()
        .filter(|change| change.status != 'D' && !is_memory(&change.path))
        .min_by_key(|change| location_rank(&change.path))
        .map(|change| change.path.as_str())
}

pub fn capture_files(
    changes: &[Change],
    location: Option<&str>,
    is_memory: impl Fn(&str) -> bool,
) -> Vec<String> {
    let (memory, code): (Vec<&str>, Vec<&str>) = changes
        .iter()
        .map(|change| change.path.as_str())
        .filter(|path| Some(*path) != location)
        .partition(|path| is_memory(path));
    location
        .into_iter()
        .chain(code)
        .chain(memory)
        .take(MAX_FILES)
        .map(str::to_owned)
        .collect()
}
