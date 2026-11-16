use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::git;
use super::types::{HooksLocation, UnmanagedReason};
use crate::adapters::fs::{StoreError, identity, points_outside, read_optional_text};
use crate::core::hooks::shell_path;

pub(crate) fn same_file(left: &Path, right: &Path) -> bool {
    matches!((identity(left), identity(right)), (Some(left), Some(right)) if left == right)
}

#[must_use]
pub fn hook_leads_outside(path: &Path, root: &Path, folder: &Path) -> bool {
    points_outside(path, root) && points_outside(path, folder)
}

fn owns_git_dir(root: &Path, git_dir: &Path, common: &Path) -> bool {
    let dot_git = root.join(".git");
    if same_file(git_dir, &dot_git) {
        return same_file(common, &dot_git) && !points_outside(&dot_git, root);
    }
    let linked = git_dir
        .parent()
        .is_some_and(|parent| same_file(parent, &common.join("worktrees")));
    linked
        && read_optional_text(&git_dir.join("gitdir"))
            .ok()
            .flatten()
            .is_some_and(|backlink| {
                let backlink = Path::new(backlink.trim());
                backlink.is_absolute() && same_file(backlink, &dot_git)
            })
}

fn outside_git_dir(path: PathBuf) -> HooksLocation {
    HooksLocation::Unmanaged {
        path,
        reason: UnmanagedReason::OutsideGitDir,
    }
}

fn fallback(root: &Path) -> HooksLocation {
    let dot_git = root.join(".git");
    let hooks = dot_git.join("hooks");
    if dot_git.is_dir() && (points_outside(&dot_git, root) || points_outside(&hooks, &dot_git)) {
        outside_git_dir(hooks)
    } else {
        HooksLocation::Folder(hooks)
    }
}

#[must_use]
pub fn hooks_location(root: &Path) -> HooksLocation {
    let Some(output) = git(
        root,
        &[
            "rev-parse",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
            "--git-path",
            "hooks",
        ],
    ) else {
        return fallback(root);
    };
    let Ok(text) = String::from_utf8(output) else {
        return fallback(root);
    };
    let mut lines = text.lines().map(str::trim);
    let (Some(top), Some(git_dir), Some(common), Some(hooks)) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    else {
        return fallback(root);
    };
    let common = root.join(common);
    if !same_file(Path::new(top), root) || !owns_git_dir(root, Path::new(git_dir), &common) {
        return fallback(root);
    }
    let hooks = root.join(hooks);
    if hooks != common.join("hooks") {
        HooksLocation::Unmanaged {
            path: hooks,
            reason: UnmanagedReason::HooksPath,
        }
    } else if points_outside(&hooks, &common) {
        outside_git_dir(hooks)
    } else {
        HooksLocation::Folder(hooks)
    }
}

#[cfg(unix)]
fn has_execute_bit(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn has_execute_bit(_path: &Path) -> bool {
    true
}

fn is_runnable_file(path: &Path, windows: bool) -> bool {
    path.is_file() && (windows || has_execute_bit(path))
}

const fn binary_names(windows: bool) -> [&'static str; 2] {
    if windows {
        ["djf.exe", "dejafix.exe"]
    } else {
        ["djf", "dejafix"]
    }
}

fn found_in_exe_dir(exe_dir: &Path, windows: bool) -> Option<PathBuf> {
    binary_names(windows)
        .into_iter()
        .map(|name| exe_dir.join(name))
        .find(|candidate| is_runnable_file(candidate, windows))
}

fn found_on_path(path_var: &OsStr, windows: bool) -> Option<PathBuf> {
    binary_names(windows).into_iter().find_map(|name| {
        std::env::split_paths(path_var)
            .filter(|directory| !directory.as_os_str().is_empty())
            .map(|directory| directory.join(name))
            .find(|candidate| is_runnable_file(candidate, windows))
    })
}

#[must_use]
pub fn resolve_djf_binary(exe: Option<&Path>, path_var: Option<&OsStr>, windows: bool) -> String {
    let on_path = path_var.and_then(|var| found_on_path(var, windows));
    let found = on_path
        .clone()
        .filter(|candidate| {
            candidate.is_absolute() && exe.is_some_and(|exe| same_file(candidate, exe))
        })
        .or_else(|| {
            exe.and_then(Path::parent)
                .and_then(|directory| found_in_exe_dir(directory, windows))
        })
        .or(on_path);
    match found {
        Some(path) => shell_path(&path.to_string_lossy()),
        None => "djf".to_owned(),
    }
}

pub fn read_hook(path: &Path) -> Result<Option<String>, StoreError> {
    read_optional_text(path)
}

fn write_failure(path: &Path) -> impl Fn(io::Error) -> StoreError + '_ {
    |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)?.permissions().mode();
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o111))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

pub fn write_hook(path: &Path, text: &str, existing: bool) -> Result<(), StoreError> {
    let failed = write_failure(path);
    if existing {
        fs::write(path, text.as_bytes())
    } else {
        File::create_new(path).and_then(|mut file| file.write_all(text.as_bytes()))
    }
    .map_err(&failed)?;
    make_executable(path).map_err(failed)
}

#[must_use]
pub fn hook_has_crlf(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes.windows(2).any(|pair| pair == b"\r\n"))
}

pub fn remove_hook(path: &Path) -> Result<(), StoreError> {
    fs::remove_file(path).map_err(write_failure(path))
}
