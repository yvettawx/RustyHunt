//! 崩溃日志：写在本地，不上传。下次启动时发通知。

use std::io::Write;
use std::sync::OnceLock;

use crate::notify::{Kind, Notification};
use crate::state::Shared;

static GAME_VERSION: OnceLock<std::sync::Mutex<String>> = OnceLock::new();

pub fn set_game_version(v: &str) {
    *GAME_VERSION.get_or_init(Default::default).lock().unwrap() = v.to_string();
}

pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let msg = info.to_string();
        log::error!("线程 {thread} 崩溃：{msg}");
        let game = GAME_VERSION
            .get()
            .map(|m| m.lock().map(|g| g.clone()).unwrap_or_default())
            .unwrap_or_default();
        let name = format!("crash-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let path = rh_core::paths::crash().join(name);
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = writeln!(f, "RustyHunt {}", rh_core::VERSION);
            let _ = writeln!(f, "游戏版本 {}", if game.is_empty() { "-" } else { &game });
            let _ = writeln!(f, "线程 {thread}");
            let _ = writeln!(f, "{msg}");
            let _ = writeln!(f, "\n{}", std::backtrace::Backtrace::force_capture());
            let _ = writeln!(f, "\n最后 200 行日志：");
            for l in crate::logging::tail(200) {
                let _ = writeln!(f, "{}", l.format());
            }
        }
        default(info);
    }));
}

/// 上次运行留下了没报告过的崩溃日志，就发一条通知。
pub fn report_previous(shared: &Shared) {
    let dir = rh_core::paths::crash();
    let seen = dir.join(".seen");
    let last_seen = std::fs::metadata(&seen).and_then(|m| m.modified()).ok();
    let newest = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t);
    if let Some((t, path)) = newest
        && last_seen.is_none_or(|s| t > s)
    {
        shared.notify(Notification::new(
            Kind::Crash {
                path: path.display().to_string(),
            },
            rh_core::i18n::t("notify.crash"),
            rh_core::i18n::t("notify.crash_body"),
        ));
    }
    let _ = std::fs::write(seen, b"");
}
