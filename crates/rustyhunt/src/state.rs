//! State shared by the reader thread, the overlay and the main window.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

use rh_core::Event;
use rh_core::model::Snapshot;
use rh_core::settings::Settings;
use ui_kit::Theme;

use crate::notify::Notifications;

#[derive(Debug, Clone, PartialEq)]
pub enum Connection {
    /// Looking for the game process
    Searching,
    Connected,
    /// No address map for this version; reading stopped
    MapMismatch {
        version: String,
        latest: Option<String>,
    },
    /// Can't open the process (not enough rights, for example)
    Failed(String),
    /// No game; using demo data
    Demo,
}

#[derive(Debug, Clone)]
pub struct Live {
    pub snapshot: Arc<Snapshot>,
    pub connection: Connection,
    pub pid: Option<u32>,
    pub exe: String,
    pub version: Option<String>,
    pub attached_at: Option<Instant>,
    /// The game's client area on screen, in physical pixels
    pub game_rect: Option<platform::window::Rect>,
    pub game_foreground: bool,
    pub game_minimized: bool,
    /// Time spent reading memory last frame
    pub read_ms: f32,
    pub actual_hz: f32,
    /// Our DPS every 2 seconds, for the curve
    pub dps_history: Vec<f32>,
    /// Fight time this quest, which DPS is measured over
    pub combat: rh_core::combat::CombatClock,
    /// Current target monster
    pub target: Option<u64>,
    /// Damage of the latest attack chain (on the target monster)
    pub combo_damage: f32,
    pub combo_at: f64,
    /// Count of read errors, summarized by count in the log
    pub errors: u64,
}

impl Default for Live {
    fn default() -> Self {
        Live {
            snapshot: Arc::new(Snapshot::default()),
            connection: Connection::Searching,
            pid: None,
            exe: String::new(),
            version: None,
            attached_at: None,
            game_rect: None,
            game_foreground: false,
            game_minimized: false,
            read_ms: 0.0,
            actual_hz: 0.0,
            dps_history: Vec::new(),
            combat: Default::default(),
            target: None,
            combo_damage: 0.0,
            combo_at: -100.0,
            errors: 0,
        }
    }
}

impl Live {
    pub fn connected(&self) -> bool {
        matches!(self.connection, Connection::Connected)
    }

    /// There is data to show: connected to the game, or in demo mode
    pub fn has_data(&self) -> bool {
        matches!(self.connection, Connection::Connected | Connection::Demo)
    }
}

/// Actions sent by hotkeys, the tray and the main window.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    ToggleOverlay,
    ToggleLayout,
    SetLayout(bool),
    CycleDensity,
    CycleTarget,
    ShowMain,
    Quit,
    Plugin { plugin: String, action: String },
}

/// Recent events: the overlay uses them for callouts, the main window for the timeline.
#[derive(Default)]
pub struct EventLog {
    pub items: VecDeque<(f64, Event)>,
}

impl EventLog {
    pub fn push(&mut self, t: f64, e: Event) {
        if self.items.len() > 200 {
            self.items.pop_front();
        }
        self.items.push_back((t, e));
    }
}

pub struct Shared {
    pub settings: RwLock<Settings>,
    settings_dirty: AtomicBool,
    pub live: RwLock<Live>,
    pub events: Mutex<EventLog>,
    pub notifications: Mutex<Notifications>,
    pub theme: RwLock<Arc<Theme>>,
    pub actions: Mutex<VecDeque<Action>>,
    pub layout_mode: AtomicBool,
    /// "Show everything" is held
    pub show_all: AtomicBool,
    /// Overlay time spent on the last frame, microseconds
    pub frame_us: AtomicU32,
    pub ctx: OnceLock<egui::Context>,
    pub started: Instant,
    pub quit: AtomicBool,
}

impl Shared {
    pub fn new(settings: Settings, theme: Theme) -> Arc<Shared> {
        Arc::new(Shared {
            settings: RwLock::new(settings),
            settings_dirty: AtomicBool::new(false),
            live: RwLock::new(Live::default()),
            events: Mutex::new(EventLog::default()),
            notifications: Mutex::new(Notifications::default()),
            theme: RwLock::new(Arc::new(theme)),
            actions: Mutex::new(VecDeque::new()),
            layout_mode: AtomicBool::new(false),
            show_all: AtomicBool::new(false),
            frame_us: AtomicU32::new(0),
            ctx: OnceLock::new(),
            started: Instant::now(),
            quit: AtomicBool::new(false),
        })
    }

    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    pub fn settings(&self) -> std::sync::RwLockReadGuard<'_, Settings> {
        self.settings.read().unwrap()
    }

    /// Changes settings; marks them dirty so the main thread writes them to disk a little later.
    pub fn edit_settings<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> R {
        let r = f(&mut self.settings.write().unwrap());
        self.settings_dirty.store(true, Ordering::Release);
        self.repaint();
        r
    }

    pub fn take_dirty(&self) -> bool {
        self.settings_dirty.swap(false, Ordering::AcqRel)
    }

    pub fn save_settings(&self) {
        if let Err(e) = self.settings().save(&rh_core::paths::settings_file()) {
            log::warn!("failed to save settings: {e}");
        }
    }

    pub fn live(&self) -> std::sync::RwLockReadGuard<'_, Live> {
        self.live.read().unwrap()
    }

    pub fn theme(&self) -> Arc<Theme> {
        self.theme.read().unwrap().clone()
    }

    pub fn set_theme(&self, t: Theme) {
        *self.theme.write().unwrap() = Arc::new(t);
        self.repaint();
    }

    pub fn act(&self, a: Action) {
        self.actions.lock().unwrap().push_back(a);
        self.repaint();
    }

    pub fn take_actions(&self) -> Vec<Action> {
        self.actions.lock().unwrap().drain(..).collect()
    }

    /// Asks every window to repaint. Called by the reader thread when data changes.
    pub fn repaint(&self) {
        if let Some(ctx) = self.ctx.get() {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            if !self.game_in_front() {
                ctx.request_repaint_of(crate::window::main_viewport());
            }
        }
    }

    /// While the game is in front the system throttles our other windows, and drawing the main window
    /// then holds up the overlay, which shares its thread. The main window waits until the game is left.
    pub fn game_in_front(&self) -> bool {
        self.live.try_read().is_ok_and(|l| l.game_foreground && l.connected())
    }

    pub fn repaint_overlay(&self) {
        if let Some(ctx) = self.ctx.get() {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }

    pub fn layout_mode(&self) -> bool {
        self.layout_mode.load(Ordering::Relaxed)
    }

    pub fn notify(&self, n: crate::notify::Notification) {
        let channels = self.settings().notify.channels.clone();
        crate::plugins::forward(&n, &channels);
        self.notifications.lock().unwrap().push(n);
        self.repaint();
    }
}
