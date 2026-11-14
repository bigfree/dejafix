mod checks;
mod inherit;
mod promote;
mod prune;
mod query;
mod record;
mod report;
mod types;

pub use checks::{check_gotcha, normalize_input};
pub(crate) use inherit::is_truthy;
pub use inherit::{inherited_block, relevant_entries};
pub use promote::{
    PROMOTABLE_SEED, gotcha_entry, is_gotcha_signal, promotable_libraries, promotable_set,
    promotion, similar,
};
pub use prune::is_prunable;
pub use query::{DEFAULT_BUDGET, GLOBAL_FIELDS, global_reply};
pub use record::{is_upgraded, line, parse_store};
pub use report::{add_reply, filtered, global_report};
pub use types::{
    AddOutcome, CheckFailure, CheckReport, CheckWarning, Entry, GlobalAction, GlobalDoc,
    GlobalIndex, GlobalOutcome, GotchaInput, GotchaV2, Libraries, Relevant, Stack, StoredGotcha,
    Usage,
};

#[cfg(test)]
mod tests;
