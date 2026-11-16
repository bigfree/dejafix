mod config;
mod glob;
mod rank;
mod remote;
mod render;
mod resolve;
mod types;

pub use config::{CONFIG_LIMIT, RELATION_CHARS, parse_config, sanitize};
pub use glob::{boundary_globs, matching, path_query};
pub use rank::linked_blocks;
pub use remote::{origin_url, project_identity};
pub use render::{direction_text, linked_report, links_section};
pub use resolve::resolve_links;
pub use types::{ConfigError, Glob, Link, LinkResolution, LinkedBlock, ProjectLinks, Registered};

#[cfg(test)]
mod tests;
