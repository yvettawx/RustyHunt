//! Windows-specific features: processes, memory, windows, keys, Steam.

pub mod keys;
pub mod memory;
pub mod steam;

#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod window;

pub use memory::{FakeMemory, Memory, ReadError, ReadResult};

/// Converts to null-terminated UTF-16 for the Win32 `W` functions.
pub fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        s.encode_wide().chain(std::iter::once(0)).collect()
    }
    #[cfg(not(windows))]
    {
        s.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect()
    }
}
