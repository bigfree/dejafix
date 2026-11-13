mod decide;
mod types;

pub use decide::resolve;
pub use types::{Probe, Resolution, ResolveError, Source};

#[cfg(test)]
mod tests;
