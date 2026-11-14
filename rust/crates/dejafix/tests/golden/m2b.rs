use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dejafix_core::{
    ai_instructions, bridge_block, initial_plan, initial_project_map, initial_summary,
    manifest_info,
};
use serde_json::{Value, json};

use crate::harness::{GIT_NO_MAINTENANCE, diff, fixed, run_scenario};
use crate::m2::oracle;
use crate::normalize::without_language_rules;
use crate::scenario::canonical;
use crate::types::{Arguments, Implementation};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn templates_match_python() -> TestResult {
    let dir = tempfile::tempdir()?;
    let python: Value = serde_json::from_str(&oracle(&["templates"], dir.path(), "")?)?;
    let rust = json!({
        "instructions": without_language_rules(ai_instructions()),
        "summary": initial_summary("demo"),
        "map": initial_project_map("demo"),
        "plan": initial_plan("demo"),
        "bridge": without_language_rules(&bridge_block(Some("demo"))),
        "anonymous": without_language_rules(&bridge_block(None)),
    });
    for key in [
        "instructions",
        "summary",
        "map",
        "plan",
        "bridge",
        "anonymous",
    ] {
        assert_eq!(python[key], rust[key], "{key}");
    }
    Ok(())
}

const MANIFESTS: &[&[(&str, &str)]] = &[
    &[(
        "pyproject.toml",
        "[project]\nname = \"demo\"\ndescription = \"A demo CLI.\"\n[project.scripts]\ndemo = \"demo.cli:main\"\n  other='x:y'\nbad line = \n[tool.x]\nz = \"no\"\n",
    )],
    &[(
        "package.json",
        "\u{feff}{\"description\": \" Node app \", \"scripts\": {\"start\": \"node .\", \"test\": 1, \"build\": \"tsc\"}}",
    )],
    &[
        (
            "package.json",
            "{\"scripts\": [\"dev\"], \"description\": \"\"}",
        ),
        ("Cargo.toml", "[package]\ndescription = 'Rust \"thing\"'\n"),
    ],
    &[("package.json", "not json")],
    &[(
        "pyproject.toml",
        "description = \"\"\n  description = \" spaced \"\n",
    )],
    &[],
];

#[test]
fn manifest_extraction_matches_python() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut roots = Vec::new();
    for (index, files) in MANIFESTS.iter().enumerate() {
        let root = dir.path().join(format!("case{index}"));
        fs::create_dir_all(&root)?;
        for (name, text) in *files {
            fs::write(root.join(name), text)?;
        }
        for folder in [
            "src",
            "tests",
            ".hidden",
            "node_modules",
            "Zeta",
            "alpha",
            "build",
        ] {
            fs::create_dir_all(root.join(folder))?;
        }
        roots.push(root);
    }
    let arguments: Vec<String> = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect();
    let mut args = vec!["manifest"];
    args.extend(arguments.iter().map(String::as_str));
    let expected = oracle(&args, dir.path(), "")?;
    for (root, python) in roots.iter().zip(expected.lines()) {
        let info = manifest_info(root);
        let rust = json!([info.description, info.entry_points, info.main_folders]);
        assert_eq!(
            serde_json::from_str::<Value>(python)?,
            rust,
            "{}",
            root.display()
        );
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let status = Command::new("git")
        .args(GIT_NO_MAINTENANCE)
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(args)
        .current_dir(dir)
        .status()?;
    if !status.success() {
        return Err(format!("git {args:?} failed").into());
    }
    Ok(())
}

fn decision_id(root: &Path, summary: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let text = fs::read_to_string(root.join(".dejafix/events.jsonl"))?;
    let id = text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|event| {
            event.get("type").and_then(Value::as_str) == Some("decision")
                && event.get("summary").and_then(Value::as_str) == Some(summary)
        })
        .and_then(|event| event.get("id").and_then(Value::as_str).map(str::to_owned))
        .ok_or("the first decision is missing")?;
    Ok(vec![
        "decision".to_owned(),
        "Session state lives in signed cookies".to_owned(),
        "--supersedes".to_owned(),
        id,
    ])
}

