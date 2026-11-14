use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::errors::io_error;
use super::types::{
    FsProbe, GitMemo, LogCache, McpSettings, McpState, MemoGit, ToolCall, ToolError, ToolReply,
};
use crate::adapters::fs::{
    FsResolver, GlobalStore, MEMORY_DIR, StoreError, events_path, points_outside, read_text,
    resolve_path,
};
use crate::adapters::git::{GitHistory, status_files};
use crate::core::context::{ContextInput, generate_context};
use crate::core::event::IssueId;
use crate::core::registry::Registry;
use crate::core::reply::{current_reply, gotchas_reply, precheck_report, projects_reply};
use crate::core::resolve::{Probe, Resolution, resolve};
use crate::core::score::{calculate_score, raw_events, score_event, score_reply};
use crate::core::search::search_reply;
use crate::core::session::session_start;
use crate::core::text::{is_python_word, python_strip};
use crate::core::time::Clock;
use crate::core::write::Ids;
use crate::wiring::hooks::analyze_files;
use crate::wiring::registry::load_registry;
use crate::wiring::types::Settings;

const CONTEXT_RECENT_DAYS: i64 = 30;

impl McpState {
    #[must_use]
    pub fn new(mut settings: McpSettings) -> Self {
        if settings.pinned.is_none() {
            settings.pinned = FsProbe {
                cwd: &settings.cwd,
                home: settings.home.as_deref(),
            }
            .discover();
        }
        if let Some(pinned) = &settings.pinned {
            settings.cwd.clone_from(pinned);
        }
        Self {
            settings,
            cache: Mutex::new(LogCache::default()),
            git: Mutex::new(GitMemo::default()),
        }
    }

    #[must_use]
    pub fn pinned(&self) -> Option<&Path> {
        self.settings.pinned.as_deref()
    }

    #[must_use]
    pub fn global_mode(&self) -> bool {
        self.settings.pinned.is_none()
    }

    pub(super) fn probe(&self) -> FsProbe<'_> {
        FsProbe {
            cwd: &self.settings.cwd,
            home: self.settings.home.as_deref(),
        }
    }

    pub(super) fn project_settings(&self, clock: &Clock) -> Settings {
        Settings {
            dejafix_home: self.settings.dejafix_home.clone(),
            redact: self.settings.redact,
            clock: clock.clone(),
        }
    }

    fn registry(&self, clock: &Clock) -> Result<Registry, ToolError> {
        Ok(load_registry(&self.project_settings(clock))?)
    }

    pub(super) fn resolution(
        &self,
        project: Option<&str>,
        clock: &Clock,
    ) -> Result<Resolution, ToolError> {
        let registry = self.registry(clock)?;
        Ok(resolve(
            &registry,
            project,
            self.settings.pinned.as_deref(),
            &[],
            &self.probe(),
        )?)
    }

    pub fn call(
        &self,
        call: ToolCall,
        project: Option<&str>,
        now: jiff::Timestamp,
        ids: &mut impl Ids,
    ) -> ToolReply {
        let clock = Clock {
            now,
            zone: self.settings.zone.clone(),
        };
        ToolReply::from(self.dispatch(call, project, &clock, ids))
    }

    fn dispatch(
        &self,
        call: ToolCall,
        project: Option<&str>,
        clock: &Clock,
        ids: &mut impl Ids,
    ) -> Result<String, ToolError> {
        match call {
            ToolCall::ListProjects => return self.list_projects(clock),
            ToolCall::GetGlobalGotchas(library) => return self.gotchas(library.as_deref()),
            ToolCall::Write(write) => return self.write(write, project, clock, ids),
            _ => {}
        }
        let target = self.resolution(project, clock)?;
        let root = target.root.as_path();
        match call {
            ToolCall::StartSession => start(root),
            ToolCall::GetInstructions => Ok(memory_text(root, "AI_INSTRUCTIONS.md")?
                .unwrap_or_else(|| "No instructions found. Run `djf init` first.".to_owned())),
            ToolCall::GetSummary => Ok(memory_text(root, "summary.md")?
                .unwrap_or_else(|| "No summary found. Run `djf init` first.".to_owned())),
            ToolCall::GetProjectMap => Ok(memory_text(root, "PROJECT_MAP.md")?
                .unwrap_or_else(|| "No project map found.".to_owned())),
            ToolCall::GetPlan => {
                Ok(memory_text(root, "plan.md")?.unwrap_or_else(|| "No plan found.".to_owned()))
            }
            ToolCall::PrecheckFile(path) => self.precheck(root, &[path], clock),
            ToolCall::PrecheckFiles(paths) => self.precheck(root, &paths, clock),
            ToolCall::GetIssue(issue) => issue_text(root, &issue),
            ToolCall::SearchEvents { query, limit, full } => {
                let cached = self.cached_log(root, clock)?;
                Ok(search_reply(&cached.log, &query, limit, full))
            }
            ToolCall::GetScore => score(root),
            ToolCall::GetContext { tokens, focus } => {
                let cached = self.cached_log(root, clock)?;
                let map = memory_text(root, "PROJECT_MAP.md")?;
                let git_files = status_files(root);
                Ok(generate_context(
                    &cached.log,
                    &ContextInput {
                        budget: tokens,
                        focus: focus.as_deref(),
                        recent_days: CONTEXT_RECENT_DAYS,
                        now: clock.now,
                        git_files: &git_files,
                        project_map: map.as_deref(),
                    },
                ))
            }
            ToolCall::CurrentProject => Ok(current_reply(&target)),
            ToolCall::ListProjects | ToolCall::GetGlobalGotchas(_) | ToolCall::Write(_) => {
                Ok(String::new())
            }
        }
    }

    fn precheck(&self, root: &Path, paths: &[String], clock: &Clock) -> Result<String, ToolError> {
        let cached = self.cached_log(root, clock)?;
        let history = GitHistory {
            root: root.to_path_buf(),
        };
        let warnings = analyze_files(
            &cached.log,
            &cached.context,
            paths,
            clock.now,
            &MemoGit {
                inner: history,
                root,
                state: OnceLock::new(),
                memo: &self.git,
                now: clock.now,
            },
        );
        Ok(precheck_report(
            paths,
            &warnings,
            &cached.context,
            &FsResolver,
        ))
    }

    fn gotchas(&self, library: Option<&str>) -> Result<String, ToolError> {
        let store = GlobalStore::new(&self.settings.dejafix_home);
        store.ensure()?;
        let gotchas = store.gotchas().ok_or_else(|| {
            ToolError::Json(format!(
                "{} holds a line that is not a JSON object",
                store.path("library_gotchas.jsonl").display()
            ))
        })?;
        Ok(gotchas_reply(&gotchas, library))
    }

    fn list_projects(&self, clock: &Clock) -> Result<String, ToolError> {
        if let Some(pinned) = self.pinned() {
            return Ok(projects_reply(&Registry::default(), Some(pinned), |_| None));
        }
        let registry = self.registry(clock)?;
        let home = self.settings.home.as_deref();
        let cwd = self.settings.cwd.as_path();
        Ok(projects_reply(&registry, None, |text| {
            resolve_path(text, home, cwd)
        }))
    }
}

