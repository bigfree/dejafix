mod snippet;
mod sync;
mod types;

pub use snippet::{auto_capture_snippet, precheck_snippet, shell_path};
pub use sync::{remove_blocks, sync_hook};
pub use types::{HOOK_MARKER_END, HOOK_MARKER_START, HOOK_SHEBANG, HookChange, HookKind, Removal};

#[cfg(test)]
mod tests;
