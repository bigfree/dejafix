use std::path::PathBuf;

use crate::adapters::fs::{MemoryDir, StoreError};
use crate::core::path::PathContext;
use crate::core::time::Clock;
use crate::core::write::{ValidationError, WriteOutcome};

#[derive(Debug, Clone)]
pub struct Settings {
    pub dejafix_home: PathBuf,
    pub redact: bool,
    pub clock: Clock,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub memory: MemoryDir,
    pub settings: Settings,
    pub(super) context: PathContext,
}

#[derive(Debug)]
pub struct Recorded {
    pub outcome: WriteOutcome,
    pub notices: Vec<String>,
    pub incomplete: Option<WiringError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOutput {
    Matches(Vec<String>),
    Nothing(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum WiringError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Store(#[from] StoreError),
}
