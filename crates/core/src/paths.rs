//! Data directory. The installed build uses `%APPDATA%\RustyHunt`; with a `portable` marker
//! file next to the program it is the portable build and uses `data\` next to the program.

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

/// Call once at startup; tests can point it at another directory.
pub fn init(root: Option<PathBuf>) -> &'static Path {
    ROOT.get_or_init(|| {
        root.unwrap_or_else(|| {
            if is_portable() {
                exe_dir().join("data")
            } else {
                std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(exe_dir).join("RustyHunt")
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

/// Directory of bundled resources (address maps, data files), next to the exe.
/// During development it is found from the workspace root.
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
