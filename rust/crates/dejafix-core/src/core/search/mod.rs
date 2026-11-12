mod matcher;
mod types;

pub use matcher::{match_line, no_match_message, search};
pub use types::SearchQuery;

#[cfg(test)]
mod tests;
