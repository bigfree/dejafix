mod files;
mod global;
mod hooks;
mod init;
mod links;
mod mcp;
mod memory;
mod project;
mod python;
mod read;
mod record;
mod registry;
mod setup;
mod types;

pub use files::{add_plan_idea, read_memory_file};
pub use global::add_gotcha;
pub use hooks::{
    CaptureOutcome, EditHookRun, HooksAction, HooksOutcome, HooksReport, PrecheckOutcome,
    PrecheckRequest, auto_capture, edit_hook, hooks_action, precheck,
};
pub use init::{init_project, other_projects};
pub use links::links_listing;
pub use mcp::{CachedLog, McpSettings, McpState, ToolCall, ToolReply, WriteCall};
pub use memory::{brief, global};
pub use read::{search_project, show};
pub use record::{record, regenerate};
pub use registry::{register_project, registry_op, upgrade_notice};
pub use setup::{
    MigrateEvent, MigrateOptions, ProjectState, UpdateStatus, doctor, migrate, scan_projects,
    set_auto_check, update_status,
};
pub use types::{
    BridgeOutcome, CorruptLog, DoctorOptions, DoctorReport, FoundProject, InitHooks, InitOptions,
    InitReport, IssueScope, ListedProject, MemoryFile, Project, Recorded, Regenerated, RegistryOp,
    RegistryReply, ScanReport, SearchOutput, Searched, Settings, UpgradeNotice, WiringError,
};

#[cfg(test)]
mod tests;
