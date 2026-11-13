use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::adapters::fs::{StoreError, read_optional_text};
use crate::core::hooks::shell_path;

#[must_use]
pub fn hooks_dir(root: &Path) -> PathBuf {
    root.join(".git").join("hooks")
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
pub fn resolve_djf_binary(
    exe_dir: Option<&Path>,
    path_var: Option<&OsStr>,
    windows: bool,
) -> String {
    let found = exe_dir
        .and_then(|directory| found_in_exe_dir(directory, windows))
        .or_else(|| path_var.and_then(|var| found_on_path(var, windows)));
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
