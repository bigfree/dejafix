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

pub const GIT_NO_MAINTENANCE: [&str; 4] = ["-c", "maintenance.auto=false", "-c", "gc.auto=0"];

pub fn isolated(program: impl AsRef<OsStr>, home: &Path, source: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for name in ["PATH", "LLVM_PROFILE_FILE", "SYSTEMROOT"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
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
            Action::Mcp(session) => (
                env!("CARGO_BIN_EXE_djf-mcp"),
                "dejafix.mcp_server",
                session
                    .args
                    .iter()
                    .map(|arg| arg.replace("{project}", &project.to_string_lossy()))
                    .collect(),
                None,
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
        outputs.push(match (action, stdin) {
            (Action::Mcp(session), _) => {
                let cwd = match session.cwd {
                    Some(relative) => temp.path().join(relative),
                    None => project.clone(),
                };
                fs::create_dir_all(&cwd)?;
                command.current_dir(&cwd);
                mcp_session(&mut command, &session.calls)?
            }
            (_, Some(stdin)) => output_with_stdin(&mut command, &stdin)?,
            (_, None) => output(&mut command)?,
        });
    }
    let mut files = BTreeMap::new();
    snapshot(&project, "project", &mut files)?;
    snapshot(&home, "home", &mut files)?;
    snapshot_git_hooks(implementation, &project, &mut files)?;
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
            stdout: normalizer.apply(&printed(implementation, &output.stdout)),
            stderr: normalizer.apply(&printed(implementation, &output.stderr)),
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

fn printed(implementation: Implementation, bytes: &[u8]) -> String {
    match implementation {
        Implementation::Python => console_text(bytes),
        Implementation::Rust => String::from_utf8_lossy(bytes).into_owned(),
    }
}

pub fn console_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    if cfg!(windows) {
        text.replace("\r\n", "\n")
    } else {
        text.into_owned()
    }
}

fn sorted(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let ordered: BTreeMap<&String, serde_json::Value> = map
                .iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            serde_json::Value::Object(
                ordered
                    .into_iter()
                    .map(|(key, value)| (key.clone(), value))
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sorted).collect())
        }
        other => other.clone(),
    }
}

fn mcp_transcript(name: &str, response: &serde_json::Value) -> Result<String, Box<dyn Error>> {
    let result = response
        .get("result")
        .ok_or_else(|| format!("{name}: {response}"))?;
    Ok(match name {
        "initialize" => format!(
            "### initialize\n{}\n",
            result
                .get("instructions")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
        ),
        "tools/list" => format!(
            "### tools/list\n{}\n",
            serde_json::to_string_pretty(&sorted(
                result.get("tools").unwrap_or(&serde_json::Value::Null)
            ))?
        ),
        _ => {
            let text = result
                .pointer("/content/0/text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let failed = result
                .get("isError")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || text.starts_with("dejafix tool error: ");
            let structured = result
                .pointer("/structuredContent/result")
                .and_then(serde_json::Value::as_str)
                == Some(text);
            format!("### {name} error={failed} structured={structured}\n{text}\n")
        }
    })
}

fn mcp_session(
    command: &mut Command,
    calls: &[(&str, serde_json::Value)],
) -> Result<Output, Box<dyn Error>> {
    use std::io::{BufRead, BufReader};
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", command.get_program().display()))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let reader = BufReader::new(child.stdout.take().ok_or("no stdout")?);
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in reader.lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut ask = |id: Option<usize>,
                   method: &str,
                   params: serde_json::Value|
     -> Result<serde_json::Value, Box<dyn Error>> {
        let mut message = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        if let Some(id) = id
            && let Some(object) = message.as_object_mut()
        {
            object.insert("id".to_owned(), serde_json::json!(id));
        }
        writeln!(stdin, "{message}")?;
        stdin.flush()?;
        let Some(id) = id else {
            return Ok(serde_json::Value::Null);
        };
        loop {
            let line = lines
                .recv_timeout(std::time::Duration::from_secs(60))
                .map_err(|error| format!("no reply from the MCP server: {error}"))??;
            let response: serde_json::Value = serde_json::from_str(&line)?;
            if response.get("id") == Some(&serde_json::json!(id)) {
                return Ok(response);
            }
        }
    };
    let initialized = ask(
        Some(0),
        "initialize",
        serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "golden", "version": "1"}}),
    )?;
    let mut transcript = mcp_transcript("initialize", &initialized)?;
    ask(None, "notifications/initialized", serde_json::json!({}))?;
    for (index, (name, arguments)) in calls.iter().enumerate() {
        let response = if *name == "tools/list" {
            ask(Some(index + 1), "tools/list", serde_json::json!({}))?
        } else {
            ask(
                Some(index + 1),
                "tools/call",
                serde_json::json!({"name": name, "arguments": arguments}),
            )?
        };
        transcript.push_str(&mcp_transcript(name, &response)?);
    }
    drop(ask);
    drop(stdin);
    let status = child.wait()?;
    Ok(Output {
        status,
        stdout: transcript.into_bytes(),
        stderr: Vec::new(),
    })
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
    implementation: Implementation,
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
        let text = match implementation {
            Implementation::Python if cfg!(windows) => text.replace("\r\n", "\n"),
            _ => text,
        };
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

fn at(path: &Path) -> impl FnOnce(std::io::Error) -> Box<dyn Error> + '_ {
    move |error| format!("{}: {error}", path.display()).into()
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(to).map_err(at(to))?;
    for entry in fs::read_dir(from).map_err(at(from))? {
        let entry = entry.map_err(at(from))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(at(&source))?.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, target).map_err(at(&source))?;
        }
    }
    Ok(())
}

fn snapshot(
    root: &Path,
    label: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root).map_err(at(root))? {
        let entry = entry.map_err(at(root))?;
        let path = entry.path();
        let key = format!("{label}/{}", entry.file_name().to_string_lossy());
        if key.ends_with("/.git") {
            continue;
        }
        if entry.file_type().map_err(at(&path))?.is_dir() {
            snapshot(&path, &key, files)?;
        } else {
            let text = String::from_utf8_lossy(&fs::read(&path).map_err(at(&path))?).into_owned();
            files.insert(key, text);
        }
    }
    Ok(())
}

fn now_iso() -> Result<String, Box<dyn Error>> {
    let seconds = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?;
    Ok(jiff::Timestamp::from_second(seconds)?.to_string())
}
