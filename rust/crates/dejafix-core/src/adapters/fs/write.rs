use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use super::types::StoreError;
use crate::core::text::platform_newlines;

const TEMP_PREFIX: &str = ".djf-";

fn failed(path: &Path) -> impl Fn(io::Error) -> StoreError + '_ {
    move |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    }
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), StoreError> {
    let existing = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if fs::symlink_metadata(path).is_ok() {
                fs::remove_file(path).map_err(failed(path))?;
            }
            None
        }
        Err(error) => return Err(failed(path)(error)),
    };
    let Some(permissions) = existing else {
        let mut file = File::create_new(path).map_err(failed(path))?;
        return file.write_all(contents).map_err(failed(path));
    };
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempfile_in(directory)
        .map_err(failed(path))?;
    temporary.write_all(contents).map_err(failed(path))?;
    fs::set_permissions(temporary.path(), permissions).map_err(failed(path))?;
    temporary
        .persist(path)
        .map(drop)
        .map_err(|error| failed(path)(error.error))
}

pub fn write_text(path: &Path, text: &str) -> Result<(), StoreError> {
    write_atomic(path, platform_newlines(text).as_bytes())
}

pub fn write_text_if_changed(path: &Path, text: &str) -> Result<bool, StoreError> {
    let contents = platform_newlines(text);
    match fs::read(path) {
        Ok(current) if current == contents.as_bytes() => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(StoreError::Read {
                path: path.to_path_buf(),
                source: error,
            });
        }
    }
    write_atomic(path, contents.as_bytes())?;
    Ok(true)
}
