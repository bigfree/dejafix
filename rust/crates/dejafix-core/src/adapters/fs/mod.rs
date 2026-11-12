mod events;
mod global;
mod manifest;
mod marker;
mod registry;
mod resolve;
mod scan;
mod stack;
mod store;
mod types;
mod write;

pub use events::{lock_events, read_events_file};
pub use manifest::manifest_info;
pub use marker::{clear_marker, read_marker, write_marker};
pub use registry::resolve_path;
pub use resolve::path_context;
pub use scan::{
    ClientConfig, ClientPaths, ConfigState, client_configs, pinned_client_configs,
    scan_client_configs,
};
pub use stack::detect_stack;
pub use store::{
    find_memory_dir, issues_dir, project_name, read_optional_text, read_text, read_text_within,
};
pub use types::{FsResolver, GlobalStore, MemoryDir, RegistryFiles, StoreError};
pub use write::{write_text, write_text_if_changed};

#[cfg(test)]
mod tests;
