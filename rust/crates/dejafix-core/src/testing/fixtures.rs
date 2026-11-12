use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use jiff::Timestamp;
use serde_json::{Map, Value, json};

use super::types::{FixtureError, FixtureState, SplitMix64};
use crate::core::json::write_canonical;

pub const FIXTURE_SIZES: [usize; 3] = [100, 3_000, 30_000];
pub const FIXTURE_SEED: u64 = 0x00de_7afa_0000_0001;

const START: i64 = 1_704_067_200;
const WINDOW: usize = 63_072_000;
const CONFIG: &str = "recent_days = 30\nproject_description = \"\"\n";
const FILES: [&str; 8] = [
    "src/app.py",
    "src/auth/session.py",
    "src/api/routes.ts",
    "web/components/Login.tsx",
    "crates/core/src/lib.rs",
    "C:\\work\\legacy\\Report.cs",
    "docs/setup.md",
    "scripts/deploy.sh",
];
const TOPICS: [&str; 8] = [
    "login loop after token refresh",
    "flaky upload test on CI",
    "slow summary rebuild",
    "Unicode path – naïve façade",
    "deploy script exits 0 on failure",
    "race in session cache",
    "\"quoted\" config value is dropped",
    "release build 🚀 misses assets",
];
const OUTCOMES: [&str; 3] = ["failed", "partial", "worked"];
const SOURCES: [&str; 5] = [
    "git_post_commit",
    "git_post_revert",
    "git_post_merge",
    "churn_detector",
    "ci_parser",
];
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];

pub fn write_project(root: &Path, events: usize, seed: u64) -> Result<(), FixtureError> {
    let mem = root.join(".dejafix");
    fs::create_dir_all(mem.join("issues")).map_err(at(&mem))?;
    let config = mem.join("config.toml");
    fs::write(&config, CONFIG).map_err(at(&config))?;
    let log = mem.join("events.jsonl");
    let mut out = BufWriter::new(File::create(&log).map_err(at(&log))?);
    let mut state = FixtureState {
        rng: SplitMix64 { state: seed },
        clock: START,
        next_issue: 1,
        open_issues: Vec::new(),
        decisions: Vec::new(),
    };
    let step = (WINDOW / events.max(1)).max(1);
    for number in 0..events {
        let event = state.event(number, step)?;
        write_canonical(&mut out, &event).map_err(|source| FixtureError::Event {
            path: log.clone(),
            source,
        })?;
        out.write_all(b"\n").map_err(at(&log))?;
    }
    out.flush().map_err(at(&log))
}

fn at(path: &Path) -> impl FnOnce(io::Error) -> FixtureError {
    let path = path.to_path_buf();
    move |source| FixtureError::Io { path, source }
}

impl FixtureState {
    fn event(&mut self, number: usize, step: usize) -> Result<Value, FixtureError> {
        let mut event = Map::new();
        let id = format!(
            "evt_{:016x}{:04x}",
            self.rng.next(),
            self.rng.next() & 0xffff
        );
        self.clock += i64::try_from(self.rng.below(step)).unwrap_or(0) + 1;
        event.insert("id".into(), json!(id));
        event.insert(
            "timestamp".into(),
            json!(Timestamp::from_second(self.clock)?.to_string()),
        );
        let topic = self.rng.pick(&TOPICS);
        let file = self.rng.pick(&FILES);
        let kind = self.kind();
        event.insert("type".into(), json!(kind));
        match kind {
            "issue" => {
                let issue = format!("{:04}", self.next_issue);
                self.next_issue += 1;
                self.open_issues.push(issue.clone());
                event.insert("issue_id".into(), json!(issue));
                event.insert(
                    "location".into(),
                    json!(format!("{file}:{}", self.rng.below(400) + 1)),
                );
                event.insert("summary".into(), json!(format!("{topic} (#{number})")));
            }
            "hypothesis" | "attempt" => {
                let index = self.rng.below(self.open_issues.len());
                let issue = self.open_issues.get(index).cloned().unwrap_or_default();
                event.insert("issue_id".into(), json!(issue));
                event.insert(
                    "summary".into(),
                    json!(format!("Try: {topic} via {file} (#{number})")),
                );
                if kind == "attempt" {
                    event.insert("outcome".into(), json!(self.rng.pick(&OUTCOMES)));
                    event.insert("files".into(), json!([file]));
                    if self.rng.below(3) == 0 {
                        event.insert(
                            "command".into(),
                            json!(format!("pytest -x tests/test_{number}.py")),
                        );
                    }
                }
            }
            "fix" => {
                let index = self.rng.below(self.open_issues.len());
                let issue = self.open_issues.swap_remove(index);
                event.insert("issue_id".into(), json!(issue));
                event.insert("files".into(), json!([file, self.rng.pick(&FILES)]));
                event.insert(
                    "git_commit".into(),
                    json!(format!(
                        "{:016x}{:016x}{:08x}",
                        self.rng.next(),
                        self.rng.next(),
                        self.rng.next() & 0xffff_ffff
                    )),
                );
                event.insert(
                    "summary".into(),
                    json!(format!("Fixed {topic} (#{number})")),
                );
            }
            "decision" => {
                if !self.decisions.is_empty() && self.rng.below(5) == 0 {
                    let index = self.rng.below(self.decisions.len());
                    event.insert(
                        "supersedes".into(),
                        json!(self.decisions.swap_remove(index)),
                    );
                }
                self.decisions.push(id);
                event.insert(
                    "summary".into(),
                    json!(format!("Decide: keep {file} for {topic} (#{number})")),
                );
            }
            _ => {
                event.insert(
                    "summary".into(),
                    json!(format!("Note on {topic} (#{number})")),
                );
                if self.rng.below(4) == 0 {
                    event.insert("auto_captured".into(), json!(true));
                    event.insert("capture_source".into(), json!(self.rng.pick(&SOURCES)));
                    event.insert(
                        "capture_confidence".into(),
                        json!(self.rng.pick(&CONFIDENCE)),
                    );
                    event.insert("git_message".into(), json!(format!("chore: touch {file}")));
                }
            }
        }
        if self.rng.below(100) == 0 {
            event.insert("x_fixture_extra".into(), json!({"kept": true, "n": number}));
        }
        Ok(Value::Object(event))
    }

    fn kind(&mut self) -> &'static str {
        let roll = self.rng.below(100);
        if self.open_issues.is_empty() {
            return if roll < 40 { "issue" } else { "note" };
        }
        match roll {
            0..15 => "issue",
            15..25 => "hypothesis",
            25..60 => "attempt",
            60..72 => "fix",
            72..85 => "decision",
            _ => "note",
        }
    }
}
