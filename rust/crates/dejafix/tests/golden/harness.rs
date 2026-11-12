use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Implementation, Normalizer, Run, Step, StepOutput};

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
    let since = now_iso()?;
    let temp = tempfile::tempdir()?;
    let project = temp.path().join("project");
    let home = temp.path().join("home");
    copy_tree(fixture, &project)?;
    fs::create_dir_all(&home)?;
    let repository = repository()?;
    let source = repository.join("src");
    let interpreter = python(&repository);
    let outputs = steps
        .iter()
        .map(|args| {
            let mut command = match implementation {
                Implementation::Python => {
                    let mut command = isolated(&interpreter, &home, &source);
                    command.args(["-m", "dejafix.cli"]);
                    command
                }
                Implementation::Rust => isolated(env!("CARGO_BIN_EXE_djf"), &home, &source),
            };
            output(command.args(*args).current_dir(&project))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut files = BTreeMap::new();
    snapshot(&project, "project", &mut files)?;
    snapshot(&home, "home", &mut files)?;
    let mut normalizer = Normalizer::new(&[(&project, "<PROJECT>"), (&home, "<HOME>")], &since)?;
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
