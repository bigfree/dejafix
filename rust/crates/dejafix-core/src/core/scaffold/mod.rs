mod bridge;
mod files;
mod initial;
mod instructions;
mod map;
mod types;

pub use bridge::{BRIDGE_END, BRIDGE_START, apply_bridge, bridge_block};
pub use files::{gitignore_with, plan_with_idea};
pub use initial::{INITIAL_CONFIG, initial_plan, initial_project_map, initial_summary};
pub use instructions::{
    INSTRUCTIONS_BACKUP_FILE, INSTRUCTIONS_VERSION, TITLE_CHARS, ai_instructions,
    extract_global_block, instructions_version, splice_global_block,
};
pub use map::{project_map_from_stack, stack_summary_suffix};
pub use types::{BridgeChange, ManifestInfo};

#[cfg(test)]
mod tests;
