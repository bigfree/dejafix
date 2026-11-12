mod find;
mod types;

pub use find::{STALE_COMMIT_THRESHOLD, find_stale, stale_label};
pub use types::{CommitsSince, FileHistory, Stale};

#[cfg(test)]
mod tests;
