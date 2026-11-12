mod adapters;
mod core;
mod wiring;

pub use crate::adapters::fs::{
    ClientConfig, ClientPaths, ConfigState, FsResolver, GlobalStore, MemoryDir, RegistryFiles,
    StoreError, client_configs, detect_stack, find_memory_dir, manifest_info, path_context,
    pinned_client_configs, read_events_file, resolve_path, scan_client_configs,
};
pub use crate::core::event::{
    CaptureSource, Confidence, Event, EventId, EventKind, Extra, IssueId, LineError, Origin,
    Outcome, Summary,
};
pub use crate::core::global::{
    Entry, PROMOTABLE_SEED, Relevant, Stack, gotcha_entry, inherited_block, is_gotcha_signal,
    promotable_libraries, promotable_set, promotion, relevant_entries, similar,
};
pub use crate::core::log::{EventLog, EventRefError, Skipped};
pub use crate::core::path::{
    PathContext, PathResolver, RelativePath, is_memory_path, location_file, location_to_file,
    project_relative,
};
pub use crate::core::redact::{REDACTABLE_FIELDS, Redaction, SecretKind, redact, redaction_notice};
pub use crate::core::registry::{
    DEFAULT_BRAIN, Loaded, ProjectRecord, RegisterResult, Registry, RegistryError, is_slug,
    list_payload, load as load_registry, meta_payload, normalize_path, same_path, slugify,
    unique_id,
};
pub use crate::core::scaffold::{
    BRIDGE_END, BRIDGE_START, BridgeChange, INITIAL_CONFIG, INSTRUCTIONS_BACKUP_FILE,
    INSTRUCTIONS_VERSION, ManifestInfo, TITLE_CHARS, ai_instructions, apply_bridge, bridge_block,
    extract_global_block, gitignore_with, initial_plan, initial_project_map, initial_summary,
    instructions_version, plan_with_idea, project_map_from_stack, splice_global_block,
    stack_summary_suffix,
};
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
    BridgeOutcome, Inherited, InitOptions, InitReport, ListedProject, MemoryFile, Project,
    Recorded, RegistryOp, RegistryReply, SearchOutput, Settings, UpgradeNotice, WiringError,
    add_plan_idea, init_project, other_projects, read_memory_file, record, regenerate,
    register_project, registry_op, search_project, show, upgrade_notice,
};

#[cfg(any(test, feature = "testing"))]
pub mod testing;
