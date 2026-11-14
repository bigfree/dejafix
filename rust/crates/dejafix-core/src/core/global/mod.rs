mod checks;
mod hint;
mod prune;
mod query;
mod record;
mod relevant;
mod report;
mod types;
mod upgrade;

pub use checks::{check_gotcha, is_package_name, normalize_input};
pub use hint::{global_hint, hint_candidate};
pub use prune::is_prunable;
pub use query::{DEFAULT_BUDGET, GLOBAL_FIELDS, global_reply};
pub use record::{is_upgraded, line, parse_store, stored_line};
pub(crate) use relevant::is_truthy;
pub use relevant::relevant_entries;
pub use report::{add_reply, filtered, global_report};
pub use types::{
    AddOutcome, CheckFailure, CheckReport, CheckWarning, Draft, DraftError, Entry, GlobalAction,
    GlobalDoc, GlobalHint, GlobalIndex, GlobalOutcome, GotchaInput, GotchaV2, Libraries, Relevant,
    Stack, StoredGotcha, UpgradeOutcome, Usage,
};
pub use upgrade::{
    apply as apply_upgrade, drafts as upgrade_drafts, export_notes, legacy_input, parse_drafts,
    touches_version_one,
};

#[cfg(test)]
mod tests;
