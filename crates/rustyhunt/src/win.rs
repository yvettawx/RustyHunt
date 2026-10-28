//! Odds and ends of dealing with Windows: the screen size,
//! single instance, start at login, opening folders, system light/dark mode.

use std::ffi::OsStr;
use std::path::Path;

use platform::wide;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, MB_ICONERROR, MB_OK, MessageBoxW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
};

pub fn primary_screen() -> (i32, i32) {
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

pub fn focus_window(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd as HWND);
    }
}

/// Single instance: returns false when one is already running.
pub fn single_instance() -> bool {
    let name = wide(OsStr::new("Local\\RustyHunt.SingleInstance"));
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        !h.is_null() && windows_sys::Win32::Foundation::GetLastError() != windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
    }
}

pub fn open(path: &Path) {
    let op = wide(OsStr::new("open"));
    let p = wide(path.as_os_str());
    unsafe {
        ShellExecuteW(std::ptr::null_mut(), op.as_ptr(), p.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
    }
}

pub fn error_box(title: &str, text: &str) {
    let t = wide(OsStr::new(title));
    let b = wide(OsStr::new(text));
    unsafe { MessageBoxW(std::ptr::null_mut(), b.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR) };
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn open_run_key() -> Option<HKEY> {
    let key = wide(OsStr::new(RUN_KEY));
    let mut h: HKEY = std::ptr::null_mut();
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut h) };
    (rc == 0).then_some(h)
}

/// Start at login: written under the current user's Run key, no admin rights needed.
pub fn set_autostart(on: bool) {
    let Some(h) = open_run_key() else { return };
    let name = wide(OsStr::new("RustyHunt"));
    unsafe {
        if on {
            let exe = std::env::current_exe().unwrap_or_default();
            let v = wide(OsStr::new(&format!("\"{}\" --tray", exe.display())));
            RegSetValueExW(h, name.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32);
        } else {
            RegDeleteValueW(h, name.as_ptr());
        }
        RegCloseKey(h);
    }
}

/// Whether the system is in dark mode.
pub fn system_dark() -> bool {
    let key = wide(OsStr::new("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"));
    let val = wide(OsStr::new("AppsUseLightTheme"));
    let mut data: u32 = 1;
    let mut size = 4u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            val.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    rc == 0 && data == 0
}

/// Finds a top-level window by title (any process).
pub fn own_window_any(title: &str) -> Option<isize> {
    let t = wide(OsStr::new(title));
    let h = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(std::ptr::null(), t.as_ptr()) };
    (!h.is_null()).then_some(h as isize)
}
