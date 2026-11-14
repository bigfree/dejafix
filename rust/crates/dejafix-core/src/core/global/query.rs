use super::types::{GlobalDoc, Libraries, StoredGotcha};
use crate::core::rank::{Document, FieldSpec};

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
                visit(self.record.library());
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

pub(super) fn global_docs<'r>(
    records: impl Iterator<Item = &'r StoredGotcha>,
    libraries: &'r Libraries,
) -> Vec<GlobalDoc<'r>> {
    records
        .map(|record| GlobalDoc {
            record,
            aliases: libraries.aliases(&libraries.canonical(record.library())),
        })
        .collect()
}
