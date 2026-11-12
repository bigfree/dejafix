mod adapters;
mod core;
mod wiring;

pub use crate::adapters::fs::{
    FsResolver, GlobalStore, MemoryDir, StoreError, detect_stack, find_memory_dir, path_context,
    read_events_file,
};
pub use crate::core::event::{
    CaptureSource, Confidence, Event, EventId, EventKind, Extra, IssueId, LineError, Origin,
    Outcome, Summary,
};
pub use crate::core::global::{
    Entry, PROMOTABLE_SEED, Stack, gotcha_entry, is_gotcha_signal, promotable_libraries,
    promotable_set, promotion, similar,
};
pub use crate::core::log::{EventLog, EventRefError, Skipped};
pub use crate::core::path::{
    PathContext, PathResolver, RelativePath, is_memory_path, location_file, location_to_file,
    project_relative,
};
pub use crate::core::redact::{REDACTABLE_FIELDS, Redaction, SecretKind, redact, redaction_notice};
pub use crate::core::search::{SearchQuery, match_line, no_match_message, search};
pub use crate::core::summary::{
    IssueFile, RECORD_CHARS, SummaryInput, clip_record, extract_purpose_from_map,
    extract_purpose_from_summary, issue_files, key_files, looks_like_placeholder, render_summary,
    slugify_title,
};
pub use crate::core::text::{casefold, python_strip};
pub use crate::core::time::{Clock, Timestamp, TimestampOverflow, iso_micros, normalize_timestamp};
pub use crate::core::write::{
    Command, Ids, MarkerChange, MarkerState, OsIds, ValidationError, WriteContext, WriteOutcome,
    WritePlan, checks_marker, issue_exists, marker_state, next_issue_id, plan_command,
    sole_open_issue,
};
pub use crate::wiring::{
    Project, Recorded, SearchOutput, Settings, WiringError, record, regenerate, search_project,
    show,
};

#[cfg(any(test, feature = "testing"))]
pub mod testing;
