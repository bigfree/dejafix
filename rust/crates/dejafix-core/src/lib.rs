mod adapters;
mod core;
mod wiring;

pub use crate::adapters::fs::{
    ClientConfig, ClientPaths, ConfigState, FsResolver, GlobalStore, Identity, MemoryDir,
    RegistryFiles, StoreError, canonical_path, client_configs, dedupe_paths, default_roots,
    detect_stack, find_memory_dir, find_projects, identity, manifest_info, path_context,
    pinned_client_configs, read_events_file, resolve_path, scan_client_configs,
};
pub use crate::adapters::fs::{forget_session, report_is_new, seen_root};
pub use crate::adapters::git::{GitHistory, recent_changes};
pub use crate::adapters::git::{hooks_dir, read_hook, remove_hook, resolve_djf_binary, write_hook};
pub use crate::adapters::git::{
    last_changes, last_message, staged_files, status_files, working_files,
};
pub use crate::adapters::http::latest_version;
pub use crate::core::brief::{BriefInput, render_brief};
pub use crate::core::capture::{
    Change, Classified, capture_event, capture_files, classify, first_line, location_rank,
    meets_threshold, merge_event, parse_name_status, pick_location,
};
pub use crate::core::context::{ContextInput, generate_context};
pub use crate::core::doctor::{Reverted, SeenConfig, is_newer, track_pins, version_key};
pub use crate::core::event::{
    CaptureSource, Confidence, Event, EventId, EventKind, Extra, IssueId, LineError, Origin,
    Outcome, Summary,
};
pub use crate::core::global::{
    Entry, GlobalAction, GlobalOutcome, PROMOTABLE_SEED, Relevant, Stack, Usage, filtered,
    global_report, gotcha_entry, inherited_block, is_gotcha_signal, is_prunable,
    promotable_libraries, promotable_set, promotion, relevant_entries, similar,
};
pub use crate::core::hooks::{
    HOOK_MARKER_END, HOOK_MARKER_START, HOOK_SHEBANG, HookChange, HookKind, Removal,
    auto_capture_snippet, precheck_snippet, remove_blocks, shell_path, sync_hook,
};
pub use crate::core::lessons::{
    LessonKind, Memory, Record, crc32, edited_paths, hook_output, is_skipped, main_checkout,
    marker_name, mention, python_json, records, session_key,
};
pub use crate::core::log::{EventLog, EventRefError, Skipped, skipped_lines};
pub use crate::core::path::{
    PathContext, PathResolver, RelativePath, is_memory_path, location_file, location_to_file,
    project_relative,
};
pub use crate::core::precheck::{
    FAILED_ATTEMPT_BLOCK_COUNT, HIGH_CHURN_THRESHOLD, RECENT_DAYS, Severity, SnoozeError, Targets,
    Warning, WarningKind, age, analyze, expiry_iso, parse_snooze, read_expiry, remaining,
    render_warnings, snooze_note, snoozed_until, targets,
};
pub use crate::core::redact::redact_event;
pub use crate::core::redact::{REDACTABLE_FIELDS, Redaction, SecretKind, redact, redaction_notice};
pub use crate::core::registry::{
    DEFAULT_BRAIN, Loaded, ProjectRecord, RegisterResult, Registry, RegistryError, is_slug,
    list_payload, load as load_registry, meta_payload, normalize_path, same_path, slugify,
    unique_id,
};
pub use crate::core::reply::{current_reply, gotchas_reply, precheck_report, projects_reply};
pub use crate::core::resolve::{Probe, Resolution, ResolveError, Source, resolve};
pub use crate::core::scaffold::{
    BRIDGE_END, BRIDGE_START, BridgeChange, INITIAL_CONFIG, INSTRUCTIONS_BACKUP_FILE,
    INSTRUCTIONS_VERSION, ManifestInfo, TITLE_CHARS, ai_instructions, apply_bridge, bridge_block,
    extract_global_block, gitignore_with, initial_plan, initial_project_map, initial_summary,
    instructions_version, plan_with_idea, project_map_from_stack, splice_global_block,
    stack_summary_suffix,
};
pub use crate::core::score::{
    Score, ScoreError, ScoreEvent, calculate_score, raw_events, score_event, score_event_of,
    score_reply,
};
pub use crate::core::search::{
    SearchQuery, excerpt, match_line, mcp_match_line, no_match_message, search, search_reply,
};
pub use crate::core::session::{
    MAP_POINTER, SESSION_START_DONE, current_plan, placeholder_map, session_start,
};
pub use crate::core::stale::{
    CommitsSince, FileHistory, STALE_COMMIT_THRESHOLD, Stale, find_stale, stale_label,
};
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
    BridgeOutcome, CaptureOutcome, CorruptLog, DoctorOptions, DoctorReport, EditHookRun,
    FoundProject, HooksAction, HooksOutcome, HooksReport, Inherited, InitHooks, InitOptions,
    InitReport, ListedProject, MemoryFile, PrecheckOutcome, PrecheckRequest, Project, Recorded,
    RegistryOp, RegistryReply, ScanReport, SearchOutput, Settings, UpgradeNotice, WiringError,
    add_plan_idea, auto_capture, brief, doctor, edit_hook, global, hooks_action, init_project,
    other_projects, precheck, read_memory_file, record, regenerate, register_project, registry_op,
    scan_projects, search_project, show, upgrade_notice,
};
pub use crate::wiring::{CachedLog, McpSettings, McpState, ToolCall, ToolReply, WriteCall};

#[cfg(any(test, feature = "testing"))]
pub mod testing;
