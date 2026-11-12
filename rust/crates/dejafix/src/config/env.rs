use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use super::types::Config;

const NO_REDACT_VALUES: [&str; 3] = ["1", "true", "yes"];

fn non_empty(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

impl Config {
    pub fn from_env() -> std::io::Result<Self> {
        let cwd = env::current_dir()?;
        let home = env::home_dir();
        let dejafix_home = non_empty("DEJAFIX_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".dejafix")));
        let redact = env::var("DEJAFIX_NO_REDACT").map_or(true, |value| {
            !NO_REDACT_VALUES.contains(&dejafix_core::python_strip(&value))
        });
        Ok(Self {
            cwd,
            dejafix_home,
            redact,
            zone: jiff::tz::TimeZone::system(),
        })
    }
}
