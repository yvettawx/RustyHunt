//! 数据目录。安装版放在 `%APPDATA%\RustyHunt`；程序旁边有 `portable`
//! 标记文件时是便携版，放在程序旁边的 `data\`。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn is_portable() -> bool {
    exe_dir().join("portable").exists()
}

/// 只在启动时调用一次；测试里可以指定别的目录。
pub fn init(root: Option<PathBuf>) -> &'static Path {
    ROOT.get_or_init(|| {
        root.unwrap_or_else(|| {
            if is_portable() {
                exe_dir().join("data")
            } else {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(exe_dir)
                    .join("RustyHunt")
            }
        })
    })
}

pub fn root() -> &'static Path {
    init(None)
}

fn sub(name: &str) -> PathBuf {
    let p = root().join(name);
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn settings_file() -> PathBuf {
    root().join("settings.toml")
}

pub fn maps() -> PathBuf {
    sub("maps")
}

pub fn records() -> PathBuf {
    sub("records")
}

pub fn logs() -> PathBuf {
    sub("logs")
}

pub fn crash() -> PathBuf {
    sub("crash")
}

pub fn themes() -> PathBuf {
    sub("themes")
}

pub fn plugins() -> PathBuf {
    sub("plugins")
}

pub fn locales() -> PathBuf {
    sub("locales")
}

pub fn updates() -> PathBuf {
    sub("updates")
}

pub fn cache() -> PathBuf {
    sub("cache")
}

/// 程序自带的资源目录（地址表、数据文件），和 exe 放在一起。
/// 开发时从工作区根目录找。
pub fn bundled(name: &str) -> PathBuf {
    let beside = exe_dir().join(name);
    if beside.exists() {
        return beside;
    }
    let mut dir = exe_dir();
    for _ in 0..4 {
        let candidate = dir.join(name);
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            break;
        }
    }
    beside
}
