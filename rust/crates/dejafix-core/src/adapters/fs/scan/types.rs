use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientPaths {
    pub home: PathBuf,
    pub xdg_config_home: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub local_appdata: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigState {
    Pinned,
    Clean,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub client: &'static str,
    pub path: PathBuf,
    pub state: ConfigState,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identity {
    pub(super) device: u64,
    pub(super) inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identity(pub(super) PathBuf);
