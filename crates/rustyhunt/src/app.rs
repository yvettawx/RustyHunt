//! Program root on the main thread: actions from hotkeys, the tray and the windows, saving settings,
//! and the main window's UI. The main window goes to the tray when closed and reopens on demand;
//! the overlay runs on its own thread (`overlay::host`).

use std::sync::Arc;
use std::time::Instant;

use rh_core::model::Game;
use rh_core::settings::TargetMode;

use crate::state::{Action, Shared};
use crate::window::MainWindow;

pub struct App {
    shared: Arc<Shared>,
    main: MainWindow,
    /// The main window should be on screen
    pub main_open: bool,
    /// Bring the main window to the front on the next tick
    pub raise: bool,
    last_save: Instant,
    _tray: Option<crate::tray::Tray>,
}

impl App {
    /// For screenshots: switch the main window to a page
    pub fn open_onboarding(&mut self, step: usize) {
        self.main.open_onboarding();
        self.main.onboarding_step(step);
    }

    pub fn set_page(&mut self, page: crate::window::Page, tab: usize) {
        self.main.page = page;
        self.main.set_tab(tab);
    }

    pub fn new(ctx: &egui::Context, shared: Arc<Shared>, start_in_tray: bool) -> App {
        let _ = shared.ctx.set(ctx.clone());
        crate::themes::install_fonts(ctx, &shared.theme());
        ctx.set_theme(egui::Theme::Dark);
        crate::window::style::apply(ctx, &shared.theme());

        let tray = crate::tray::Tray::new(shared.clone());
        App {
            main: MainWindow::new(shared.clone()),
            shared,
            main_open: !start_in_tray,
            raise: false,
            last_save: Instant::now(),
            _tray: tray,
        }
    }

    /// Handles queued actions and saves settings now and then. Runs whenever the main thread wakes,
    /// whether or not the main window is shown. Returns false when the program should quit.
    pub fn tick(&mut self) -> bool {
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
                    self.raise = true;
                }
                Action::Quit => self.shared.quit.store(true, std::sync::atomic::Ordering::Relaxed),
                Action::Plugin { plugin, action } => crate::plugins::run_action(&self.shared, &plugin, &action),
            }
        }
        if self.shared.take_dirty() || self.last_save.elapsed().as_secs() > 30 {
            self.last_save = Instant::now();
            self.shared.save_settings();
        }
        !self.shared.quit.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn set_layout(&mut self, on: bool) {
        let was = self.shared.layout_mode.swap(on, std::sync::atomic::Ordering::Relaxed);
        if was && !on {
            // Save when leaving layout mode
            self.shared.save_settings();
            log::info!("layout saved");
        }
        self.shared.repaint();
    }

    /// The main window's frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.main.ui(ui);
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            self.main.on_close(&self.shared);
        }
        if self.main.closed {
            self.main.closed = false;
            self.main_open = false;
        }
    }

    pub fn on_exit(&mut self) {
        self.shared.quit.store(true, std::sync::atomic::Ordering::Relaxed);
        self.shared.repaint_overlay();
        self.shared.save_settings();
        self.shared.notifications.lock().unwrap().save();
        log::info!("exit");
    }
}
