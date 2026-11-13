use std::sync::Arc;

use dejafix_core::McpState;
use rmcp::model::Tool;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DejafixServer {
    pub(super) state: Arc<McpState>,
    pub(super) tools: Arc<[Tool]>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectArgs {
    #[serde(default)]
    pub(super) project: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FileArgs {
    pub(super) file_path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct FilesArgs {
    pub(super) file_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueArgs {
    pub(super) issue_id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchArgs {
    pub(super) query: String,
    #[serde(default = "default_limit")]
    pub(super) limit: i64,
    #[serde(default)]
    pub(super) full: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContextArgs {
    #[serde(default = "default_tokens")]
    pub(super) tokens: i64,
    #[serde(default)]
    pub(super) focus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GotchaArgs {
    #[serde(default)]
    pub(super) library: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WriteArgs {
    pub(super) summary: String,
    #[serde(default)]
    pub(super) location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AttemptArgs {
    #[serde(default = "default_outcome")]
    pub(super) outcome: String,
    #[serde(default)]
    pub(super) issue_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FixArgs {
    #[serde(default)]
    pub(super) issue_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DecisionArgs {
    #[serde(default)]
    pub(super) supersedes: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArgumentError {
    #[error("{0}")]
    Shape(#[from] serde_json::Error),
    #[error("{field}: input should be {expected}")]
    Range {
        field: &'static str,
        expected: &'static str,
    },
    #[error("unknown tool")]
    Unknown,
}

fn default_limit() -> i64 {
    10
}

fn default_tokens() -> i64 {
    2000
}

fn default_outcome() -> String {
    "failed".to_owned()
}
