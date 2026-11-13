use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::harness::{diff, fixed, run_scenario};
use crate::scenario::canonical;
use crate::types::{Arguments, Implementation};

type TestResult = Result<(), Box<dyn Error>>;
type Prepared = Result<Vec<String>, Box<dyn Error>>;

const PINNED: &str =
    r#"{"mcpServers": {"dejafix": {"command": "djf-mcp", "args": ["--root", "/work/one"]}}}"#;
const CLEAN: &str = r#"{"mcpServers": {"dejafix": {"command": "djf-mcp", "args": []}}}"#;

fn home_of(project: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let parent = project.parent().ok_or("the project has no parent")?;
    Ok(dejafix_core::canonical_path(&parent.join("home")).ok_or("the home is missing")?)
}

fn memory(path: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(path.join(".dejafix"))?;
    fs::write(path.join(".dejafix/config.toml"), "recent_days = 30\n")?;
    Ok(())
}

fn doctor_fixture(fixture: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let project = fixture.join("project");
    fs::create_dir_all(&project)?;
    let home = fixture.join("home");
    memory(&home.join("code/a"))?;
    memory(&home.join("code/b"))?;
    fs::create_dir_all(home.join("code/c/src"))?;
    fs::create_dir_all(home.join("code/node_modules/pkg/.dejafix"))?;
    fs::create_dir_all(home.join(".cursor"))?;
    fs::create_dir_all(home.join(".codex/config.toml"))?;
    fs::write(home.join(".claude.json"), PINNED)?;
    fs::write(home.join(".cursor/mcp.json"), CLEAN)?;
    Ok((project, home))
}

fn seed_registry(project: &Path) -> Prepared {
    let home = home_of(project)?;
    let listed = Value::from(vec![
        home.join("code/a").display().to_string(),
        home.join("code/c").display().to_string(),
    ]);
    fs::create_dir_all(home.join(".dejafix"))?;
    fs::write(home.join(".dejafix/projects.json"), listed.to_string())?;
    Ok(vec!["doctor".to_owned()])
}

fn pin_cursor(project: &Path) -> Prepared {
    fs::write(home_of(project)?.join(".cursor/mcp.json"), PINNED)?;
    Ok(vec!["doctor".to_owned()])
}

fn under_home(project: &Path, args: &[&str], relative: &str) -> Prepared {
    let mut out: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let target = home_of(project)?.join(relative);
    out.insert(2, target.display().to_string());
    Ok(out)
}

fn add_nested(project: &Path) -> Prepared {
    memory(&home_of(project)?.join("code/nested/d"))?;
    under_home(project, &["project", "scan", "--dry-run"], "code")
}

#[test]
fn doctor_matches_python() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let (project, home) = doctor_fixture(fixture.path())?;
    let steps: Vec<Arguments<'_>> = vec![
        Box::new(seed_registry),
        fixed(&["doctor", "--fix"]),
        fixed(&["doctor"]),
        Box::new(pin_cursor),
        Box::new(add_nested),
        Box::new(|project: &Path| under_home(project, &["project", "scan"], "code")),
        Box::new(|project: &Path| under_home(project, &["project", "scan"], "code")),
        Box::new(|project: &Path| under_home(project, &["project", "scan"], "missing")),
        Box::new(|project: &Path| under_home(project, &["doctor", "--path", "-d", "2"], "code")),
        fixed(&["project", "list"]),
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
    let first = python.steps.first().ok_or("no steps")?;
    assert!(first.stdout.contains("not registered"), "{}", first.stdout);
    assert!(
        python
            .steps
            .get(3)
            .is_some_and(|step| step.stdout.contains("was clean on <MINUTE>")),
        "{:?}",
        python.steps.get(3)
    );
    for file in [
        "home/.dejafix/projects.json",
        "home/.dejafix/projects.meta.json",
        "home/.dejafix/doctor-state.json",
    ] {
        assert!(python.files.contains_key(file), "{file}");
    }
    Ok(())
}
