mod files;
mod issues;
mod purpose;
mod render;
mod types;

pub use files::{RECORD_CHARS, clip_record, key_files};
pub use issues::{issue_files, slugify_title};
pub use purpose::{extract_purpose_from_map, extract_purpose_from_summary, looks_like_placeholder};
pub use render::render_summary;
pub use types::{IssueFile, SummaryInput};

#[cfg(test)]
mod tests;
