use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::slug::{normalize_path, path_name, slugify, unique_id};
use super::types::{Loaded, ProjectRecord, Registry};

pub const DEFAULT_BRAIN: &str = "coding";
const SCHEMA_VERSION: u64 = 1;

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn string_tags(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn record(
    id: String,
    normalized: String,
    entry: Option<&Map<String, Value>>,
    stamp: &str,
) -> ProjectRecord {
    let get = |key: &str| entry.and_then(|entry| entry.get(key));
    ProjectRecord {
        id,
        path: PathBuf::from(normalized),
        alias: non_empty_string(get("alias")),
        default_brain: non_empty_string(get("default_brain"))
            .unwrap_or_else(|| DEFAULT_BRAIN.to_owned()),
        tags: string_tags(get("tags")),
        created_at: non_empty_string(get("created_at")).unwrap_or_else(|| stamp.to_owned()),
        updated_at: non_empty_string(get("updated_at")).unwrap_or_else(|| stamp.to_owned()),
    }
}

fn legacy(payload: &Map<String, Value>) -> Registry {
    let projects = match payload.get("projects") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|item| {
                let path = item.get("path")?.as_str()?;
                let id = item.get("id")?.as_str()?;
                Some(record(id.to_owned(), normalize_path(path), Some(item), ""))
            })
            .collect(),
        _ => Vec::new(),
    };
    Registry {
        active_project: non_empty_string(payload.get("active_project")),
        projects,
    }
}

fn listed_paths(payload: Option<&Value>) -> Vec<String> {
    match payload {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(text)) => text.chars().map(String::from).collect(),
        _ => Vec::new(),
    }
}

#[must_use]
pub fn load(
    payload: Option<&Value>,
    meta: Option<&Value>,
    now: &str,
    is_absolute: impl Fn(&str) -> bool,
) -> Loaded {
    if let Some(Value::Object(payload)) = payload {
        return Loaded::Legacy(legacy(payload));
    }
    let meta = meta.and_then(Value::as_object);
    let by_path = meta
        .and_then(|meta| meta.get("by_path"))
        .and_then(Value::as_object);
    let mut taken: HashSet<String> = HashSet::new();
    let mut dirty = false;
    let mut projects = Vec::new();
    for raw in listed_paths(payload)
        .into_iter()
        .filter(|raw| is_absolute(raw))
    {
        let entry = by_path
            .and_then(|by_path| by_path.get(&raw))
            .and_then(Value::as_object);
        let normalized = normalize_path(&raw);
        let id = match entry
            .and_then(|entry| entry.get("id"))
            .and_then(Value::as_str)
        {
            Some(id) if !taken.contains(id) => id.to_owned(),
            _ => {
                dirty = true;
                unique_id(&slugify(path_name(&normalized)), |id| taken.contains(id))
            }
        };
        taken.insert(id.clone());
        projects.push(record(id, normalized, entry, now));
    }
    let registry = Registry {
        active_project: meta
            .and_then(|meta| meta.get("active_project"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        projects,
    };
    if dirty {
        Loaded::Dirty(registry)
    } else {
        Loaded::Clean(registry)
    }
}

impl Loaded {
    #[must_use]
    pub fn registry(&self) -> &Registry {
        match self {
            Self::Clean(registry) | Self::Dirty(registry) | Self::Legacy(registry) => registry,
        }
    }

    #[must_use]
    pub fn into_registry(self) -> Registry {
        match self {
            Self::Clean(registry) | Self::Dirty(registry) | Self::Legacy(registry) => registry,
        }
    }
}

fn path_text(record: &ProjectRecord) -> String {
    record.path.to_string_lossy().into_owned()
}

#[must_use]
pub fn list_payload(registry: &Registry) -> Value {
    Value::Array(
        registry
            .projects
            .iter()
            .map(|record| Value::String(path_text(record)))
            .collect(),
    )
}

#[must_use]
pub fn meta_payload(registry: &Registry) -> Value {
    let mut by_path = Map::new();
    for record in &registry.projects {
        by_path.insert(
            path_text(record),
            json!({
                "id": record.id,
                "alias": record.alias,
                "default_brain": record.default_brain,
                "tags": record.tags,
                "created_at": record.created_at,
                "updated_at": record.updated_at,
            }),
        );
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "active_project": registry.active_project,
        "by_path": by_path,
    })
}
