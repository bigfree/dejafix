mod doctor;
mod migrate;
mod types;
mod update;

pub use doctor::{doctor, scan_projects};
pub use migrate::migrate;
pub use types::{MigrateEvent, MigrateOptions, ProjectState, UpdateStatus};
pub use update::{set_auto_check, update_status};

#[cfg(test)]
mod tests;
