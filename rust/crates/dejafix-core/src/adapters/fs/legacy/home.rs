use std::fs;
use std::io;
use std::path::Path;

#[cfg(unix)]
fn copy_link(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(from)?, to)
}

#[cfg(windows)]
fn copy_link(from: &Path, to: &Path) -> io::Result<()> {
    let target = fs::read_link(from)?;
    if fs::metadata(from).is_ok_and(|metadata| metadata.is_dir()) {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    let kind = fs::symlink_metadata(from)?.file_type();
    if kind.is_symlink() {
        return copy_link(from, to);
    }
    if !kind.is_dir() {
        return fs::copy(from, to).map(drop);
    }
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        copy_tree(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

fn remove_tree(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn move_entry(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            copy_tree(from, to)?;
            remove_tree(from)
        }
        other => other,
    }
}

fn remove_empty_tree(path: &Path) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_empty_tree(&entry.path())?;
        }
    }
    fs::remove_dir(path)
}

pub fn clear_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => remove_empty_tree(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
