use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Action, Arguments, Implementation, Normalizer, Run, Step, StepOutput};

pub fn python(repository: &Path) -> PathBuf {
    if let Some(python) = std::env::var_os("DEJAFIX_PYTHON") {
        return PathBuf::from(python);
    }
    let venv = repository.join(".venv");
    [venv.join("bin/python"), venv.join("Scripts/python.exe")]
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("python3"))
}

pub fn repository() -> Result<PathBuf, Box<dyn Error>> {
    Ok(fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."),
    )?)
}

pub fn isolated(program: impl AsRef<OsStr>, home: &Path, source: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for name in ["PATH", "LLVM_PROFILE_FILE"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("HOME", home)
        .env("DEJAFIX_HOME", home.join(".dejafix"))
        .env("PYTHONPATH", source)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONIOENCODING", "utf-8");
    command
}

pub fn output(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    command.output().map_err(|error| {
        let program = command.get_program().display();
        format!("cannot run {program}: {error}; set DEJAFIX_PYTHON to a Python with dejafix's dependencies").into()
    })
}

pub fn run(
    implementation: Implementation,
    fixture: &Path,
    steps: &[Step],
) -> Result<Run, Box<dyn Error>> {
    let steps: Vec<Arguments<'_>> = steps.iter().map(|args| fixed(args)).collect();
    run_scenario(implementation, fixture, None, &steps)
}

pub fn fixed<'a>(args: &'a [&'a str]) -> Arguments<'a> {
    Box::new(move |_| Ok(args.iter().map(|arg| (*arg).to_owned()).collect()))
}

pub fn python_version(repository: &Path) -> Result<String, Box<dyn Error>> {
    let init = fs::read_to_string(repository.join("src/dejafix/__init__.py"))?;
    init.lines()
        .find_map(|line| line.strip_prefix("__version__ = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "no __version__ in src/dejafix/__init__.py".into())
}

pub fn run_scenario(
    implementation: Implementation,
    fixture: &Path,
    home_fixture: Option<&Path>,
    steps: &[Arguments<'_>],
) -> Result<Run, Box<dyn Error>> {
    let actions: Vec<Action<'_>> = steps
        .iter()
        .map(|step| Action::Djf(Box::new(move |project: &Path| step(project))))
        .collect();
    run_actions(implementation, fixture, home_fixture, &actions)
}

const GIT_HOOKS: [&str; 3] = ["post-commit", "post-merge", "pre-commit"];

pub fn run_actions(
    implementation: Implementation,
    fixture: &Path,
    home_fixture: Option<&Path>,
    actions: &[Action<'_>],
) -> Result<Run, Box<dyn Error>> {
    let since = now_iso()?;
    let temp = tempfile::tempdir()?;
    let project = temp.path().join("project");
    let home = temp.path().join("home");
    let scratch = temp.path().join("tmp");
    copy_tree(fixture, &project)?;
    match home_fixture {
        Some(source) => copy_tree(source, &home)?,
        None => fs::create_dir_all(&home)?,
    }
    fs::create_dir_all(&scratch)?;
    let repository = repository()?;
    let source = repository.join("src");
    let interpreter = python(&repository);
    let mut outputs = Vec::with_capacity(actions.len());
    for action in actions {
        let (program, module, args, stdin) = match action {
            Action::Djf(arguments) => (
                env!("CARGO_BIN_EXE_djf"),
                "dejafix.cli",
                arguments(&project)?,
                None,
            ),
            Action::EditHook(payload) => (
                env!("CARGO_BIN_EXE_djf-edit-hook"),
                "dejafix.edit_hook",
                Vec::new(),
                Some(payload(&project)?),
            ),
        };
        let mut command = match implementation {
            Implementation::Python => {
                let mut command = isolated(&interpreter, &home, &source);
                command.args(["-m", module]);
                command
            }
            Implementation::Rust => isolated(program, &home, &source),
        };
        for name in ["TMPDIR", "TEMP", "TMP"] {
            command.env(name, &scratch);
        }
        command.args(&args).current_dir(&project);
        outputs.push(match stdin {
            Some(stdin) => output_with_stdin(&mut command, &stdin)?,
            None => output(&mut command)?,
        });
    }
    let mut files = BTreeMap::new();
    snapshot(&project, "project", &mut files)?;
    snapshot(&home, "home", &mut files)?;
    snapshot_git_hooks(&project, &mut files)?;
    let python_version = python_version(&repository)?;
    let mut normalizer = Normalizer::new(
        &[
            (&project, "<PROJECT>"),
            (&home, "<HOME>"),
            (&scratch, "<TMP>"),
            (temp.path(), "<ROOT>"),
        ],
        &since,
    )?
    .with_versions(&[&python_version, env!("CARGO_PKG_VERSION")])?;
    let steps = outputs
        .into_iter()
        .map(|output| StepOutput {
            code: output.status.code(),
            stdout: normalizer.apply(&String::from_utf8_lossy(&output.stdout)),
            stderr: normalizer.apply(&String::from_utf8_lossy(&output.stderr)),
        })
        .collect();
    let files = files
        .into_iter()
        .map(|(path, text)| {
            let text = normalizer.apply(&text);
            (path, text)
        })
        .collect();
    Ok(Run { steps, files })
}

fn output_with_stdin(command: &mut Command, stdin: &str) -> Result<Output, Box<dyn Error>> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", command.get_program().display()))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(stdin.as_bytes())?;
    Ok(child.wait_with_output()?)
}

fn snapshot_git_hooks(
    project: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    let hooks = project.join(".git/hooks");
    for name in GIT_HOOKS {
        let path = hooks.join(name);
        if !path.is_file() {
            continue;
        }
        let text = String::from_utf8_lossy(&fs::read(&path)?).into_owned();
        files.insert(format!("project/.git/hooks/{name}"), text);
        files.insert(format!("project/.git/hooks/{name} (mode)"), mode(&path)?);
    }
    Ok(())
}

#[cfg(unix)]
fn mode(path: &Path) -> Result<String, Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt;
    Ok(format!(
        "{:o}",
        fs::metadata(path)?.permissions().mode() & 0o777
    ))
}

#[cfg(not(unix))]
fn mode(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(format!(
        "readonly={}",
        fs::metadata(path)?.permissions().readonly()
    ))
}

pub fn diff(left: &Run, right: &Run) -> Vec<String> {
    let count = (left.steps.len() != right.steps.len())
        .then(|| format!("step count {} != {}", left.steps.len(), right.steps.len()));
    let steps = left
        .steps
        .iter()
        .zip(&right.steps)
        .enumerate()
        .filter(|(_, (l, r))| l != r)
        .map(|(index, (l, r))| format!("step {index}: {l:?} != {r:?}"));
    let keys: BTreeSet<&String> = left.files.keys().chain(right.files.keys()).collect();
    let files =
        keys.into_iter()
            .filter_map(|key| match (left.files.get(key), right.files.get(key)) {
                (Some(l), Some(r)) if l == r => None,
                (Some(l), Some(r)) => Some(first_difference(key, l, r)),
                (Some(_), None) => Some(format!("{key}: only on the left")),
                _ => Some(format!("{key}: only on the right")),
            });
    count.into_iter().chain(steps).chain(files).collect()
}

fn first_difference(key: &str, left: &str, right: &str) -> String {
    let mut left_lines = left.split_inclusive('\n');
    let mut right_lines = right.split_inclusive('\n');
    let mut number = 1;
    loop {
        match (left_lines.next(), right_lines.next()) {
            (Some(l), Some(r)) if l == r => number += 1,
            (l, r) => {
                let l = l.unwrap_or("<end>");
                let r = r.unwrap_or("<end>");
                return format!("{key}:{number}: {l:?} != {r:?}");
            }
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn snapshot(
    root: &Path,
    label: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let key = format!("{label}/{}", entry.file_name().to_string_lossy());
        if key.ends_with("/.git") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            snapshot(&entry.path(), &key, files)?;
        } else {
            let text = String::from_utf8_lossy(&fs::read(entry.path())?).into_owned();
            files.insert(key, text);
        }
    }
    Ok(())
}

fn now_iso() -> Result<String, Box<dyn Error>> {
    let seconds = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?;
    Ok(jiff::Timestamp::from_second(seconds)?.to_string())
}
