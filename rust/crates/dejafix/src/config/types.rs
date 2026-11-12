use std::path::PathBuf;

use jiff::tz::TimeZone;

#[derive(Debug, Clone)]
pub struct Config {
    pub cwd: PathBuf,
    pub dejafix_home: Option<PathBuf>,
    pub redact: bool,
    pub zone: TimeZone,
}