pub(super) fn memory_path(root: &Path, name: &str) -> PathBuf {
    root.join(MEMORY_DIR).join(name)
}

pub(super) fn memory_text(root: &Path, name: &str) -> Result<Option<String>, ToolError> {
    let path = memory_path(root, name);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_text(&path)?))
}

fn stripped(root: &Path, name: &str) -> Result<String, ToolError> {
    Ok(memory_text(root, name)?
        .map(|text| python_strip(&text).to_owned())
        .unwrap_or_default())
}

fn start(root: &Path) -> Result<String, ToolError> {
    Ok(session_start(
        &stripped(root, "AI_INSTRUCTIONS.md")?,
        &stripped(root, "summary.md")?,
        &stripped(root, "plan.md")?,
        &stripped(root, "PROJECT_MAP.md")?,
    ))
}

fn issue_text(root: &Path, requested: &str) -> Result<String, ToolError> {
    let missing = || format!("No issue found with ID {requested}.");
    let wanted = IssueId::normalize(requested)
        .map(|issue| issue.as_str().to_owned())
        .unwrap_or_default();
    if wanted.is_empty() || !wanted.chars().all(|c| is_python_word(c) || c == '-') {
        return Ok(missing());
    }
    let directory = root.join(MEMORY_DIR).join("issues");
    let prefix = format!("{wanted}-");
    let Ok(entries) = fs::read_dir(&directory) else {
        return Ok(missing());
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".md")
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    match found.first() {
        Some(path) => Ok(read_text(path)?),
        None => Ok(missing()),
    }
}

fn score(root: &Path) -> Result<String, ToolError> {
    let path = events_path(&root.join(MEMORY_DIR));
    if points_outside(&path, root) {
        return Err(StoreError::OutsideProject { path }.into());
    }
    let text = fs::read(&path)
        .map_err(|error| io_error(&path, &error))
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|error| ToolError::Io {
                class: "UnicodeDecodeError",
                message: error.to_string(),
            })
        })?;
    let raw = raw_events(&text).map_err(|error| ToolError::Json(error.to_string()))?;
    let events: Vec<_> = raw.iter().map(score_event).collect();
    Ok(score_reply(&calculate_score(&events)))
}
