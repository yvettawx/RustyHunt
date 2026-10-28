//! Layered windows for the overlay. Each window shows part of one premultiplied BGRA bitmap through
//! `UpdateLayeredWindow`: the system copies the pixels during the call, so nothing waits on the
//! compositor and no GPU device is involved. Module windows are click-through; the layout window
//! takes the mouse and keyboard, which arrive here as egui events.

use std::cell::RefCell;
use std::ffi::OsStr;

use egui::{CursorIcon, Event, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, TouchPhase, pos2, vec2};
use platform::wide;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MonitorFromRect, SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE,
    VK_HOME, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTCLIENT, HWND_TOPMOST, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL,
    IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, LoadCursorW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetCursor, SetForegroundWindow, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// A top-down 32-bit bitmap in a memory DC: the CPU draws into it, `UpdateLayeredWindow` reads from it.
pub struct Bitmap {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut [u8; 4],
    pub w: usize,
    pub h: usize,
}

impl Bitmap {
    pub fn new(w: usize, h: usize) -> Option<Bitmap> {
        let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            // Negative: rows from top to bottom
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..unsafe { std::mem::zeroed() }
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        unsafe {
            let bmp = CreateDIBSection(std::ptr::null_mut(), &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bmp.is_null() || bits.is_null() {
                return None;
            }
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let old = SelectObject(dc, bmp);
            Some(Bitmap {
                dc,
                bmp,
                old,
                bits: bits as *mut [u8; 4],
                w,
                h,
            })
        }
    }

    /// The pixels, BGRA with premultiplied alpha, row by row.
    pub fn pixels(&mut self) -> &mut [[u8; 4]] {
        // GDI may still be reading the bitmap from the last update
        unsafe { GdiFlush() };
        unsafe { std::slice::from_raw_parts_mut(self.bits, self.w * self.h) }
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}

/// Sent after `TrackMouseEvent` when the mouse leaves the window
const WM_MOUSELEAVE: u32 = 0x02A3;

const CLASS_PLAIN: &str = "RustyHuntOverlay";
const CLASS_INPUT: &str = "RustyHuntLayout";

fn register_classes() {
    thread_local! {
        static DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if DONE.get() {
        return;
    }
    DONE.set(true);
    let plain = wide(OsStr::new(CLASS_PLAIN));
    let input = wide(OsStr::new(CLASS_INPUT));
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let arrow = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.hInstance = inst;
        wc.hCursor = arrow;
        wc.lpfnWndProc = Some(plain_proc);
        wc.lpszClassName = plain.as_ptr();
        RegisterClassW(&wc);
        wc.lpfnWndProc = Some(input_proc);
        wc.lpszClassName = input.as_ptr();
        RegisterClassW(&wc);
    }
}

unsafe extern "system" fn plain_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, msg, w, l) }
}

/// One layered, topmost, tool window (not in the taskbar, never activated by a click).
pub struct Layered {
    hwnd: HWND,
    shown: bool,
    /// Last update: screen position, source position, size
    placed: Option<Place>,
}

type Place = ((i32, i32), (i32, i32), (i32, i32));

