mod location;
mod project;
mod types;

pub use location::{is_network_path, location_to_file};
pub(crate) use project::{MEMORY_DIR, normpath};
pub use project::{is_memory_path, location_file, project_relative};
pub use types::{PathContext, PathResolver, RelativePath};

#[cfg(test)]
mod tests;
