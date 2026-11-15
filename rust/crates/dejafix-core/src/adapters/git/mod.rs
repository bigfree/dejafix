mod diff;
mod head;
mod history;
mod hooks;
mod ignore;
mod run;
mod state;
mod status;
mod types;

pub use diff::{last_changes, last_message, staged_files, working_files};
pub use head::head_short;
pub use history::recent_changes;
pub use hooks::{
    hook_has_crlf, hooks_location, read_hook, remove_hook, resolve_djf_binary, write_hook,
};
pub use ignore::{exclude_file, is_ignored};
pub(crate) use run::{git, git_status};
pub use state::{git_dirs, repository_state};
pub use status::status_files;
pub use types::{GitDirs, GitHistory, HooksLocation, RepoState};

#[cfg(test)]
mod tests;
