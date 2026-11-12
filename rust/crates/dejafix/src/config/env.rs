use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;

use dejafix_core::ClientPaths;

use super::types::Config;

const NO_REDACT_VALUES: [&str; 3] = ["1", "true", "yes"];

fn non_empty(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

impl Config {
    pub fn client_paths(&self) -> Option<ClientPaths> {
        Some(ClientPaths {
            home: self.home.clone()?,
            xdg_config_home: self.xdg_config_home.clone(),
            appdata: self.appdata.clone(),
            local_appdata: self.local_appdata.clone(),
        })
    }

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
            home,
            dejafix_home,
            xdg_config_home: non_empty("XDG_CONFIG_HOME").map(PathBuf::from),
            appdata: non_empty("APPDATA").map(PathBuf::from),
            local_appdata: non_empty("LOCALAPPDATA").map(PathBuf::from),
            redact,
            color: std::io::stdout().is_terminal(),
            zone: jiff::tz::TimeZone::system(),
        })
    }
}
