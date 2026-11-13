use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreEvent<'e> {
    pub kind: &'e str,
    pub outcome: Option<&'e str>,
    pub auto: bool,
    pub files: Vec<Cow<'e, str>>,
    pub at: Option<jiff::Timestamp>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub score: u32,
    pub grade: &'static str,
    pub failed: usize,
    pub worked: usize,
    pub decisions: usize,
    pub fixes: usize,
    pub notes: usize,
    pub files_with_gotchas: usize,
    pub high_churn_files: usize,
    pub auto: usize,
    pub manual: usize,
    pub total: usize,
    pub hours: f64,
    pub tokens: u64,
    pub usd: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScoreError {
    #[error("line {line}: {message}")]
    Json { line: usize, message: String },
}
