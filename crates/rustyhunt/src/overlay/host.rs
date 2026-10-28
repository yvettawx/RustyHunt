//! The overlay thread. It draws the modules with egui, rasterizes them on the CPU and shows each
//! module in its own small click-through layered window, like HunterPie's widgets. Layout mode shows
//! one window over the whole game instead, which takes the mouse and keyboard.
//!
//! Nothing here uses the GPU, and no frame waits for the compositor: `UpdateLayeredWindow` copies the
//! pixels and returns. The thread sleeps until the reader thread or the main window wakes it, an
//! animation needs its next frame, or half a second passes (to keep the windows on top).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Rect, ViewportId};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateEventW, CreateWaitableTimerExW, INFINITE, SetWaitableTimer, TIMER_ALL_ACCESS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage};

use super::Overlay;
use crate::layered::{self, Bitmap, INPUT, Layered};
use crate::state::{Connection, Live, Shared};

/// Windows are put back on top this often, and the game window's place is checked
const TICK: Duration = Duration::from_millis(500);

/// Room around what a module painted, for anti-aliased edges
const EDGE_PX: i32 = 2;

pub fn spawn(shared: Arc<Shared>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("overlay".into())
        .spawn(move || Host::new(shared).run())
        .expect("failed to start the overlay thread")
}

struct Host {
    shared: Arc<Shared>,
    overlay: Overlay,
    ctx: egui::Context,
    render: EguiSoftwareRender,
    bitmap: Option<Bitmap>,
    windows: BTreeMap<String, Layered>,
    layout: Option<Layered>,
    wake: HANDLE,
    timer: HANDLE,
    /// When the next frame is due (an animation is running); None when the picture is still
    due: Option<Instant>,
    raised_at: Instant,
    /// When the last frame was drawn, for the frame rate cap
    framed_at: Instant,
    /// Where the overlay was drawn last (physical pixels), to notice the game window moving
    drawn_at: Option<(i32, i32, i32, i32)>,
    start: Instant,
    /// The reader's last data that a frame has shown
    data_seen: Instant,
}

impl Host {
    fn new(shared: Arc<Shared>) -> Host {
        let wake = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        let _ = shared.overlay_wake.set(wake as isize);
        let timer = unsafe { CreateWaitableTimerExW(std::ptr::null(), std::ptr::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS) };

        let ctx = egui::Context::default();
        crate::themes::install_fonts(&ctx, &shared.theme());
        ctx.set_theme(egui::Theme::Dark);
        crate::window::style::apply(&ctx, &shared.theme());
        // Repaints asked for from other threads (or by egui itself) wake this one
        let wake_handle = wake as isize;
        ctx.set_request_repaint_callback(move |info| {
            if info.delay.is_zero() {
                unsafe { windows_sys::Win32::System::Threading::SetEvent(wake_handle as HANDLE) };
            }
        });
        let _ = shared.overlay_ctx.set(ctx.clone());

        Host {
            overlay: Overlay::new(shared.clone()),
            shared,
            ctx,
            render: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
            bitmap: None,
            windows: BTreeMap::new(),
            layout: None,
            wake,
            timer,
            due: Some(Instant::now()),
            raised_at: Instant::now(),
            framed_at: Instant::now() - Duration::from_secs(1),
            drawn_at: None,
            start: Instant::now(),
            data_seen: Instant::now(),
        }
    }

    fn run(mut self) {
        loop {
            let woken = self.wait();
            if self.shared.quit.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let now = Instant::now();
            let input = INPUT.with_borrow(|i| !i.events.is_empty());
            let due = self.due.is_some_and(|t| t <= now);
            let moved = self.drawn_at.is_some() && self.drawn_at != self.target(&self.shared.live(), self.shared.layout_mode());
            if woken || due || input || moved {
                // Not sooner than the frame rate cap after the last frame
                let next = self.framed_at + crate::runner::frame_period(&self.shared);
                if next <= now {
                    self.framed_at = now;
                    self.frame();
                } else {
                    self.due = Some(self.due.map_or(next, |t| t.max(next)));
                }
            }
            if self.raised_at.elapsed() >= TICK {
                self.raised_at = Instant::now();
                for w in self.windows.values() {
                    w.raise();
                }
                if let Some(l) = &self.layout {
                    l.raise();
                }
            }
        }
        // Windows go with the thread
        self.windows.clear();
        self.layout = None;
    }

