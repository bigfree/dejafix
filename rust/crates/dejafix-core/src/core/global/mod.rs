mod inherit;
mod promote;
mod prune;
mod report;
mod types;

pub(crate) use inherit::is_truthy;
pub use inherit::{inherited_block, relevant_entries};
pub use promote::{
    PROMOTABLE_SEED, gotcha_entry, is_gotcha_signal, promotable_libraries, promotable_set,
    promotion, similar,
};
pub use prune::is_prunable;
pub use report::{filtered, global_report};
pub use types::{Entry, GlobalAction, GlobalOutcome, Relevant, Stack, Usage};

#[cfg(test)]
mod tests;
