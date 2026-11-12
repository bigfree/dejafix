mod inherit;
mod promote;
mod types;

pub use inherit::{inherited_block, relevant_entries};
pub use promote::{
    PROMOTABLE_SEED, gotcha_entry, is_gotcha_signal, promotable_libraries, promotable_set,
    promotion, similar,
};
pub use types::{Entry, Relevant, Stack};

#[cfg(test)]
mod tests;
