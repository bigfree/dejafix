mod calc;
mod render;
mod types;

pub use calc::{calculate_score, raw_events, score_event, score_event_of};
pub use render::score_reply;
pub use types::{Score, ScoreError, ScoreEvent};

#[cfg(test)]
mod tests;
