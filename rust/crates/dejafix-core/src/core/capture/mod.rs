mod classify;
mod rank;
mod types;

pub use classify::{capture_event, classify, first_line, meets_threshold, merge_event};
pub use rank::{capture_files, location_rank, parse_name_status, pick_location};
pub use types::{Change, Classified};

#[cfg(test)]
mod tests;
