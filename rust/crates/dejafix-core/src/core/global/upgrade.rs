use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::{Map, Value};

use super::checks::normalize_input;
use super::record::{extra_fields, line, stored_line};
use super::types::{
    CheckFailure, CheckReport, Draft, DraftError, Entry, GotchaInput, GotchaV2, Libraries,
    StoredGotcha, UpgradeOutcome, Verdict,
};

const INSTRUCTIONS: &str = "For every record below: translate the original text into English and write it as one general lesson of 30 to 600 characters in \"gotcha\"; remove project details (paths, ticket numbers, internal hosts, project names); give 2 to 8 English keywords of at most 4 words in \"keywords\"; correct \"library\" to the canonical package name in lower case; fill \"versions\" when the lesson applies to a version range; set \"drop\" to true for anything that is not a general lesson. Leave \"original\" and \"checks\" as they are, they are ignored when the file is applied.";
const UNKNOWN_ID: &str = "no record has this id";
const ALREADY_UPGRADED: &str = "the record is already version 2";
const LEGACY_VERSIONS: &str = "version_range";

fn text_of(entry: &Entry, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn non_empty_text(entry: &Entry, key: &str) -> Option<String> {
    Some(text_of(entry, key)).filter(|text| !text.is_empty())
}

fn value_or_null(entry: &Entry, key: &str) -> Value {
    entry.get(key).cloned().unwrap_or(Value::Null)
}

fn json_line(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[must_use]
pub fn legacy_input(entry: &Entry) -> GotchaInput {
    GotchaInput {
        library: text_of(entry, "library"),
        keywords: Vec::new(),
        gotcha: text_of(entry, "gotcha"),
        versions: None,
    }
}

#[must_use]
pub fn export_notes(report: &CheckReport) -> Vec<String> {
    report
        .failures
        .iter()
        .filter(|failure| {
            !matches!(
                failure,
                CheckFailure::KeywordCount(_) | CheckFailure::EmptyKeyword
            )
        })
        .map(ToString::to_string)
        .chain(report.warnings.iter().map(ToString::to_string))
        .collect()
}

fn draft_line(entry: &Entry, checks: Vec<String>) -> String {
    let mut map = Map::new();
    map.insert("id".to_owned(), value_or_null(entry, "id"));
    map.insert("library".to_owned(), value_or_null(entry, "library"));
    map.insert(
        "source_project".to_owned(),
        value_or_null(entry, "source_project"),
    );
    map.insert("discovered".to_owned(), value_or_null(entry, "discovered"));
    map.insert("original".to_owned(), value_or_null(entry, "gotcha"));
    map.insert("gotcha".to_owned(), Value::String(String::new()));
    map.insert("keywords".to_owned(), Value::Array(Vec::new()));
    map.insert(
        "versions".to_owned(),
        non_empty_text(entry, LEGACY_VERSIONS).map_or(Value::Null, Value::String),
    );
    map.insert("drop".to_owned(), Value::Bool(false));
    map.insert(
        "checks".to_owned(),
        Value::Array(checks.into_iter().map(Value::String).collect()),
    );
    json_line(&Value::Object(map))
}

pub fn drafts(records: &[StoredGotcha], checks: impl Fn(&Entry) -> Vec<String>) -> Vec<String> {
    let header = json_line(&Value::Object(Map::from_iter([(
        "instructions".to_owned(),
        Value::String(INSTRUCTIONS.to_owned()),
    )])));
    std::iter::once(header)
        .chain(records.iter().filter_map(|record| match record {
            StoredGotcha::V1(entry) => Some(draft_line(entry, checks(entry))),
            StoredGotcha::V2(_) => None,
        }))
        .collect()
}

fn optional_text(
    object: &Entry,
    key: &'static str,
    line: usize,
) -> Result<Option<String>, DraftError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(DraftError::WrongType { line, field: key }),
    }
}

fn keywords(object: &Entry, line: usize) -> Result<Vec<String>, DraftError> {
    let wrong = DraftError::WrongType {
        line,
        field: "keywords",
    };
    match object.get("keywords") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| wrong.clone())
            })
            .collect(),
        Some(_) => Err(wrong),
    }
}

fn draft_of(object: &Entry, line: usize) -> Result<Draft, DraftError> {
    let id = optional_text(object, "id", line)?
        .filter(|id| !id.is_empty())
        .ok_or(DraftError::MissingId { line })?;
    let drop = match object.get("drop") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(drop)) => *drop,
        Some(_) => {
            return Err(DraftError::WrongType {
                line,
                field: "drop",
            });
        }
    };
    Ok(Draft {
        id,
        input: GotchaInput {
            library: optional_text(object, "library", line)?.unwrap_or_default(),
            keywords: keywords(object, line)?,
            gotcha: optional_text(object, "gotcha", line)?.unwrap_or_default(),
            versions: optional_text(object, "versions", line)?,
        },
        drop,
    })
}

