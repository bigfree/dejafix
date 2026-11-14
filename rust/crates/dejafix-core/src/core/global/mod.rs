mod inherit;
mod promote;
mod prune;
mod record;
mod report;
mod types;

pub(crate) use inherit::is_truthy;
pub use inherit::{inherited_block, relevant_entries};
pub use promote::{
    PROMOTABLE_SEED, gotcha_entry, is_gotcha_signal, promotable_libraries, promotable_set,
    promotion, similar,
};
pub use prune::is_prunable;
pub use record::{is_upgraded, line, parse_store};
pub use report::{filtered, global_report};
pub use types::{
    Entry, GlobalAction, GlobalOutcome, GotchaV2, Libraries, Relevant, Stack, StoredGotcha, Usage,
};

#[cfg(test)]
mod tests;
