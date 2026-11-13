mod diff;
mod head;
mod history;
mod hooks;
mod run;
mod state;
mod status;
mod types;

pub use diff::{last_changes, last_message, staged_files, working_files};
pub use head::head_short;
pub use history::recent_changes;
pub use hooks::{hooks_dir, read_hook, remove_hook, resolve_djf_binary, write_hook};
pub(crate) use run::{git, git_status};
pub use state::{git_dirs, repository_state};
pub use status::status_files;
pub use types::{GitDirs, GitHistory, RepoState};

#[cfg(test)]
mod tests;
