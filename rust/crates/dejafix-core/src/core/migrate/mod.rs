mod blocks;
mod steps;
mod types;

pub use blocks::{
    BRIDGE_BLOCK, HOOK_BLOCK, LEGACY_DIR, convert_block, has_legacy_path, is_stale_config,
    rewrite_legacy_paths,
};
pub use types::{Block, MigrateStep};

#[cfg(test)]
mod tests;
