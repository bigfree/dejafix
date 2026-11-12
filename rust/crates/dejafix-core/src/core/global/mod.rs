mod promote;
mod types;

pub use promote::{
    PROMOTABLE_SEED, gotcha_entry, is_gotcha_signal, promotable_libraries, promotable_set,
    promotion, similar,
};
pub use types::{Entry, Stack};

#[cfg(test)]
mod tests;
