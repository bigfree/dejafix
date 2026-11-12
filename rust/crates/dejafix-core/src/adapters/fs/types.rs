use std::fs::File;
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
    #[error("cannot write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot lock {path}")]
    Lock {
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
    #[error(
        "No .dejafix directory found in {} or any parent. If running over MCP, set the project root via the `cwd` field in your MCP client config or via the DEJAFIX_ROOT environment variable. Otherwise run `dejafix init` to create one.",
        cwd.display()
    )]
    NotFound { cwd: PathBuf },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FsResolver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDir {
    pub root: PathBuf,
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct LockedLog {
    pub(super) file: File,
    pub(super) path: PathBuf,
    pub(super) unterminated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalStore {
    pub dir: PathBuf,
}
