//! Windows 平台相关的功能：进程、内存、窗口、按键、Steam。

pub mod keys;
pub mod memory;
pub mod steam;

#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod window;

pub use memory::{FakeMemory, Memory, ReadError, ReadResult};

/// 转成以 0 结尾的 UTF-16，给 Win32 的 `W` 系列函数用。
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
