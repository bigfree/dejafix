mod events;
mod global;
mod marker;
mod resolve;
mod stack;
mod store;
mod types;
mod write;

pub use events::{lock_events, read_events_file};
pub use marker::{clear_marker, read_marker, write_marker};
pub use resolve::path_context;
pub use stack::detect_stack;
pub use store::{find_memory_dir, issues_dir, project_name, read_optional_text, read_text};
pub use types::{FsResolver, GlobalStore, MemoryDir, StoreError};
pub use write::{write_text, write_text_if_changed};

#[cfg(test)]
mod tests;