    /// Sleeps until woken, the next frame is due, a window message arrives, or a tick passes.
    /// Returns whether something asked for a frame.
    fn wait(&mut self) -> bool {
        let now = Instant::now();
        let until = self.due.map_or(now + TICK, |t| t.min(now + TICK));
        let rel = -((until.saturating_duration_since(now).as_nanos() / 100) as i64).max(1);
        unsafe { SetWaitableTimer(self.timer, &rel, 0, None, std::ptr::null(), 0) };
        let handles = [self.wake, self.timer];
        let r = unsafe { MsgWaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE, QS_ALLINPUT) };
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        r == WAIT_OBJECT_0
    }

    /// Which part of the screen the overlay covers (physical pixels). None when there is nothing to show.
    fn target(&self, live: &Live, layout: bool) -> Option<(i32, i32, i32, i32)> {
        if let Some(r) = live.game_rect
            && live.connected()
            && !live.game_minimized
            && r.w > 0
            && r.h > 0
        {
            return Some((r.x, r.y, r.w, r.h));
        }
        if matches!(live.connection, Connection::Demo) || layout {
            let (w, h) = crate::win::primary_screen();
            return Some((0, 0, w, h));
        }
        None
    }

    fn hide_all(&mut self) {
        for w in self.windows.values_mut() {
            w.hide();
        }
        if let Some(l) = &mut self.layout {
            l.hide();
        }
        self.drawn_at = None;
        self.due = None;
    }

    fn frame(&mut self) {
        let t0 = Instant::now();
        let layout_on = self.shared.layout_mode();
        let live = self.shared.live().clone();
        let overlay_visible = self.shared.settings().overlay_visible;
        let target = self.target(&live, layout_on);
        // The windows are topmost: show nothing over other programs while the game is in the background
        let in_front = live.game_foreground || layout_on || matches!(live.connection, Connection::Demo);
        let Some((x, y, w, h)) = target.filter(|_| (overlay_visible || layout_on) && in_front) else {
            self.hide_all();
            return;
        };
        let scale = layered::scale_at((x, y, w, h));
        let fresh = self.bitmap.as_ref().is_none_or(|b| (b.w, b.h) != (w as usize, h as usize));
        if fresh {
            self.bitmap = Bitmap::new(w as usize, h as usize);
        }

        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32) / scale)),
            time: Some(self.start.elapsed().as_secs_f64()),
            ..Default::default()
        };
        raw.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        INPUT.with_borrow_mut(|i| {
            i.scale = scale;
            if layout_on {
                raw.events = std::iter::once(egui::Event::ModifiersChanged(layered::modifiers()))
                    .chain(std::mem::take(&mut i.events))
                    .collect();
                raw.focused = i.focused;
            } else {
                i.events.clear();
            }
        });

        let data_at = *self.shared.data_at.lock().unwrap();
        let mut out = self.ctx.run_ui(raw, |ui| self.overlay.paint(ui));
        self.overlay.end_frame();
        let ppp = out.pixels_per_point;
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), ppp);
        let t_ui = Instant::now();
        let Some(bitmap) = &mut self.bitmap else {
            out.textures_delta.clear();
            return;
        };
        // The renderer keeps the whole picture in 64x64 tiles; only the tiles that changed are copied to
        // the bitmap, and only windows over those tiles are updated
        self.render.render_to_canvas(w as usize, h as usize, &prims, &out.textures_delta, ppp);
        let changed = self
            .render
            .copy_changed_tiles(&mut BufferMutRef::new(bitmap.pixels(), w as usize, h as usize), fresh);
        // Applied by the renderer above (egui checks that deltas were handled)
        out.textures_delta.clear();
        let t_raster = Instant::now();
        crate::screenshot::overlay_frame(&self.shared, bitmap, &self.overlay.state.rects);

        if layout_on {
            for win in self.windows.values_mut() {
                win.hide();
            }
            if self.layout.is_none() {
                self.layout = Layered::new("RustyHunt Layout", true);
            }
            if let Some(l) = &mut self.layout {
                let first = !l.shown();
                if first || !changed.is_empty() || l.placed() != Some(((x, y), (0, 0), (w, h))) {
                    l.update(bitmap, (x, y), (0, 0), (w, h));
                }
                if first {
                    l.focus();
                }
            }
            layered::set_cursor(out.platform_output.cursor_icon);
        } else {
            if let Some(l) = &mut self.layout {
                l.hide();
            }
            self.show_modules(x, y, w, h, scale, &changed);
        }
        self.drawn_at = Some((x, y, w, h));
        let t_end = Instant::now();

        // Counted from the start of this frame, so animations keep their pace
        let delay = out.viewport_output.get(&ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        self.due = (delay < Duration::from_secs(3600)).then(|| t0 + delay);

        let ms = |d: Duration| d.as_secs_f32() * 1000.0;
        let mut split = self.shared.overlay_split.lock().unwrap();
        let fresh_data = data_at.filter(|&t| t != self.data_seen);
        let parts = [
            ms(t_ui - t0),
            ms(t_raster - t_ui),
            ms(t_end - t_raster),
            fresh_data.map_or(split[3], |t| ms(t_end - t)),
        ];
        for (avg, v) in split.iter_mut().zip(parts) {
            *avg = if *avg == 0.0 { v } else { *avg * 0.9 + v * 0.1 };
        }
        drop(split);
        if let Some(t) = fresh_data {
            self.data_seen = t;
        }

        self.shared.overlay_frames.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let us = t0.elapsed().as_micros() as u32;
        let prev = self.shared.frame_us.load(std::sync::atomic::Ordering::Relaxed);
        self.shared
            .frame_us
            .store(if prev == 0 { us } else { (prev * 7 + us) / 8 }, std::sync::atomic::Ordering::Relaxed);
    }

    /// One window per module that painted something, sized to what it painted. A window is updated when
    /// it moved, was hidden, or one of the `changed` tiles (pixel rects) lies under it.
    fn show_modules(&mut self, x: i32, y: i32, w: i32, h: i32, scale: f32, changed: &[[usize; 4]]) {
        let Some(bitmap) = &self.bitmap else { return };
        for (id, b) in &self.overlay.state.bounds {
            if !b.is_positive() {
                continue;
            }
            let x0 = ((b.min.x * scale).floor() as i32 - EDGE_PX).clamp(0, w);
            let y0 = ((b.min.y * scale).floor() as i32 - EDGE_PX).clamp(0, h);
            let x1 = ((b.max.x * scale).ceil() as i32 + EDGE_PX).clamp(0, w);
            let y1 = ((b.max.y * scale).ceil() as i32 + EDGE_PX).clamp(0, h);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            if !self.windows.contains_key(id) {
                let Some(win) = Layered::new(&format!("RustyHunt {id}"), false) else {
                    continue;
                };
                self.windows.insert(id.clone(), win);
            }
            let Some(win) = self.windows.get_mut(id) else { continue };
            let place = ((x + x0, y + y0), (x0, y0), (x1 - x0, y1 - y0));
            let touched = changed
                .iter()
                .any(|c| (c[0] as i32) < x1 && (c[2] as i32) > x0 && (c[1] as i32) < y1 && (c[3] as i32) > y0);
            if touched || !win.shown() || win.placed() != Some(place) {
                win.update(bitmap, place.0, place.1, place.2);
            }
        }
        let drawn = &self.overlay.state.bounds;
        for (id, win) in self.windows.iter_mut() {
            if !drawn.get(id).is_some_and(|b| b.is_positive()) {
                win.hide();
            }
        }
    }
}
