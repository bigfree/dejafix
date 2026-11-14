use serde_json::{Map, Value};

use super::types::{Entry, GotchaV2, Libraries, StoredGotcha};
use crate::core::json::write_python_json;

const FORMAT_VERSION: u64 = 2;

const KNOWN_FIELDS: [&str; 9] = [
    "v",
    "id",
    "library",
    "keywords",
    "versions",
    "gotcha",
    "source_project",
    "discovered",
    "reviewed",
];

fn text(entry: &Entry, key: &str) -> Option<String> {
    entry.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn optional(entry: &Entry, key: &str) -> Result<Option<String>, ()> {
    match entry.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(()),
    }
}

fn version_two(entry: &Entry) -> Option<GotchaV2> {
    if entry.get("v").and_then(Value::as_u64) != Some(FORMAT_VERSION) {
        return None;
    }
    let keywords = entry
        .get("keywords")?
        .as_array()?
        .iter()
        .map(|keyword| keyword.as_str().map(str::to_owned))
        .collect::<Option<Vec<String>>>()?;
    Some(GotchaV2 {
        id: text(entry, "id")?,
        library: text(entry, "library")?,
        keywords,
        versions: optional(entry, "versions").ok()?,
        gotcha: text(entry, "gotcha")?,
        source_project: optional(entry, "source_project").ok()?,
        discovered: optional(entry, "discovered").ok()?,
        reviewed: optional(entry, "reviewed").ok()?,
        extra: entry
            .iter()
            .filter(|(key, _)| !KNOWN_FIELDS.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    })
}

impl StoredGotcha {
    #[must_use]
    pub fn from_entry(entry: Entry) -> Self {
        version_two(&entry).map_or(Self::V1(entry), Self::V2)
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::V2(record) => &record.id,
            Self::V1(entry) => entry.get("id").and_then(Value::as_str).unwrap_or(""),
        }
    }

    #[must_use]
    pub fn library(&self) -> &str {
        match self {
            Self::V2(record) => &record.library,
            Self::V1(entry) => entry.get("library").and_then(Value::as_str).unwrap_or(""),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::V2(record) => &record.gotcha,
            Self::V1(entry) => entry.get("gotcha").and_then(Value::as_str).unwrap_or(""),
        }
    }

    #[must_use]
    pub fn is_upgraded(&self) -> bool {
        matches!(self, Self::V2(_))
    }
}

#[must_use]
pub fn is_upgraded(entry: &Entry) -> bool {
    version_two(entry).is_some()
}

#[must_use]
pub fn parse_store(entries: Vec<Entry>) -> Vec<StoredGotcha> {
    entries.into_iter().map(StoredGotcha::from_entry).collect()
}

fn optional_value(value: Option<&String>) -> Value {
    value.map_or(Value::Null, |text| Value::String(text.clone()))
}

#[must_use]
fn record_value(record: &GotchaV2) -> Value {
    let mut map = Map::new();
    map.insert("v".to_owned(), Value::from(FORMAT_VERSION));
    map.insert("id".to_owned(), Value::String(record.id.clone()));
    map.insert("library".to_owned(), Value::String(record.library.clone()));
    map.insert(
        "keywords".to_owned(),
        Value::Array(record.keywords.iter().cloned().map(Value::String).collect()),
    );
    map.insert(
        "versions".to_owned(),
        optional_value(record.versions.as_ref()),
    );
    map.insert("gotcha".to_owned(), Value::String(record.gotcha.clone()));
    for (key, value) in [
        ("source_project", &record.source_project),
        ("discovered", &record.discovered),
        ("reviewed", &record.reviewed),
    ] {
        if let Some(value) = value {
            map.insert(key.to_owned(), Value::String(value.clone()));
        }
    }
    for (key, value) in &record.extra {
        map.entry(key.clone()).or_insert_with(|| value.clone());
    }
    Value::Object(map)
}

#[must_use]
pub fn line(record: &GotchaV2) -> String {
    let mut out = Vec::new();
    if write_python_json(&mut out, &record_value(record)).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default()
}

impl Libraries {
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        let mut map = std::collections::BTreeMap::new();
        for (library, aliases) in value.as_object()? {
            let aliases = aliases
                .as_array()?
                .iter()
                .map(|alias| alias.as_str().map(str::to_lowercase))
                .collect::<Option<Vec<String>>>()?;
            map.insert(library.to_lowercase(), aliases);
        }
        Some(Self(map))
    }

    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|(library, aliases)| {
                    (
                        library.clone(),
                        Value::Array(aliases.iter().cloned().map(Value::String).collect()),
                    )
                })
                .collect(),
        )
    }

    #[must_use]
    pub fn canonical(&self, name: &str) -> String {
        let lowered = name.trim().to_lowercase();
        if self.0.contains_key(&lowered) {
            return lowered;
        }
        self.0
            .iter()
            .find(|(_, aliases)| aliases.contains(&lowered))
            .map_or(lowered, |(library, _)| library.clone())
    }

    #[must_use]
    pub fn aliases(&self, library: &str) -> &[String] {
        self.0.get(library).map_or(&[], Vec::as_slice)
    }

    pub fn add_library(&mut self, library: &str) -> bool {
        if self.0.contains_key(library) {
            return false;
        }
        self.0.insert(library.to_owned(), Vec::new());
        true
    }

    pub fn add_alias(&mut self, library: &str, alias: &str) -> bool {
        let alias = alias.trim().to_lowercase();
        let aliases = self.0.entry(library.to_owned()).or_default();
        if alias.is_empty() || alias == library || aliases.contains(&alias) {
            return false;
        }
        aliases.push(alias);
        aliases.sort();
        true
    }
}
