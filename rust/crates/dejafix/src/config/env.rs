use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use dejafix_core::ClientPaths;

use super::types::{Config, HookConfig, McpConfig};

const NO_REDACT_VALUES: [&str; 3] = ["1", "true", "yes"];
const LEGACY_PREFIXES: [&str; 2] = ["PROJECTMEM_", "PJM_"];

fn legacy_env() -> Vec<String> {
    let mut names: Vec<String> = env::vars_os()
        .map(|(name, _)| {
            let name = name.to_string_lossy().into_owned();
            if cfg!(windows) {
                name.to_uppercase()
            } else {
                name
            }
        })
        .filter(|name| {
            LEGACY_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .collect();
    names.sort();
    names
}

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
            path_var: env::var_os("PATH"),
            exe_dir: env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf)),
            legacy_env: legacy_env(),
        })
    }
}

impl HookConfig {
    pub fn from_env() -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_default(),
            temp: env::temp_dir(),
            log: non_empty("DJF_EDIT_HOOK_LOG").map(PathBuf::from),
        }
    }

    pub fn zone() -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::system()
    }
}

fn expand_user(path: PathBuf, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return path;
    };
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path,
    }
}

impl McpConfig {
    pub fn from_env(arguments: impl IntoIterator<Item = OsString>) -> std::io::Result<Self> {
        let config = Config::from_env()?;
        let mut arguments = arguments.into_iter().skip(1);
        let flagged = arguments
            .by_ref()
            .find(|argument| argument == "--root")
            .and_then(|_| arguments.next());
        let root = flagged.or_else(|| non_empty("DEJAFIX_ROOT")).map(|root| {
            let expanded = expand_user(PathBuf::from(root), config.home.as_deref());
            let absolute = config.cwd.join(expanded);
            dejafix_core::canonical_path(&absolute).unwrap_or(absolute)
        });
        Ok(Self {
            root,
            cwd: config.cwd,
            home: config.home,
            dejafix_home: config.dejafix_home,
            redact: config.redact,
            zone: config.zone,
        })
    }
}
