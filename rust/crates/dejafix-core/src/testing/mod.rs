mod fakes;
mod fixtures;
mod rng;
mod types;

pub use fakes::{FIXED_NOW, fixed_clock};
pub use fixtures::{FIXTURE_SEED, FIXTURE_SIZES, write_project};
pub use types::{FixtureError, NoResolve, SeqIds};

pub use crate::core::json::write_python_json_indented;
pub use crate::core::redact::redact;
pub use crate::core::text::{
    casefold, casefold_table, is_python_decimal, is_python_space, is_python_word, python_lines,
    universal_newlines,
};

#[cfg(test)]
mod tests;
