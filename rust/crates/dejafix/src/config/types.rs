use std::ffi::OsString;
use std::path::PathBuf;

use jiff::tz::TimeZone;

#[derive(Debug, Clone)]
pub struct Config {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub dejafix_home: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub local_appdata: Option<PathBuf>,
    pub redact: bool,
    pub color: bool,
    pub zone: TimeZone,
    pub path_var: Option<OsString>,
    pub exe_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct HookConfig {
    pub cwd: PathBuf,
    pub temp: PathBuf,
    pub log: Option<PathBuf>,
}
