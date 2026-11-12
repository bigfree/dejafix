use serde_json::{Map, Value, json};

use super::types::{Reverted, SeenConfig};
use crate::core::json::python_str;

const NEVER_SEEN: &str = "a previous run";

fn was_clean(previous: Option<&Value>) -> Option<&Map<String, Value>> {
    match previous {
        Some(Value::Object(entry))
            if !entry.is_empty() && entry.get("pinned") == Some(&Value::Bool(false)) =>
        {
            Some(entry)
        }
        _ => None,
    }
}

pub fn track_pins(
    state: &mut Map<String, Value>,
    configs: &[SeenConfig<'_>],
    now: &str,
) -> Vec<Reverted> {
    let mut pinned: Vec<(&str, &str)> = Vec::new();
    for config in configs.iter().filter(|config| config.pinned) {
        match pinned.iter_mut().find(|(key, _)| *key == config.key) {
            Some(entry) => entry.1 = config.client,
            None => pinned.push((&config.key, config.client)),
        }
    }
    let mut reverted = Vec::new();
    for (key, client) in &pinned {
        if let Some(entry) = was_clean(state.get(*key)) {
            reverted.push(Reverted {
                client: (*client).to_owned(),
                clean_since: entry
                    .get("seen")
                    .map_or_else(|| NEVER_SEEN.to_owned(), python_str),
            });
        }
        state.insert(
            (*key).to_owned(),
            json!({"pinned": true, "seen": now, "client": client}),
        );
    }
    for config in configs.iter().filter(|config| !config.pinned) {
        if pinned.iter().any(|(key, _)| *key == config.key) {
            continue;
        }
        state.insert(
            config.key.clone(),
            json!({"pinned": false, "seen": now, "client": config.client}),
        );
    }
    reverted
}
