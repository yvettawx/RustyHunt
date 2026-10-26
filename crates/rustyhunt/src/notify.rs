//! 通知：窗口右下角弹出，标题栏的铃铛里能看历史。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    Update { version: String },
    MapMismatch { version: String },
    MapUpdated { version: String },
    SaveRead { new_gold: usize, new_mini: usize },
    SaveFailed,
    Disconnected,
    Crash { path: String },
    Crown { monster: String, gold: bool },
    Plugin { plugin: String },
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// Unix 秒
    pub time: i64,
    pub read: bool,
    #[serde(skip)]
    pub toast: bool,
}

impl Notification {
    pub fn new(kind: Kind, title: impl Into<String>, body: impl Into<String>) -> Notification {
        Notification {
            kind,
            title: title.into(),
            body: body.into(),
            time: chrono::Local::now().timestamp(),
            read: false,
            toast: true,
        }
    }

    pub fn quiet(mut self) -> Notification {
        self.toast = false;
        self
    }
}

#[derive(Default)]
pub struct Notifications {
    pub items: Vec<Notification>,
    /// 正在右下角显示的那一条
    pub toast: Option<(Notification, std::time::Instant)>,
}

const KEEP: usize = 60;

fn file() -> PathBuf {
    rh_core::paths::root().join("notifications.json")
}

impl Notifications {
    pub fn load() -> Notifications {
        let items = std::fs::read_to_string(file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Notifications { items, toast: None }
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(&self.items) {
            let _ = std::fs::write(file(), t);
        }
    }

    pub fn push(&mut self, n: Notification) {
        // 同一类的旧通知（比如同一个新版本）不重复
        self.items.retain(|o| o.kind != n.kind || matches!(n.kind, Kind::Info | Kind::Plugin { .. }));
        if n.toast {
            self.toast = Some((n.clone(), std::time::Instant::now()));
        }
        self.items.insert(0, n);
        self.items.truncate(KEEP);
        self.save();
    }

    pub fn unread(&self) -> usize {
        self.items.iter().filter(|n| !n.read).count()
    }

    pub fn mark_all_read(&mut self) {
        for n in &mut self.items {
            n.read = true;
        }
        self.save();
    }
}
