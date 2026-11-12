use std::collections::BTreeSet;
use std::path::Path;

use super::parse;
use super::types::{Detector, Parser};

use crate::core::global::Stack;

const DETECTORS: [Detector; 16] = [
    detector("pyproject.toml", &["python"], Some(Parser::Pyproject)),
    detector("requirements.txt", &["python"], Some(Parser::Requirements)),
    detector("setup.py", &["python"], None),
    detector("Pipfile", &["python", "pipenv"], None),
    detector("package.json", &["javascript"], Some(Parser::PackageJson)),
    detector("tsconfig.json", &["typescript"], None),
    detector("bun.lockb", &["bun"], None),
    detector("Cargo.toml", &["rust"], Some(Parser::Cargo)),
    detector("go.mod", &["go"], Some(Parser::GoMod)),
    detector("Gemfile", &["ruby"], None),
    detector("pom.xml", &["java", "maven"], None),
    detector("build.gradle", &["java", "gradle"], None),
    detector("build.gradle.kts", &["kotlin", "gradle"], None),
    detector("Dockerfile", &["docker"], None),
    detector("docker-compose.yml", &["docker"], None),
    Detector {
        file: ".github/workflows",
        tags: &["github-actions"],
        parser: None,
        directory: true,
    },
];

const FRAMEWORKS: [(&str, &str); 10] = [
    ("fastapi", "fastapi"),
    ("flask", "flask"),
    ("django", "django"),
    ("react", "react"),
    ("vue", "vue"),
    ("next", "nextjs"),
    ("express", "express"),
    ("actix-web", "actix"),
    ("axum", "axum"),
    ("gin", "gin"),
];

const fn detector(
    file: &'static str,
    tags: &'static [&'static str],
    parser: Option<Parser>,
) -> Detector {
    Detector {
        file,
        tags,
        parser,
        directory: false,
    }
}

#[must_use]
pub fn detect_stack(root: &Path) -> Stack {
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut libraries: BTreeSet<String> = BTreeSet::new();
    let mut manifest_files = Vec::new();
    for found in DETECTORS {
        let target = root.join(found.file);
        let present = if found.directory {
            target.is_dir()
        } else {
            target.exists()
        };
        if !present {
            continue;
        }
        tags.extend(found.tags.iter().map(|tag| (*tag).to_owned()));
        manifest_files.push(found.file);
        if let Some(parser) = found.parser {
            let parsed = parse::parse(parser, &target);
            tags.extend(parsed.tags);
            libraries.extend(parsed.libraries);
        }
    }
    let mut frameworks: BTreeSet<String> = BTreeSet::new();
    for library in &libraries {
        let lowered = library.to_lowercase();
        let tokens: BTreeSet<&str> = lowered
            .split(['-', '_', '/', '@', '.'])
            .chain(std::iter::once(lowered.as_str()))
            .collect();
        for (key, framework) in FRAMEWORKS {
            if tokens.contains(key) {
                frameworks.insert(framework.to_owned());
                tags.insert(framework.to_owned());
            }
        }
    }
    Stack {
        tags: tags.into_iter().collect(),
        libraries: libraries.into_iter().collect(),
        frameworks: frameworks.into_iter().collect(),
        manifest_files,
    }
}
