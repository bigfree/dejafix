mod adapters;
mod core;
mod wiring;

pub use crate::adapters::fs::{FsResolver, StoreError, path_context, read_events_file};
pub use crate::core::event::{
    CaptureSource, Confidence, Event, EventId, EventKind, Extra, IssueId, LineError, Origin,
    Outcome, Summary,
};
pub use crate::core::log::{EventLog, EventRefError, Skipped};
pub use crate::core::path::{
    PathContext, PathResolver, RelativePath, is_memory_path, location_file, location_to_file,
    project_relative,
};
pub use crate::core::time::{Clock, Timestamp, TimestampOverflow, normalize_timestamp};

#[cfg(any(test, feature = "testing"))]
pub mod testing;