fn is_header(object: &Entry) -> bool {
    object.contains_key("instructions") && !object.contains_key("id")
}

pub fn parse_drafts(text: &str) -> Result<Vec<Draft>, Vec<DraftError>> {
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let object = match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(object)) => object,
            Ok(_) => {
                errors.push(DraftError::NotObject { line });
                continue;
            }
            Err(error) => {
                errors.push(DraftError::Json {
                    line,
                    message: error.to_string(),
                });
                continue;
            }
        };
        if is_header(&object) {
            continue;
        }
        match draft_of(&object, line) {
            Ok(draft) if seen.insert(draft.id.clone()) => parsed.push(draft),
            Ok(draft) => errors.push(DraftError::DuplicateId { line, id: draft.id }),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(parsed)
    } else {
        Err(errors)
    }
}

#[must_use]
pub fn touches_version_one(records: &[StoredGotcha], drafts: &[Draft]) -> bool {
    let ids: HashSet<&str> = drafts.iter().map(|draft| draft.id.as_str()).collect();
    records
        .iter()
        .any(|record| !record.is_upgraded() && ids.contains(record.id()))
}

fn upgraded(entry: &Entry, input: GotchaInput, today: &str) -> GotchaV2 {
    let extra = extra_fields(entry)
        .into_iter()
        .filter(|(key, _)| key != LEGACY_VERSIONS)
        .collect();
    GotchaV2 {
        id: text_of(entry, "id"),
        library: input.library,
        keywords: input.keywords,
        versions: input.versions,
        gotcha: input.gotcha,
        source_project: non_empty_text(entry, "source_project"),
        discovered: non_empty_text(entry, "discovered"),
        reviewed: Some(today.to_owned()),
        extra,
    }
}

fn verdicts<'d>(
    records: &[StoredGotcha],
    drafts: &'d [Draft],
    today: &str,
    libraries: &Libraries,
    check: impl Fn(&str, &GotchaInput) -> CheckReport,
    outcome: &mut UpgradeOutcome,
) -> HashMap<&'d str, Verdict> {
    let by_id: HashMap<&str, &StoredGotcha> =
        records.iter().map(|record| (record.id(), record)).collect();
    let mut verdicts = HashMap::new();
    for draft in drafts {
        let reason = |text: &str| (draft.id.clone(), vec![text.to_owned()]);
        match by_id.get(draft.id.as_str()) {
            None => outcome.kept.push(reason(UNKNOWN_ID)),
            Some(StoredGotcha::V2(_)) => outcome.kept.push(reason(ALREADY_UPGRADED)),
            Some(StoredGotcha::V1(_)) if draft.drop => {
                verdicts.insert(draft.id.as_str(), Verdict::Drop);
            }
            Some(StoredGotcha::V1(entry)) => {
                let input = normalize_input(draft.input.clone(), libraries);
                let report = check(&draft.id, &input);
                if report.failures.is_empty() {
                    verdicts.insert(
                        draft.id.as_str(),
                        Verdict::Replace(Box::new(upgraded(entry, input, today))),
                    );
                } else {
                    outcome.kept.push((
                        draft.id.clone(),
                        report.failures.iter().map(ToString::to_string).collect(),
                    ));
                }
            }
        }
    }
    verdicts
}

#[must_use]
pub fn apply(
    records: &[StoredGotcha],
    drafts: &[Draft],
    today: &str,
    libraries: &Libraries,
    check: impl Fn(&str, &GotchaInput) -> CheckReport,
) -> UpgradeOutcome {
    let mut outcome = UpgradeOutcome::default();
    let verdicts = verdicts(records, drafts, today, libraries, check, &mut outcome);
    let mut new_libraries = BTreeSet::new();
    for record in records {
        let verdict = match record {
            StoredGotcha::V1(_) => verdicts.get(record.id()),
            StoredGotcha::V2(_) => None,
        };
        match (record, verdict) {
            (_, Some(Verdict::Drop)) => outcome.dropped += 1,
            (_, Some(Verdict::Replace(replacement))) => {
                new_libraries.insert(replacement.library.clone());
                outcome.lines.push(line(replacement));
                outcome.replaced += 1;
            }
            (StoredGotcha::V1(entry), None) => outcome
                .lines
                .push(stored_line(&StoredGotcha::V1(entry.clone()))),
            (StoredGotcha::V2(kept), None) => outcome.lines.push(line(kept)),
        }
    }
    outcome.libraries = new_libraries.into_iter().collect();
    outcome
}
