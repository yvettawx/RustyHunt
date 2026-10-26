//! 读取线程、overlay、主界面共用的状态。

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
    /// 在找游戏进程
    Searching,
    Connected,
    /// 没有这个版本的地址表，停止读取
    MapMismatch { version: String, latest: Option<String> },
    /// 打不开进程（权限不够等）
    Failed(String),
    /// 没有游戏，用演示数据
    Demo,
}

#[derive(Debug, Clone)]
pub struct Live {
    pub snapshot: Arc<Snapshot>,
    pub connection: Connection,
    pub pid: Option<u32>,
    pub exe: String,
    pub version: Option<String>,
    pub map_file: Option<String>,
    pub attached_at: Option<Instant>,
    /// 游戏客户区在屏幕上的位置，物理像素
    pub game_rect: Option<platform::window::Rect>,
    pub game_foreground: bool,
    pub game_minimized: bool,
    /// 上一帧读内存花的时间
    pub read_ms: f32,
    pub actual_hz: f32,
    /// 自己每 2 秒的 DPS，用来画曲线
    pub dps_history: Vec<f32>,
    /// 当前目标怪物
    pub target: Option<u64>,
    /// 最近一连串攻击的伤害（目标怪物）
    pub combo_damage: f32,
    pub combo_at: f64,
    /// 读取出错的计数，日志里按次数汇总
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
            map_file: None,
            attached_at: None,
            game_rect: None,
            game_foreground: false,
            game_minimized: false,
            read_ms: 0.0,
            actual_hz: 0.0,
            dps_history: Vec::new(),
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

    /// 有数据可显示：连上了游戏，或者在演示
    pub fn has_data(&self) -> bool {
        matches!(self.connection, Connection::Connected | Connection::Demo)
    }
}

/// 热键、托盘、主界面发出的动作。
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

/// 最近的事件，overlay 用来播报，主界面用来显示时间轴。
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
    /// 按住"显示全部"
    pub show_all: AtomicBool,
    /// overlay 上一帧的耗时，微秒
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

    /// 改设置；改完标记为待保存，由主线程隔一会儿写盘。
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
            log::warn!("保存设置失败：{e}");
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

    /// 请所有窗口重绘。读取线程在数据变化时调用。
    pub fn repaint(&self) {
        if let Some(ctx) = self.ctx.get() {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            ctx.request_repaint_of(crate::window::main_viewport());
        }
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
