mod capture;
mod edit;
mod install;
mod precheck;
mod types;

pub use capture::auto_capture;
pub use edit::edit_hook;
pub use install::hooks_action;
pub(crate) use install::install;
pub(crate) use precheck::analyze_files;
pub use precheck::precheck;
pub use types::{
    CaptureOutcome, EditHookRun, HooksAction, HooksOutcome, HooksReport, PrecheckOutcome,
    PrecheckRequest,
};

#[cfg(test)]
mod tests;
