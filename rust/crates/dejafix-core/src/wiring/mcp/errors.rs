use std::io;
use std::path::Path;

use super::types::{ToolError, ToolReply};
use crate::adapters::fs::StoreError;
use crate::core::resolve::ResolveError;
use crate::core::write::ValidationError;
use crate::wiring::python::python_io_error;
use crate::wiring::types::WiringError;

pub(super) fn io_error(path: &Path, error: &io::Error) -> ToolError {
    let (class, message) = python_io_error(error, path);
    ToolError::Io { class, message }
}

impl From<ResolveError> for ToolError {
    fn from(error: ResolveError) -> Self {
        Self::Resolution(error.to_string())
    }
}

impl From<StoreError> for ToolError {
    fn from(error: StoreError) -> Self {
        match &error {
            StoreError::Read { path, source }
            | StoreError::Write { path, source }
            | StoreError::Lock { path, source }
            | StoreError::Root { path, source } => io_error(path, source),
            StoreError::NotFound { .. } => Self::Dejafix(error.to_string()),
        }
    }
}

impl From<WiringError> for ToolError {
    fn from(error: WiringError) -> Self {
        match error {
            WiringError::Validation(ValidationError::EventRef(error)) => {
                Self::Value(error.to_string())
            }
            WiringError::Validation(error) => Self::Dejafix(error.to_string()),
            WiringError::Store(error) => error.into(),
            WiringError::Registry(error) => Self::Registry(error.to_string()),
        }
    }
}

impl From<Result<String, ToolError>> for ToolReply {
    fn from(result: Result<String, ToolError>) -> Self {
        match result {
            Ok(text) => Self { text, error: false },
            Err(error) => Self {
                text: format!("dejafix tool error: {error}"),
                error: true,
            },
        }
    }
}
