use super::glob::{matching, path_query};
use super::render::lesson_line;
use super::types::{Glob, LessonDoc, Link, LinkedBlock};
use crate::core::lessons::Lesson;
use crate::core::rank::{Document, FieldSpec, Index, RankParams};

pub const SHOWN_PER_PROJECT: usize = 3;

pub const LINK_PARAMS: RankParams = RankParams {
    min_coverage: 0.25,
    min_ratio: 0.5,
    ..RankParams::DEFAULT
};

const FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "summary",
        weight: 1.0,
    },
    FieldSpec {
        name: "file",
        weight: 2.0,
    },
];

impl Document for LessonDoc<'_> {
    fn id(&self) -> &str {
        &self.id
    }

    fn texts<'d>(&'d self, field: usize, visit: &mut dyn FnMut(&'d str)) {
        match field {
            0 => visit(self.summary),
            1 => self.files.iter().for_each(|file| visit(file)),
            _ => {}
        }
    }
}

#[must_use]
pub fn linked_blocks(
    link: &Link,
    lessons: &[Lesson<'_, '_>],
    globs: &[Glob],
    queries: &[&str],
) -> Vec<Option<LinkedBlock>> {
    let docs: Vec<LessonDoc<'_>> = lessons
        .iter()
        .enumerate()
        .map(|(number, lesson)| LessonDoc {
            id: format!("{number:08}"),
            summary: lesson.record.summary.as_ref(),
            files: lesson
                .files
                .iter()
                .map(|file| path_query(file, matching(globs, file)))
                .collect(),
        })
        .collect();
    let index = Index::build(FIELDS, &docs);
    queries
        .iter()
        .map(|query| {
            let scored = index.rank(query, &LINK_PARAMS);
            let best = scored.first()?.score;
            let lines = scored
                .iter()
                .take(SHOWN_PER_PROJECT)
                .filter_map(|found| lessons.get(usize::try_from(found.doc).ok()?))
                .map(lesson_line)
                .collect();
            Some(LinkedBlock {
                id: link.id.clone(),
                relation: link.relation.clone(),
                lines,
                hidden: scored.len().saturating_sub(SHOWN_PER_PROJECT),
                query: (*query).to_owned(),
                score: best,
            })
        })
        .collect()
}
