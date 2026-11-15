use std::fmt::Write as _;

use serde_json::Value;

use super::types::{GlobalDoc, GlobalIndex, Libraries, StoredGotcha};
use crate::core::rank::{Document, FieldSpec, Index, RankParams};
use crate::core::text::collapse_whitespace;

pub const GLOBAL_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "library",
        weight: 3.0,
    },
    FieldSpec {
        name: "keywords",
        weight: 2.0,
    },
    FieldSpec {
        name: "gotcha",
        weight: 1.0,
    },
];

impl Document for GlobalDoc<'_> {
    fn id(&self) -> &str {
        self.record.id()
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        match field {
            0 => {
                let stored = self.record.library();
                if stored != self.canonical && !self.aliases.iter().any(|alias| alias == stored) {
                    visit(stored);
                }
                visit(self.canonical);
                self.aliases.iter().for_each(|alias| visit(alias));
            }
            1 => {
                if let StoredGotcha::V2(record) = self.record {
                    record.keywords.iter().for_each(|keyword| visit(keyword));
                }
            }
            _ => visit(self.record.text()),
        }
    }
}

pub(super) fn library_docs<'r>(
    records: impl Iterator<Item = &'r StoredGotcha>,
    canonical: &'r str,
    libraries: &'r Libraries,
) -> Vec<GlobalDoc<'r>> {
    let aliases = libraries.aliases(canonical);
    records
        .map(|record| GlobalDoc {
            record,
            canonical,
            aliases,
        })
        .collect()
}

pub const DEFAULT_BUDGET: u32 = 2_000;
const MAX_RESULTS: usize = 8;
const CHARS_PER_TOKEN: usize = 4;

impl GlobalIndex {
    #[must_use]
    pub fn build(records: Vec<StoredGotcha>, libraries: Libraries) -> Self {
        let canonical: Vec<Box<str>> = records
            .iter()
            .map(|record| libraries.canonical(record.library()).into_boxed_str())
            .collect();
        let docs: Vec<GlobalDoc<'_>> = records
            .iter()
            .zip(&canonical)
            .map(|(record, library)| GlobalDoc {
                record,
                canonical: library,
                aliases: libraries.aliases(library),
            })
            .collect();
        let index = Index::build(GLOBAL_FIELDS, &docs);
        Self {
            index,
            records,
            canonical,
            libraries,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn matches(&self, query: &str, library: Option<&str>) -> Vec<&StoredGotcha> {
        let wanted = library
            .map(str::trim)
            .filter(|library| !library.is_empty())
            .map(|library| self.libraries.canonical(library));
        let keep = |doc: usize| {
            wanted.as_deref().is_none_or(|wanted| {
                self.canonical
                    .get(doc)
                    .is_some_and(|library| **library == *wanted)
            })
        };
        if query.trim().is_empty() {
            let mut all: Vec<(&str, usize)> = self
                .records
                .iter()
                .enumerate()
                .filter(|(doc, _)| keep(*doc))
                .map(|(doc, record)| (record.id(), doc))
                .collect();
            all.sort_unstable();
            return all
                .into_iter()
                .filter_map(|(_, doc)| self.records.get(doc))
                .collect();
        }
        let terms = self.index.query_terms(query);
        let params = RankParams {
            limit: MAX_RESULTS,
            ..RankParams::DEFAULT
        };
        self.index
            .rank_where(&terms, &params, &|doc| usize::try_from(doc).is_ok_and(keep))
            .iter()
            .filter_map(|scored| self.records.get(usize::try_from(scored.doc).ok()?))
            .collect()
    }

    fn library_counts(&self) -> String {
        let mut libraries: Vec<(&str, usize)> = self
            .canonical
            .iter()
            .enumerate()
            .map(|(doc, library)| (&**library, doc))
            .collect();
        libraries.sort_unstable();
        libraries
            .chunk_by(|left, right| left.0 == right.0)
            .filter_map(|run| {
                run.first()
                    .map(|(library, _)| format!("{library} ({})", run.len()))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn entry_text(record: &StoredGotcha) -> String {
    let mut head = format!("- {} · {}", record.id(), record.library());
    let mut provenance: Vec<String> = Vec::new();
    match record {
        StoredGotcha::V2(record) => {
            if let Some(versions) = &record.versions {
                let _ = write!(head, " {versions}");
            }
            provenance.extend(
                record
                    .source_project
                    .iter()
                    .map(|from| format!("from {from}")),
            );
            provenance.extend(record.reviewed.iter().map(|day| format!("reviewed {day}")));
        }
        StoredGotcha::V1(entry) => {
            provenance.extend(
                entry
                    .get("source_project")
                    .and_then(Value::as_str)
                    .filter(|from| !from.is_empty())
                    .map(|from| format!("from {from}")),
            );
            provenance.push("not upgraded".to_owned());
        }
    }
    if !provenance.is_empty() {
        let _ = write!(head, " · {}", provenance.join(", "));
    }
    format!("{head}\n  {}\n", collapse_whitespace(record.text()))
}

#[must_use]
pub fn global_reply(
    store: &GlobalIndex,
    query: &str,
    library: Option<&str>,
    budget_tokens: u32,
) -> String {
    if store.is_empty() {
        return "Global memory is empty. Add a lesson with add_global_gotcha.".to_owned();
    }
    let found = store.matches(query, library);
    if found.is_empty() {
        return format!(
            "No global gotcha matches this query.\nLibraries with records: {}.",
            store.library_counts()
        );
    }
    let budget = usize::try_from(budget_tokens)
        .unwrap_or(usize::MAX)
        .saturating_mul(CHARS_PER_TOKEN);
    let mut body = String::new();
    let mut shown = 0;
    let mut used = 0;
    for record in &found {
        let entry = entry_text(record);
        let size = entry.chars().count();
        if shown > 0 && used + size > budget {
            break;
        }
        body.push_str(&entry);
        used += size;
        shown += 1;
    }
    let library = library.map(str::trim).filter(|library| !library.is_empty());
    let subject = match (query.trim(), library) {
        ("", None) => String::new(),
        ("", Some(library)) => format!(" of {library}"),
        (query, None) => format!(" for \"{query}\""),
        (query, Some(library)) => format!(" for \"{query}\" in {library}"),
    };
    format!(
        "Global gotchas{subject} ({shown} of {} shown):\n{}",
        found.len(),
        body.trim_end()
    )
}
