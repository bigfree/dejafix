use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot resolve the project root {path}")]
    Root {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FsResolver;
