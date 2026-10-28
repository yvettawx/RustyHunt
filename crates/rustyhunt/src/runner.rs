//! The main thread without eframe: winit makes the main window, egui-winit turns its events into egui
//! input, the same CPU rasterizer as the overlay draws each frame, and softbuffer copies it to the
//! window (GDI on Windows). No GPU device is created anywhere in the program.
//!
//! The loop also wakes for actions (hotkeys, tray, other threads) while the main window is hidden,
//! and at least once a second, so `App::tick` always runs.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ViewportId, ViewportInfo};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use crate::app::App;
use crate::state::Shared;

/// The longest the main thread sleeps, so actions and saving never wait long
const IDLE: Duration = Duration::from_secs(1);

pub enum UserEvent {
    /// egui wants a frame of the main window at this time
    Repaint(Instant),
}

pub fn run(shared: Arc<Shared>, start_tray: bool, shot: Option<(String, PathBuf)>) -> Result<(), String> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().map_err(|e| e.to_string())?;
    let proxy = event_loop.create_proxy();
    let ctx = egui::Context::default();
    let p = proxy.clone();
    ctx.set_request_repaint_callback(move |info| {
        let _ = p.send_event(UserEvent::Repaint(Instant::now() + info.delay));
    });
    let mut runner = Runner {
        shared,
        start_tray,
        shot,
        ctx,
        _proxy: proxy,
        app: None,
        win: None,
        softbuffer: softbuffer::Context::new(event_loop.owned_display_handle()).map_err(|e| e.to_string())?,
        repaint_at: Some(Instant::now()),
        overlay: None,
        frames: 0,
        painted_at: Instant::now(),
    };
    event_loop.run_app(&mut runner).map_err(|e| e.to_string())?;
    if let Some(app) = &mut runner.app {
        app.on_exit();
    }
    if let Some(t) = runner.overlay.take() {
        let _ = t.join();
    }
    Ok(())
}

struct MainWin {
    window: Rc<Window>,
    surface: softbuffer::Surface<OwnedDisplayHandle, Rc<Window>>,
    state: egui_winit::State,
    render: EguiSoftwareRender,
    info: ViewportInfo,
    shown: bool,
}

struct Runner {
    shared: Arc<Shared>,
    start_tray: bool,
    shot: Option<(String, PathBuf)>,
    ctx: egui::Context,
    _proxy: EventLoopProxy<UserEvent>,
    app: Option<App>,
    win: Option<MainWin>,
    softbuffer: softbuffer::Context<OwnedDisplayHandle>,
    repaint_at: Option<Instant>,
    overlay: Option<std::thread::JoinHandle<()>>,
    frames: u32,
    painted_at: Instant,
}

/// The shortest time between two frames: the frame rate cap from the settings.
pub fn frame_period(shared: &Shared) -> Duration {
    Duration::from_secs_f32(1.0 / shared.settings().perf.fps_cap.clamp(30, 144) as f32)
}

impl Runner {
    fn create_window(&mut self, el: &ActiveEventLoop) -> Result<MainWin, String> {
        let builder = crate::window::builder(&self.shared).with_visible(false);
        let window = Rc::new(egui_winit::create_window(&self.ctx, el, &builder).map_err(|e| e.to_string())?);
        let surface = softbuffer::Surface::new(&self.softbuffer, window.clone()).map_err(|e| e.to_string())?;
        let state = egui_winit::State::new(self.ctx.clone(), ViewportId::ROOT, el, Some(window.scale_factor() as f32), window.theme(), None);
        let mut info = ViewportInfo::default();
        egui_winit::update_viewport_info(&mut info, &self.ctx, &window, true);
        Ok(MainWin {
            window,
            surface,
            state,
            render: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
            info,
            shown: false,
        })
    }

    /// Shows or hides the main window to match the app.
    fn sync_visibility(&mut self) {
        let (Some(app), Some(win)) = (&mut self.app, &mut self.win) else { return };
        if app.main_open != win.shown {
            win.shown = app.main_open;
            win.window.set_visible(win.shown);
            if win.shown {
                win.window.request_redraw();
            }
        }
        if std::mem::take(&mut app.raise) && win.shown {
            win.window.set_minimized(false);
            win.window.focus_window();
        }
    }

