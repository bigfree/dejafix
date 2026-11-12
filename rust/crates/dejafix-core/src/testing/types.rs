use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("cannot write fixture file {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot write a fixture event to {path}")]
    Event {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("fixture timestamp out of range")]
    Time(#[from] jiff::Error),
}

pub(crate) struct PythonJsonFormatter;

pub(crate) struct SplitMix64 {
    pub(crate) state: u64,
}

pub(crate) struct FixtureState {
    pub(crate) rng: SplitMix64,
    pub(crate) clock: i64,
    pub(crate) next_issue: u32,
    pub(crate) open_issues: Vec<String>,
    pub(crate) decisions: Vec<String>,
}
