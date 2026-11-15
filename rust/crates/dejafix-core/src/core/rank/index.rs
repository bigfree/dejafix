use std::borrow::Cow;
use std::collections::HashMap;

use super::types::{Analyzer, Document, FieldSpec, Index, QueryTerms};

pub(super) fn slot(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

struct Entry {
    term: u32,
    field: u32,
    doc: u32,
    frequency: u16,
}

fn postings_table(entries: &[Entry], terms: usize, fields: usize) -> (Vec<u32>, Vec<(u32, u16)>) {
    let bucket = |entry: &Entry| slot(entry.term) * fields + slot(entry.field);
    let mut offsets = vec![0_u32; terms * fields + 1];
    for entry in entries {
        if let Some(offset) = offsets.get_mut(bucket(entry) + 1) {
            *offset += 1;
        }
    }
    let mut running = 0;
    for offset in &mut offsets {
        running += *offset;
        *offset = running;
    }
    let mut next = offsets.clone();
    let mut postings = vec![(0_u32, 0_u16); entries.len()];
    for entry in entries {
        if let Some(cursor) = next.get_mut(bucket(entry))
            && let Some(posting) = postings.get_mut(slot(*cursor))
        {
            *posting = (entry.doc, entry.frequency);
            *cursor += 1;
        }
    }
    (offsets, postings)
}

fn average_lengths(lengths: &[u16], fields: usize, documents: usize) -> Vec<f64> {
    (0..fields)
        .map(|field| {
            let total: u64 = lengths
                .get(field * documents..(field + 1) * documents)
                .unwrap_or_default()
                .iter()
                .map(|length| u64::from(*length))
                .sum();
            if documents == 0 {
                0.0
            } else {
                f64::from(u32::try_from(total).unwrap_or(u32::MAX)) / f64::from(narrow(documents))
            }
        })
        .collect()
}

fn id_order(docs: &[&dyn Document]) -> Vec<u32> {
    let mut by_id: Vec<(&str, usize)> = docs
        .iter()
        .enumerate()
        .map(|(index, doc)| (doc.id(), index))
        .collect();
    by_id.sort_unstable();
    let mut order = vec![0_u32; docs.len()];
    for (rank, (_, doc)) in by_id.into_iter().enumerate() {
        if let Some(target) = order.get_mut(doc) {
            *target = narrow(rank);
        }
    }
    order
}

impl Index {
    #[must_use]
    pub fn build<D: Document>(fields: &'static [FieldSpec], docs: &[D]) -> Self {
        let docs: Vec<&dyn Document> = docs.iter().map(|doc| -> &dyn Document { doc }).collect();
        Self::build_from(fields, &docs)
    }

    fn build_from(fields: &'static [FieldSpec], docs: &[&dyn Document]) -> Self {
        let documents = docs.len().min(slot(u32::MAX));
        let docs = docs.get(..documents).unwrap_or_default();
        let mut ids: HashMap<Cow<'_, str>, u32> = HashMap::new();
        let mut frequency: Vec<u32> = Vec::new();
        let mut last_doc: Vec<u32> = Vec::new();
        let mut entries: Vec<Entry> = Vec::new();
        let mut lengths = vec![0_u16; fields.len() * documents];
        let mut scratch: Vec<Cow<'_, str>> = Vec::new();
        let mut field_terms: Vec<u32> = Vec::new();
        for (doc_index, doc) in docs.iter().enumerate() {
            let doc_id = narrow(doc_index);
            for field in 0..fields.len() {
                doc.texts(field, &mut |text| Analyzer.terms(text, &mut scratch));
                field_terms.clear();
                for term in scratch.drain(..) {
                    let next = narrow(ids.len());
                    let id = *ids.entry(term).or_insert(next);
                    if id == next {
                        frequency.push(0);
                        last_doc.push(u32::MAX);
                    }
                    field_terms.push(id);
                }
                if let Some(length) = lengths.get_mut(field * documents + doc_index) {
                    *length = u16::try_from(field_terms.len()).unwrap_or(u16::MAX);
                }
                field_terms.sort_unstable();
                for run in field_terms.chunk_by(|left, right| left == right) {
                    let Some(&term) = run.first() else { continue };
                    if let Some(last) = last_doc.get_mut(slot(term))
                        && *last != doc_id
                    {
                        *last = doc_id;
                        if let Some(count) = frequency.get_mut(slot(term)) {
                            *count += 1;
                        }
                    }
                    entries.push(Entry {
                        term,
                        field: narrow(field),
                        doc: doc_id,
                        frequency: u16::try_from(run.len()).unwrap_or(u16::MAX),
                    });
                }
            }
        }
        let mut vocabulary: Vec<(Cow<'_, str>, u32)> = ids.into_iter().collect();
        vocabulary.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let mut renumber = vec![0_u32; vocabulary.len()];
        for (new, (_, old)) in vocabulary.iter().enumerate() {
            if let Some(target) = renumber.get_mut(slot(*old)) {
                *target = narrow(new);
            }
        }
        let document_frequency = vocabulary
            .iter()
            .map(|(_, old)| frequency.get(slot(*old)).copied().unwrap_or(0))
            .collect();
        for entry in &mut entries {
            entry.term = renumber.get(slot(entry.term)).copied().unwrap_or(0);
        }
        let (offsets, postings) = postings_table(&entries, vocabulary.len(), fields.len());
        let average = average_lengths(&lengths, fields.len(), documents);
        let order = id_order(docs);
        Self {
            fields,
            terms: vocabulary
                .into_iter()
                .map(|(term, _)| Box::from(term.as_ref()))
                .collect(),
            document_frequency,
            offsets,
            postings,
            lengths,
            average,
            order,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    #[must_use]
    pub fn query_terms(&self, text: &str) -> QueryTerms {
        let mut analyzed = Vec::new();
        Analyzer.terms(text, &mut analyzed);
        analyzed.sort_unstable();
        analyzed.dedup();
        QueryTerms {
            ids: analyzed
                .iter()
                .filter_map(|term| {
                    self.terms
                        .binary_search_by(|known| known.as_ref().cmp(term.as_ref()))
                        .ok()
                })
                .map(narrow)
                .collect(),
            distinct: analyzed.len(),
        }
    }

    pub(super) fn postings(&self, term: u32, field: usize) -> &[(u32, u16)] {
        let bucket = slot(term) * self.fields.len() + field;
        let start = self.offsets.get(bucket).copied().map_or(0, slot);
        let end = self.offsets.get(bucket + 1).copied().map_or(start, slot);
        self.postings.get(start..end).unwrap_or_default()
    }

    pub(super) fn length(&self, field: usize, doc: u32) -> u16 {
        self.lengths
            .get(field * self.len() + slot(doc))
            .copied()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn terms_covered(&self, doc: u32, query_terms: &[u32]) -> usize {
        query_terms
            .iter()
            .filter(|term| {
                (0..self.fields.len()).any(|field| {
                    self.postings(**term, field)
                        .binary_search_by_key(&doc, |posting| posting.0)
                        .is_ok()
                })
            })
            .count()
    }
}