impl Layered {
    /// `input`: the window takes the mouse and keyboard (layout mode); otherwise clicks go through it.
    pub fn new(title: &str, input: bool) -> Option<Layered> {
        register_classes();
        let class = wide(OsStr::new(if input { CLASS_INPUT } else { CLASS_PLAIN }));
        let title = wide(OsStr::new(title));
        let mut ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
        if !input {
            ex |= WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        (!hwnd.is_null()).then_some(Layered {
            hwnd,
            shown: false,
            placed: None,
        })
    }

    /// Shows `src` (top left in the bitmap) of size `size` at `at` on screen, physical pixels.
    pub fn update(&mut self, bmp: &Bitmap, at: (i32, i32), src: (i32, i32), size: (i32, i32)) {
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let pos = POINT { x: at.0, y: at.1 };
        let sz = SIZE { cx: size.0, cy: size.1 };
        let from = POINT { x: src.0, y: src.1 };
        let ok = unsafe { UpdateLayeredWindow(self.hwnd, std::ptr::null_mut(), &pos, &sz, bmp.dc, &from, 0, &blend, ULW_ALPHA) };
        if ok == 0 {
            log::debug!("UpdateLayeredWindow failed: {}", std::io::Error::last_os_error());
            return;
        }
        self.placed = Some((at, src, size));
        if !self.shown {
            self.shown = true;
            unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
        }
    }

    pub fn hide(&mut self) {
        if self.shown {
            self.shown = false;
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
    }

    pub fn shown(&self) -> bool {
        self.shown
    }

    pub fn placed(&self) -> Option<Place> {
        self.placed
    }

    /// Back on top of other topmost windows (the game or other tools may have taken the spot).
    pub fn raise(&self) {
        if self.shown {
            unsafe { SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }
    }

    /// Keyboard focus, for the layout window.
    pub fn focus(&self) {
        unsafe {
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
        }
    }
}

impl Drop for Layered {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Scale of the monitor showing most of the rect (physical pixels), 1.0 at 96 dpi.
pub fn scale_at((x, y, w, h): (i32, i32, i32, i32)) -> f32 {
    let r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let (mut dx, mut dy) = (96u32, 96u32);
    unsafe {
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    }
    dx.max(48) as f32 / 96.0
}

/// Input that reached the layout window since the last frame, in egui terms.
#[derive(Default)]
pub struct Input {
    pub events: Vec<Event>,
    pub focused: bool,
    /// Physical pixels per egui point, to turn mouse positions into points
    pub scale: f32,
    pub cursor: CursorIcon,
    tracking: bool,
}

thread_local! {
    pub static INPUT: RefCell<Input> = RefCell::new(Input { scale: 1.0, ..Default::default() });
}

pub fn modifiers() -> Modifiers {
    let down = |vk: u16| unsafe { GetKeyState(vk as i32) } < 0;
    let ctrl = down(VK_CONTROL);
    Modifiers {
        alt: down(VK_MENU),
        ctrl,
        shift: down(VK_SHIFT),
        mac_cmd: false,
        command: ctrl,
    }
}

fn key_of(vk: u16) -> Option<Key> {
    Some(match vk {
        VK_ESCAPE => Key::Escape,
        VK_RETURN => Key::Enter,
        VK_SPACE => Key::Space,
        VK_TAB => Key::Tab,
        VK_BACK => Key::Backspace,
        VK_DELETE => Key::Delete,
        VK_LEFT => Key::ArrowLeft,
        VK_RIGHT => Key::ArrowRight,
        VK_UP => Key::ArrowUp,
        VK_DOWN => Key::ArrowDown,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        0x30..=0x39 | 0x41..=0x5A => return Key::from_name(&char::from(vk as u8).to_string()),
        _ => return None,
    })
}

fn cursor_id(c: CursorIcon) -> Option<windows_sys::core::PCWSTR> {
    Some(match c {
        CursorIcon::None => return None,
        CursorIcon::PointingHand | CursorIcon::Grab | CursorIcon::Grabbing => IDC_HAND,
        CursorIcon::Move | CursorIcon::AllScroll => IDC_SIZEALL,
        CursorIcon::ResizeNwSe | CursorIcon::ResizeNorthWest | CursorIcon::ResizeSouthEast => IDC_SIZENWSE,
        CursorIcon::ResizeNeSw | CursorIcon::ResizeNorthEast | CursorIcon::ResizeSouthWest => IDC_SIZENESW,
        CursorIcon::ResizeHorizontal | CursorIcon::ResizeEast | CursorIcon::ResizeWest | CursorIcon::ResizeColumn => IDC_SIZEWE,
        CursorIcon::ResizeVertical | CursorIcon::ResizeNorth | CursorIcon::ResizeSouth | CursorIcon::ResizeRow => IDC_SIZENS,
        CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
        CursorIcon::Wait | CursorIcon::Progress => IDC_WAIT,
        CursorIcon::Help => IDC_HELP,
        CursorIcon::Crosshair | CursorIcon::Cell => IDC_CROSS,
        _ => IDC_ARROW,
    })
}

/// Sets the mouse cursor over the layout window to what egui asked for.
pub fn set_cursor(c: CursorIcon) {
    INPUT.with_borrow_mut(|i| i.cursor = c);
    apply_cursor(c);
}

fn apply_cursor(c: CursorIcon) {
    unsafe {
        let h = cursor_id(c).map_or(std::ptr::null_mut(), |id| LoadCursorW(std::ptr::null_mut(), id));
        SetCursor(h);
    }
}

unsafe extern "system" fn input_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let pos = |i: &Input| {
        let x = (l & 0xFFFF) as i16 as f32;
        let y = ((l >> 16) & 0xFFFF) as i16 as f32;
        pos2(x / i.scale, y / i.scale)
    };
    let button = |b: PointerButton, pressed: bool| {
        INPUT.with_borrow_mut(|i| {
            let p = pos(i);
            i.events.push(Event::PointerButton {
                pos: p,
                button: b,
                pressed,
                modifiers: modifiers(),
            });
        });
        unsafe {
            if pressed {
                SetCapture(h);
            } else {
                ReleaseCapture();
            }
        }
        0
    };
    match msg {
        WM_MOUSEMOVE => {
            INPUT.with_borrow_mut(|i| {
                if !i.tracking {
                    i.tracking = true;
                    let mut t = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: h,
                        dwHoverTime: 0,
                    };
                    unsafe { TrackMouseEvent(&mut t) };
                }
                let p = pos(i);
                i.events.push(Event::PointerMoved(p));
            });
            0
        }
        WM_MOUSELEAVE => {
            INPUT.with_borrow_mut(|i| {
                i.tracking = false;
                i.events.push(Event::PointerGone);
            });
            0
        }
        WM_LBUTTONDOWN => button(PointerButton::Primary, true),
        WM_LBUTTONUP => button(PointerButton::Primary, false),
        WM_RBUTTONDOWN => button(PointerButton::Secondary, true),
        WM_RBUTTONUP => button(PointerButton::Secondary, false),
        WM_MBUTTONDOWN => button(PointerButton::Middle, true),
        WM_MBUTTONUP => button(PointerButton::Middle, false),
        WM_MOUSEWHEEL => {
            let notches = ((w >> 16) & 0xFFFF) as i16 as f32 / 120.0;
            INPUT.with_borrow_mut(|i| {
                i.events.push(Event::MouseWheel {
                    unit: MouseWheelUnit::Line,
                    delta: vec2(0.0, notches),
                    phase: TouchPhase::Move,
                    source: MouseWheelSource::Wheel,
                    modifiers: modifiers(),
                })
            });
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
            let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if let Some(key) = key_of(w as u16) {
                INPUT.with_borrow_mut(|i| {
                    i.events.push(Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed,
                        repeat: false,
                        modifiers: modifiers(),
                    })
                });
            }
            unsafe { DefWindowProcW(h, msg, w, l) }
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(w as u32).filter(|c| !c.is_control()) {
                INPUT.with_borrow_mut(|i| i.events.push(Event::Text(c.to_string())));
            }
            0
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let on = msg == WM_SETFOCUS;
            INPUT.with_borrow_mut(|i| {
                i.focused = on;
                i.events.push(Event::WindowFocused(on));
            });
            0
        }
        WM_SETCURSOR if (l & 0xFFFF) as u32 == HTCLIENT => {
            apply_cursor(INPUT.with_borrow(|i| i.cursor));
            1
        }
        _ => unsafe { DefWindowProcW(h, msg, w, l) },
    }
}