const INIT: &[&str] = &[
    "init",
    "--no-hooks",
    "--no-mcp-config",
    "--no-structure",
    "--no-backfill",
    "--no-watch",
];

fn cli_fixture(fixture: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let project = fixture.join("project");
    fs::create_dir_all(project.join("src"))?;
    fs::create_dir_all(project.join("tools/.dejafix"))?;
    fs::write(
        project.join("package.json"),
        r#"{"description": "Login service", "dependencies": {"react": "18"}, "scripts": {"dev": "vite"}}"#,
    )?;
    git(&project, &["init", "-q"])?;
    let home = fixture.join("home");
    fs::create_dir_all(home.join(".dejafix/global"))?;
    fs::write(
        home.join(".dejafix/global/library_gotchas.jsonl"),
        "{\"discovered\": \"2026-01-01T00:00:00Z\", \"gotcha\": \"strict mode mounts twice in development\", \"id\": \"got_0123456789ab\", \"library\": \"react\", \"source_project\": \"web\", \"tags\": [], \"version_range\": null}\n",
    )?;
    Ok((project, home))
}

#[test]
fn cli_scenario_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let (project, home) = cli_fixture(fixture.path())?;
    let steps: Vec<Arguments<'_>> = vec![
        fixed(INIT),
        fixed(&[
            "log",
            "Login redirect loops",
            "--at",
            "src/auth/redirect.py:88",
        ]),
        fixed(&[
            "attempt",
            "gotcha: react router v6 drops the state on redirect",
            "--failed",
        ]),
        fixed(&[
            "fix",
            "Guard the redirect in one middleware",
            "--at",
            "src/auth/redirect.py:91",
        ]),
        fixed(&["decision", "Session state lives in the server"]),
        Box::new(|root: &Path| decision_id(root, "Session state lives in the server")),
        fixed(&[
            "note",
            "gotcha: react strict mode mounts twice in development",
        ]),
        fixed(&["search", "session"]),
        fixed(&["show"]),
        fixed(&["plan", "Move auth to its own package"]),
        fixed(&["plan"]),
        fixed(&["map"]),
        fixed(&["instructions"]),
        fixed(&["project", "list"]),
        fixed(&["project", "register", "tools", "--alias", "tools"]),
        fixed(&["project", "tag", "TOOLS", "Web App"]),
        fixed(&["project", "use", "tools"]),
        fixed(&["project", "list"]),
        fixed(&["project", "alias", "tools", "Bad"]),
        fixed(&["project", "use", "nope"]),
        fixed(&["project", "register", "src"]),
        fixed(&["project", "tag", "tools", "web-app", "--remove"]),
        fixed(&["project", "remove", "tools"]),
        fixed(&["project", "use"]),
        fixed(INIT),
        fixed(&[
            "init",
            "--no-hooks",
            "--no-mcp-config",
            "--no-structure",
            "--no-backfill",
            "--no-watch",
            "--global-tags",
            "python",
        ]),
    ];
    let python = canonical(run_scenario(
        Implementation::Python,
        &project,
        Some(&home),
        &steps,
    )?);
    let rust = canonical(run_scenario(
        Implementation::Rust,
        &project,
        Some(&home),
        &steps,
    )?);
    let differences = diff(&python, &rust);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert!(
        python
            .steps
            .iter()
            .take(14)
            .all(|step| step.code == Some(0)),
        "{:?}",
        python.steps
    );
    for file in [
        "project/CLAUDE.md",
        "project/AGENTS.md",
        "project/.gitignore",
        "project/.dejafix/plan.md",
        "home/.dejafix/projects.json",
        "home/.dejafix/projects.meta.json",
    ] {
        assert!(python.files.contains_key(file), "{file}");
    }
    Ok(())
}
