use serde_json::{Number, Value};

use super::types::UpdateStatus;
use crate::adapters::fs::RegistryFiles;
use crate::core::doctor::is_newer;
use crate::core::global::is_truthy;
use crate::wiring::types::{Settings, WiringError};

const UPDATE_CHECK: &str = "update_check";
const CHECKED_AT: &str = "update_checked_at";
const LATEST: &str = "update_latest";
const CHECK_INTERVAL_SECONDS: f64 = 86_400.0;

fn epoch_seconds(settings: &Settings) -> f64 {
    let now = settings.clock.now;
    format!("{}.{:06}", now.as_second(), now.subsec_microsecond().abs())
        .parse()
        .unwrap_or(0.0)
}

fn seconds(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        Some(Value::Bool(true)) => 1.0,
        Some(Value::String(text)) => text.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

pub fn set_auto_check(settings: &Settings, enabled: bool) -> Result<(), WiringError> {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let mut meta = files.read_meta();
    meta.insert(UPDATE_CHECK.to_owned(), Value::Bool(enabled));
    files.save_meta(meta)?;
    Ok(())
}

pub fn update_status(
    settings: &Settings,
    online: bool,
    version: &str,
    fetch: impl FnOnce() -> Option<String>,
) -> UpdateStatus {
    let files = RegistryFiles::new(&settings.dejafix_home);
    let mut meta = files.read_meta();
    let enabled = meta.get(UPDATE_CHECK).is_some_and(is_truthy);
    if !(online || enabled) {
        return UpdateStatus::Offline;
    }
    let mut newest = meta
        .get(LATEST)
        .and_then(Value::as_str)
        .filter(|latest| !latest.is_empty())
        .map(str::to_owned);
    let now = epoch_seconds(settings);
    if (online || now - seconds(meta.get(CHECKED_AT)) > CHECK_INTERVAL_SECONDS)
        && let Some(fetched) = fetch().filter(|fetched| !fetched.is_empty())
    {
        if let Some(stamp) = Number::from_f64(now) {
            meta.insert(CHECKED_AT.to_owned(), Value::Number(stamp));
        }
        meta.insert(LATEST.to_owned(), Value::String(fetched.clone()));
        let _ = files.save_meta(meta);
        newest = Some(fetched);
    }
    match newest {
        None => UpdateStatus::Unreachable,
        Some(newest) if is_newer(&newest, version) => UpdateStatus::Newer(newest),
        Some(_) => UpdateStatus::Latest,
    }
}
