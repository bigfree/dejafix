use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

pub type TestResult = Result<(), Box<dyn Error>>;

pub struct Repo {
    _dir: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let status = Command::new("git")
        .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
        .args(args)
        .current_dir(dir)
        .status()?;
    if !status.success() {
        return Err(format!("git {args:?} failed").into());
    }
    Ok(())
}

impl Repo {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?.join("repo");
        let home = root.with_file_name("home");
        fs::create_dir_all(root.join(".dejafix/issues"))?;
        fs::create_dir_all(&home)?;
        fs::write(root.join(".dejafix/config.toml"), "recent_days = 30\n")?;
        fs::write(root.join(".dejafix/events.jsonl"), "")?;
        fs::write(root.join("README.md"), "x\n")?;
        git(&root, &["init", "-q"])?;
        git(&root, &["add", "README.md"])?;
        git(&root, &["commit", "-qm", "init"])?;
        Ok(Self {
            _dir: dir,
            root,
            home,
        })
    }

    pub fn djf_in(&self, cwd: &Path, args: &[&str]) -> assert_cmd::Command {
        let mut command = assert_cmd::Command::new(env!("CARGO_BIN_EXE_djf"));
        command.env_clear();
        for name in ["PATH", "LLVM_PROFILE_FILE", "SYSTEMROOT"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("DEJAFIX_HOME", self.home.join(".dejafix"))
            .current_dir(cwd)
            .args(args);
        command
    }

    pub fn djf(&self, args: &[&str]) -> assert_cmd::Command {
        self.djf_in(&self.root, args)
    }

    pub fn mem(&self) -> PathBuf {
        self.root.join(".dejafix")
    }

    pub fn events(&self) -> Result<Vec<Value>, Box<dyn Error>> {
        fs::read_to_string(self.mem().join("events.jsonl"))?
            .lines()
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }

    pub fn stdout(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let output = self.djf(args).output()?;
        Ok(String::from_utf8(output.stdout)?)
    }
}
