mod doctor;
mod files;
mod hooks;
mod init;
mod mcp;
mod memory;
mod project;
mod python;
mod read;
mod record;
mod registry;
mod types;

pub use doctor::{doctor, scan_projects};
pub use files::{add_plan_idea, read_memory_file};
pub use hooks::{
    CaptureOutcome, EditHookRun, HooksAction, HooksOutcome, HooksReport, PrecheckOutcome,
    PrecheckRequest, auto_capture, edit_hook, hooks_action, precheck,
};
pub use init::{init_project, other_projects};
pub use mcp::{CachedLog, McpSettings, McpState, ToolCall, ToolReply, WriteCall};
pub use memory::{brief, global};
pub use read::{search_project, show};
pub use record::{record, regenerate};
pub use registry::{register_project, registry_op, upgrade_notice};
pub use types::{
    BridgeOutcome, CorruptLog, DoctorOptions, DoctorReport, FoundProject, Inherited, InitHooks,
    InitOptions, InitReport, ListedProject, MemoryFile, Project, Recorded, RegistryOp,
    RegistryReply, ScanReport, SearchOutput, Settings, UpgradeNotice, WiringError,
};

#[cfg(test)]
mod tests;
