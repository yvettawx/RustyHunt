//! 程序的根：eframe 的根窗口就是 overlay（一直存在，透明、穿透），
//! 主界面是另一个窗口，关掉后缩到托盘，需要时再打开。

use std::sync::{Arc, Mutex};
use std::time::Instant;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rh_core::model::Game;
use rh_core::settings::TargetMode;

use crate::overlay::Overlay;
use crate::state::{Action, Shared};
use crate::window::{self, MainWindow};

pub struct App {
    shared: Arc<Shared>,
    overlay: Overlay,
    main: Arc<Mutex<MainWindow>>,
    main_open: bool,
    last_save: Instant,
    started_tray: bool,
    _tray: Option<crate::tray::Tray>,
}

impl App {
    /// 截图用：主窗口切到某一页
    pub fn set_page(&self, page: crate::window::Page, tab: usize) {
        let mut m = self.main.lock().unwrap();
        m.page = page;
        m.set_tab(tab);
    }

    pub fn new(cc: &eframe::CreationContext, shared: Arc<Shared>, start_in_tray: bool) -> App {
        let ctx = cc.egui_ctx.clone();
        let _ = shared.ctx.set(ctx.clone());
        crate::themes::install_fonts(&ctx, &shared.theme());
        ctx.set_theme(egui::Theme::Dark);
        crate::window::style::apply(&ctx, &shared.theme());

        let mut overlay = Overlay::new(shared.clone());
        if let Ok(h) = cc.window_handle()
            && let RawWindowHandle::Win32(w) = h.as_raw()
        {
            overlay.set_hwnd(w.hwnd.get());
        }
        let tray = crate::tray::Tray::new(shared.clone());
        App {
            main: Arc::new(Mutex::new(MainWindow::new(shared.clone()))),
            shared,
            overlay,
            main_open: !start_in_tray,
            last_save: Instant::now(),
            started_tray: start_in_tray,
            _tray: tray,
        }
    }

    fn handle_actions(&mut self, ctx: &egui::Context) {
        for a in self.shared.take_actions() {
            match a {
                Action::ToggleOverlay => self.shared.edit_settings(|s| s.overlay_visible = !s.overlay_visible),
                Action::ToggleLayout => {
                    let on = !self.shared.layout_mode();
                    self.set_layout(on);
                }
                Action::SetLayout(on) => self.set_layout(on),
                Action::CycleDensity => self.shared.edit_settings(|s| s.density = s.density.next()),
                Action::CycleTarget => {
                    let snap = self.shared.live().snapshot.clone();
                    let mode = self.shared.settings().profile(Game::World).monster.target_mode;
                    if mode == TargetMode::Manual {
                        let mut live = self.shared.live.write().unwrap();
                        live.target = crate::scanner::next_target(&snap, live.target);
                    }
                }
                Action::ShowMain => {
                    self.main_open = true;
                    if let Some(h) = crate::win::own_window(window::TITLE) {
                        crate::win::focus_window(h);
                    }
                }
                Action::Quit => {
                    self.shared.quit.store(true, std::sync::atomic::Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::Plugin { plugin, action } => crate::plugins::run_action(&self.shared, &plugin, &action),
            }
        }
    }

    fn set_layout(&mut self, on: bool) {
        let was = self.shared.layout_mode.swap(on, std::sync::atomic::Ordering::Relaxed);
        if was && !on {
            // 退出布局模式时保存
            self.shared.save_settings();
            log::info!("布局已保存");
        }
        self.shared.repaint();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.started_tray {
            self.started_tray = false;
        }
        self.handle_actions(&ctx);

        if self.shared.take_dirty() || self.last_save.elapsed().as_secs() > 30 {
            self.last_save = Instant::now();
            self.shared.save_settings();
        }

        self.overlay.ui(ui);
        self.overlay.end_frame();

        if self.main_open {
            let main = self.main.clone();
            let shared = self.shared.clone();
            ctx.show_viewport_deferred(window::main_viewport(), window::builder(&self.shared), move |ui, _class| {
                let mut m = main.lock().unwrap();
                m.ui(ui);
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    m.on_close(&shared);
                }
            });
            if self.main.lock().unwrap().closed {
                self.main.lock().unwrap().closed = false;
                self.main_open = false;
            }
        }
        if self.shared.quit.load(std::sync::atomic::Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.quit.store(true, std::sync::atomic::Ordering::Relaxed);
        self.shared.save_settings();
        self.shared.notifications.lock().unwrap().save();
        log::info!("退出");
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
