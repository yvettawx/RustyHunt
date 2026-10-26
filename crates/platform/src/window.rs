//! 游戏窗口：标题、客户区位置、前台状态。

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetClientRect, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 一个顶层窗口的句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window(pub isize);

impl Window {
    fn hwnd(self) -> HWND {
        self.0 as HWND
    }

    pub fn is_valid(self) -> bool {
        unsafe { IsWindow(self.hwnd()) != 0 }
    }

    pub fn title(self) -> String {
        let len = unsafe { GetWindowTextLengthW(self.hwnd()) };
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = unsafe { GetWindowTextW(self.hwnd(), buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    pub fn pid(self) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(self.hwnd(), &mut pid) };
        pid
    }

    /// 客户区在屏幕上的位置，物理像素。
    pub fn client_rect(self) -> Option<Rect> {
        let mut r: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetClientRect(self.hwnd(), &mut r) } == 0 {
            return None;
        }
        let mut p = POINT { x: 0, y: 0 };
        if unsafe { ClientToScreen(self.hwnd(), &mut p) } == 0 {
            return None;
        }
        Some(Rect {
            x: p.x,
            y: p.y,
            w: r.right - r.left,
            h: r.bottom - r.top,
        })
    }

    pub fn is_foreground(self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd() }
    }

    pub fn is_minimized(self) -> bool {
        unsafe { IsIconic(self.hwnd()) != 0 }
    }

    /// 窗口所在显示器的缩放比例，100% 为 1.0。
    pub fn dpi_scale(self) -> f32 {
        let dpi = unsafe { GetDpiForWindow(self.hwnd()) };
        if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
    }
}

pub fn foreground() -> Option<Window> {
    let h = unsafe { GetForegroundWindow() };
    (!h.is_null()).then_some(Window(h as isize))
}

/// 进程的主窗口：可见、没有所有者、标题不为空的第一个顶层窗口。
pub fn main_window(pid: u32) -> Option<Window> {
    struct Ctx {
        pid: u32,
        found: Option<Window>,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        let ctx = unsafe { &mut *(lparam as *mut Ctx) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == ctx.pid
            && unsafe { IsWindowVisible(hwnd) } != 0
            && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null()
            && unsafe { GetWindowTextLengthW(hwnd) } > 0
        {
            ctx.found = Some(Window(hwnd as isize));
            return 0;
        }
        1
    }
    let mut ctx = Ctx { pid, found: None };
    unsafe { EnumWindows(Some(cb), &mut ctx as *mut Ctx as LPARAM) };
    ctx.found
}