    fn paint(&mut self, el: &ActiveEventLoop) {
        let (Some(app), Some(win)) = (&mut self.app, &mut self.win) else { return };
        if !win.shown {
            return;
        }
        self.painted_at = Instant::now();
        egui_winit::update_viewport_info(&mut win.info, &self.ctx, &win.window, false);
        let mut raw = win.state.take_egui_input(&win.window);
        raw.viewports.insert(ViewportId::ROOT, win.info.clone());
        win.info.events.clear();

        let mut out = self.ctx.run_ui(raw, |ui| app.ui(ui));
        win.state
            .handle_platform_output_with_event_loop(&win.window, el, std::mem::take(&mut out.platform_output));
        let commands = out.viewport_output.remove(&ViewportId::ROOT).map(|v| v.commands).unwrap_or_default();
        let mut actions = Vec::new();
        egui_winit::process_viewport_commands(&self.ctx, &mut win.info, commands, &win.window, &mut actions);
        if !win.info.events.is_empty() {
            // A close request from the title bar: the next frame handles it
            win.window.request_redraw();
        }

        let size = win.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            // Minimized: keep the textures in step without drawing
            let ppp = out.pixels_per_point;
            let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), ppp);
            let mut none = [[0u8; 4]; 1];
            win.render.render(&mut BufferMutRef::new(&mut none, 1, 1), &prims, &out.textures_delta, ppp);
            out.textures_delta.clear();
            return;
        };
        let ppp = out.pixels_per_point;
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), ppp);
        if win.surface.resize(w, h).is_err() {
            out.textures_delta.clear();
            return;
        }
        let Ok(mut buffer) = win.surface.buffer_mut() else {
            out.textures_delta.clear();
            return;
        };
        // Only the tiles that changed are copied, unless the buffer's old contents are unknown
        let fresh = buffer.age() == 0;
        let (pw, ph) = (w.get() as usize, h.get() as usize);
        win.render.render_to_canvas(pw, ph, &prims, &out.textures_delta, ppp);
        win.render
            .copy_changed_tiles(&mut BufferMutRef::new(bytemuck::cast_slice_mut(&mut buffer), pw, ph), fresh);
        out.textures_delta.clear();
        self.frames += 1;
        crate::screenshot::main_frame(
            &self.shared,
            app,
            self.frames,
            bytemuck::cast_slice(&buffer),
            w.get() as usize,
            h.get() as usize,
        );
        if let Err(e) = buffer.present() {
            log::warn!("failed to show the main window's frame: {e}");
        }
    }
}

impl ApplicationHandler<UserEvent> for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() {
            return;
        }
        match self.create_window(el) {
            Ok(w) => self.win = Some(w),
            Err(e) => {
                log::error!("failed to create the main window: {e}");
                crate::win::error_box("RustyHunt", &e);
                el.exit();
                return;
            }
        }
        let overlay_shot = self.shot.as_ref().is_some_and(|(what, _)| !what.starts_with("main"));
        self.app = Some(App::new(&self.ctx, self.shared.clone(), self.start_tray || overlay_shot));
        if let Some((what, _)) = &self.shot
            && let Some(app) = &mut self.app
        {
            crate::screenshot::prepare(app, what);
        }
        // After the event loop exists, so the process is already DPI aware
        self.overlay = Some(crate::overlay::host::spawn(self.shared.clone()));
        // Started normally, whether or not the main window shows (autostart goes to the tray)
        crate::update::confirm_started();
        self.sync_visibility();
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Repaint(when) => self.repaint_at = Some(self.repaint_at.map_or(when, |t| t.min(when))),
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(win) = &mut self.win else { return };
        match event {
            WindowEvent::RedrawRequested => self.paint(el),
            WindowEvent::CloseRequested => {
                win.info.events.push(egui::ViewportEvent::Close);
                win.window.request_redraw();
            }
            other => {
                // Input repaints go through the frame rate cap in `about_to_wait`
                if win.state.on_window_event(&win.window, &other).repaint {
                    self.repaint_at = Some(Instant::now());
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let Some(app) = &mut self.app else { return };
        if !app.tick() {
            el.exit();
            return;
        }
        self.sync_visibility();
        let now = Instant::now();
        if let Some(t) = self.repaint_at
            && t <= now
        {
            // Not sooner than the frame rate cap after the last frame
            let next = self.painted_at + frame_period(&self.shared);
            if next <= now {
                self.repaint_at = None;
                if let Some(win) = &self.win
                    && win.shown
                {
                    win.window.request_redraw();
                }
            } else {
                self.repaint_at = Some(next);
            }
        }
        let wake = self.repaint_at.map_or(now + IDLE, |t| t.min(now + IDLE));
        el.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}
