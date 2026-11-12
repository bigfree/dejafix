use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::core::lessons::{marker_name, session_key};
use crate::core::text::{platform_newlines, universal_newlines};

const MAIN_AGENT: &str = "main";

#[cfg(unix)]
fn owner() -> String {
    rustix::process::getuid().as_raw().to_string()
}

#[cfg(not(unix))]
fn owner() -> String {
    "user".to_owned()
}

fn private_dir() -> DirBuilder {
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
}

#[cfg(unix)]
fn trusted(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.uid() == rustix::process::getuid().as_raw() && metadata.mode().trailing_zeros() >= 6
}

#[cfg(not(unix))]
fn trusted(_: &fs::Metadata) -> bool {
    true
}

pub fn seen_root(temp: &Path) -> io::Result<Option<PathBuf>> {
    let base = temp.join(format!("djf-edit-hook-{}", owner()));
    match private_dir().create(&base) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&base)?;
    Ok((metadata.is_dir() && trusted(&metadata)).then_some(base))
}

#[must_use]
pub fn report_is_new(
    temp: &Path,
    session: &str,
    agent: &str,
    file_key: &str,
    report: &str,
) -> bool {
    if session.is_empty() {
        return true;
    }
    let Ok(Some(base)) = seen_root(temp) else {
        return true;
    };
    let agent = session_key(agent);
    let agent = if agent.is_empty() { MAIN_AGENT } else { &agent };
    let marker = base
        .join(session_key(session))
        .join(agent)
        .join(marker_name(file_key));
    if marker.is_file()
        && fs::read_to_string(&marker).is_ok_and(|seen| universal_newlines(&seen) == report)
    {
        return false;
    }
    let _ = remember(&marker, report);
    true
}

fn remember(marker: &Path, report: &str) -> io::Result<()> {
    let parent = marker.parent().ok_or(io::ErrorKind::NotFound)?;
    let mut builder = private_dir();
    builder.recursive(true).create(parent)?;
    let name = marker
        .file_name()
        .ok_or(io::ErrorKind::NotFound)?
        .to_string_lossy();
    let pending = parent.join(format!("{name}.{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&pending)?;
    file.write_all(platform_newlines(report).as_bytes())?;
    drop(file);
    fs::rename(&pending, marker)
}

pub fn forget_session(temp: &Path, session: &str) {
    if session.is_empty() {
        return;
    }
    let Ok(Some(base)) = seen_root(temp) else {
        return;
    };
    let target = base.join(session_key(session));
    if fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.is_dir()) {
        let _ = fs::remove_dir_all(target);
    }
}
