mod excerpt;
mod matcher;
mod rank;
mod types;

pub use excerpt::{excerpt, mcp_match_line, search_reply};
pub use matcher::{match_line, no_match_message, search};
pub use rank::{EVENT_FIELDS, event_index, ranked};
pub use types::{EventDoc, SearchMode, SearchQuery};

#[cfg(test)]
mod tests;
