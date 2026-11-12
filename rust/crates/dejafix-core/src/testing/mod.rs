mod fakes;
mod fixtures;
mod rng;
mod types;

pub use fakes::{FIXED_NOW, fixed_clock};
pub use fixtures::{FIXTURE_SEED, FIXTURE_SIZES, write_project};
pub use types::{FixtureError, NoResolve};

pub use crate::core::text::{is_python_decimal, is_python_space, python_lines};

#[cfg(test)]
mod tests;
