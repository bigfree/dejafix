use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GitHistory {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitFailure {
    Exit,
    Unavailable,
}
