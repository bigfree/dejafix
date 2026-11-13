mod start;

pub use start::{MAP_POINTER, SESSION_START_DONE, current_plan, placeholder_map, session_start};

#[cfg(test)]
mod tests;
