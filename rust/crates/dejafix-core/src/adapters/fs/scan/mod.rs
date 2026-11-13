mod clients;
mod types;
mod walk;

pub use clients::{client_configs, pinned_client_configs, scan_client_configs};
pub use types::{ClientConfig, ClientPaths, ConfigState, Identity};
pub use walk::{dedupe_paths, default_roots, find_projects, identity};

#[cfg(test)]
mod tests;
