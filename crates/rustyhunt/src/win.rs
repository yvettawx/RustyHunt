//! Odds and ends of dealing with Windows: window style and position, the blur backing window,
//! single instance, start at login, opening folders, system light/dark mode.

use std::ffi::OsStr;
use std::path::Path;

use platform::wide;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, EnumWindows, GWL_EXSTYLE, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    HWND_TOPMOST, MB_ICONERROR, MB_OK, MessageBoxW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, WNDCLASSW, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// Overlay window: not in the taskbar, doesn't take focus.
pub fn style_overlay(hwnd: isize) {
    let h = hwnd as HWND;
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST) & !WS_EX_APPWINDOW;
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex as isize);
    }
}

/// Turns click-through on or off by toggling only WS_EX_TRANSPARENT. The window stays layered:
/// winit's mouse passthrough also drops WS_EX_LAYERED, which breaks the DirectComposition output
/// and leaves the overlay white.
pub fn set_click_through(hwnd: isize, on: bool) {
    let h = hwnd as HWND;
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32 | WS_EX_LAYERED;
        let ex = if on { ex | WS_EX_TRANSPARENT } else { ex & !WS_EX_TRANSPARENT };
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex as isize);
    }
}

/// Places the overlay over the game's client area (physical pixels).
pub fn place_overlay(hwnd: isize, (x, y, w, h): (i32, i32, i32, i32)) {
    unsafe {
        SetWindowPos(hwnd as HWND, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

pub fn window_origin(hwnd: Option<isize>) -> Option<(i32, i32)> {
    let h = hwnd? as HWND;
    let mut r: RECT = unsafe { std::mem::zeroed() };
    (unsafe { GetWindowRect(h, &mut r) } != 0).then_some((r.left, r.top))
}

/// Window rect on screen in physical pixels: (x, y, w, h).
pub fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    (unsafe { GetWindowRect(hwnd as HWND, &mut r) } != 0).then_some((r.left, r.top, r.right - r.left, r.bottom - r.top))
}

pub fn primary_screen() -> (i32, i32) {
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

pub fn hide_window(hwnd: isize) {
    unsafe { ShowWindow(hwnd as HWND, SW_HIDE) };
}

pub fn focus_window(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_SHOWNORMAL);
        SetForegroundWindow(hwnd as HWND);
    }
}

unsafe extern "system" fn blur_proc(h: HWND, msg: u32, w: usize, l: isize) -> isize {
    unsafe { DefWindowProcW(h, msg, w, l) }
}

/// Window for the blur backing: click-through, doesn't take focus, not in the taskbar.
pub fn create_blur_window() -> isize {
    let class = wide(OsStr::new("RustyHuntBacking"));
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(blur_proc),
            hInstance: inst,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let title = wide(OsStr::new("RustyHunt Backing"));
        let h = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            inst,
            std::ptr::null(),
        );
        windows_sys::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(h, 0, 255, windows_sys::Win32::UI::WindowsAndMessaging::LWA_ALPHA);
        h as isize
    }
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient: u32,
    animation: u32,
}

#[repr(C)]
struct CompositionData {
    attribute: u32,
    data: *mut std::ffi::c_void,
    size: usize,
}

type SetWca = unsafe extern "system" fn(HWND, *mut CompositionData) -> i32;

/// Lets the system blur what is behind the window. `tint` is the opacity of the black on top;
/// blur strength 12 and up uses acrylic (heavier blur), below that the plain blur.
fn set_blur(hwnd: HWND, tint: u8, strength: f32) {
    unsafe {
        let user32 = wide(OsStr::new("user32.dll"));
        let m = GetModuleHandleW(user32.as_ptr());
        let Some(f) = GetProcAddress(m, c"SetWindowCompositionAttribute".as_ptr() as *const u8) else {
            return;
        };
        let f: SetWca = std::mem::transmute(f);
        let mut accent = AccentPolicy {
            state: if strength >= 12.0 { 4 } else { 3 },
            flags: 2,
            // AABBGGRR
            gradient: (tint as u32) << 24 | 0x000a0a0a,
            animation: 0,
        };
        let mut data = CompositionData {
            attribute: 19,
            data: &mut accent as *mut _ as *mut _,
            size: size_of::<AccentPolicy>(),
        };
        f(hwnd, &mut data);
    }
}

pub fn place_blur_window(hwnd: isize, (x, y, w, h): (i32, i32, i32, i32), tint: u8, strength: f32, overlay: Option<isize>) {
    let hw = hwnd as HWND;
    set_blur(hw, tint, strength);
    unsafe {
        // Directly below the overlay
        let after = overlay.map(|o| o as HWND).unwrap_or(HWND_TOPMOST);
        SetWindowPos(hw, after, x, y, w.max(1), h.max(1), SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

/// Finds a window in this process by title (the main window's child viewport has no eframe Frame to get a handle from).
pub fn own_window(title: &str) -> Option<isize> {
    struct Ctx {
        pid: u32,
        title: String,
        found: Option<isize>,
    }
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> i32 {
        let c = unsafe { &mut *(l as *mut Ctx) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        if pid != c.pid {
            return 1;
        }
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), 256) };
        if String::from_utf16_lossy(&buf[..n.max(0) as usize]) == c.title {
            c.found = Some(h as isize);
            return 0;
        }
        1
    }
    let mut c = Ctx {
        pid: unsafe { GetCurrentProcessId() },
        title: title.to_string(),
        found: None,
    };
    unsafe { EnumWindows(Some(cb), &mut c as *mut Ctx as LPARAM) };
    c.found
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
