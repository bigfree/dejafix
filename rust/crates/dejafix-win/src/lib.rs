#![cfg(windows)]

use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

#[must_use]
pub fn is_fixed_drive(letter: char) -> bool {
    let Ok(letter) = u16::try_from(u32::from(letter)) else {
        return false;
    };
    let root = [letter, u16::from(b':'), u16::from(b'\\'), 0];
    #[expect(
        unsafe_code,
        reason = "GetDriveTypeW only reads the NUL-terminated UTF-16 root, which lives for the whole call"
    )]
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    kind == DRIVE_FIXED
}

#[cfg(test)]
mod tests;
