mod events;
mod global;
mod legacy;
mod manifest;
mod marker;
mod registry;
mod resolve;
mod scan;
mod seen;
mod stack;
mod store;
mod types;
mod write;

pub(crate) use events::checked_events_path;
pub use events::{events_path, lock_events, read_events_file, read_events_unlocked};
pub(crate) use global::read_json_text;
pub(crate) use legacy::{
    clear_destination, files_with_legacy_paths, holds_data, is_symlink, live_watcher, move_entry,
    read_universal,
};
pub use manifest::manifest_info;
pub use marker::{
    clear_marker, read_marker, read_snooze, remove_snooze, write_marker, write_snooze,
};
pub use registry::{canonical_path, resolve_path};
pub use resolve::path_context;
pub(crate) use resolve::posix;
pub use scan::{
    ClientConfig, ClientPaths, ConfigState, Identity, client_configs, dedupe_paths, default_roots,
    find_projects, identity, pinned_client_configs, scan_client_configs,
};
pub use seen::{forget_session, report_is_new, seen_root};
pub use stack::detect_stack;
pub(crate) use store::{MEMORY_DIR, points_outside};
pub use store::{
    find_memory_dir, issues_dir, project_name, read_optional_text, read_text_inside,
    read_text_within,
};
pub(crate) use types::LockedLog;
pub use types::{FsResolver, GlobalStore, MemoryDir, RegistryFiles, StoreError};
pub(crate) use write::write_atomic;
pub use write::{write_text, write_text_if_changed};

#[cfg(test)]
mod tests;
