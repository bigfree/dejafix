mod excerpt;
mod matcher;
mod types;

pub use excerpt::{excerpt, mcp_match_line, search_reply};
pub use matcher::{match_line, no_match_message, search};
pub use types::SearchQuery;

#[cfg(test)]
mod tests;
