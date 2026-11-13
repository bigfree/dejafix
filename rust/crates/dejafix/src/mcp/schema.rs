use std::sync::Arc;

use rmcp::model::Tool;

use super::instructions::{GLOBAL_INSTRUCTIONS, INSTRUCTIONS};

const TOOLS: &str = include_str!("tools.json");

pub(super) fn tools() -> Result<Arc<[Tool]>, serde_json::Error> {
    serde_json::from_str::<Vec<Tool>>(TOOLS).map(Arc::from)
}

#[must_use]
pub fn instructions(global_mode: bool) -> String {
    if global_mode {
        format!("{INSTRUCTIONS}{GLOBAL_INSTRUCTIONS}")
    } else {
        INSTRUCTIONS.to_owned()
    }
}
