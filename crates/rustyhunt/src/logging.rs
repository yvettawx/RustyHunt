//! 日志：写文件，同时在内存里留最近的几千行给"日志"页和崩溃日志用。

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

const KEEP: usize = 4000;
const MAX_FILE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Line {
    pub time: String,
    pub level: Level,
    pub target: String,
    pub message: String,
}

impl Line {
    pub fn format(&self) -> String {
        format!("{} {:5} {} {}", self.time, self.level, self.target, self.message)
    }
}

struct Logger {
    lines: Mutex<VecDeque<Line>>,
    file: Mutex<Option<File>>,
    path: PathBuf,
    /// 每次新增一行就加一，界面据此判断要不要刷新
    seq: std::sync::atomic::AtomicU64,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

fn short_target(target: &str) -> String {
    let t = target.split("::").next().unwrap_or(target);
    match t {
        "rustyhunt" => target.split("::").nth(1).unwrap_or("app").to_string(),
        "game_world" => "game".into(),
        "address_map" => "address-map".into(),
        "ui_kit" => "theme".into(),
        "plugin_host" => "plugin".into(),
        other => other.replace('_', "-"),
    }
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        // 第三方库只记警告以上
        let ours = ["rustyhunt", "game_world", "rh_core", "platform", "address_map", "ui_kit", "plugin_host"]
            .iter()
            .any(|p| m.target().starts_with(p));
        m.level() <= log::max_level() && (ours || m.level() <= Level::Warn)
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = Line {
            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            level: r.level(),
            target: short_target(r.target()),
            message: r.args().to_string(),
        };
        if let Ok(mut f) = self.file.lock()
            && let Some(f) = f.as_mut()
        {
            let _ = writeln!(f, "{}", line.format());
        }
        let mut l = self.lines.lock().unwrap();
        if l.len() >= KEEP {
            l.pop_front();
        }
        l.push_back(line);
        self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock()
            && let Some(f) = f.as_mut()
        {
            let _ = f.flush();
        }
    }
}

pub fn init(level: rh_core::settings::LogLevel) {
    let dir = rh_core::paths::logs();
    let path = dir.join("rustyhunt.log");
    // 太大时轮换一份
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE) {
        let _ = std::fs::rename(&path, dir.join("rustyhunt.1.log"));
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let logger = LOGGER.get_or_init(|| Logger {
        lines: Mutex::new(VecDeque::with_capacity(KEEP)),
        file: Mutex::new(file),
        path,
        seq: Default::default(),
    });
    let _ = log::set_logger(logger);
    set_level(level);
}

pub fn set_level(level: rh_core::settings::LogLevel) {
    use rh_core::settings::LogLevel;
    log::set_max_level(match level {
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Warn => LevelFilter::Warn,
    });
}

pub fn lines() -> Vec<Line> {
    LOGGER
        .get()
        .map(|l| l.lines.lock().unwrap().iter().cloned().collect())
        .unwrap_or_default()
}

pub fn tail(n: usize) -> Vec<Line> {
    LOGGER
        .get()
        .map(|l| {
            let g = l.lines.lock().unwrap();
            g.iter().skip(g.len().saturating_sub(n)).cloned().collect()
        })
        .unwrap_or_default()
}

pub fn seq() -> u64 {
    LOGGER
        .get()
        .map(|l| l.seq.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(0)
}

pub fn file_path() -> PathBuf {
    LOGGER
        .get()
        .map(|l| l.path.clone())
        .unwrap_or_else(|| rh_core::paths::logs().join("rustyhunt.log"))
}
