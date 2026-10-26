//! 主题管理：内置浅色和深色，本地主题放在数据目录的 themes 下，
//! 每个主题一个文件夹。当前主题的文件保存后自动重新载入。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use rh_core::settings::Settings;
use ui_kit::Theme;
use ui_kit::theme::{MAX_PACK_BYTES, ThemeError, version_ok};

use crate::state::Shared;

/// 当前主题最近一次的错误，overlay 角落和日志页用。
pub static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub struct Entry {
    pub id: String,
    pub theme: Option<Theme>,
    pub error: Option<String>,
    pub dir: Option<PathBuf>,
}

impl Entry {
    pub fn builtin(&self) -> bool {
        self.dir.is_none()
    }
}

pub fn list() -> Vec<Entry> {
    let mut out = vec![
        Entry {
            id: "light".into(),
            theme: Some(Theme::builtin(false)),
            error: None,
            dir: None,
        },
        Entry {
            id: "dark".into(),
            theme: Some(Theme::builtin(true)),
            error: None,
            dir: None,
        },
    ];
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(rh_core::paths::themes())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("theme.xml").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        match load_checked(&dir) {
            Ok(t) => out.push(Entry {
                id: t.manifest.id.clone(),
                error: t.module_errors.first().map(|e| e.to_string()),
                theme: Some(t),
                dir: Some(dir),
            }),
            Err(e) => out.push(Entry {
                id,
                theme: None,
                error: Some(e),
                dir: Some(dir),
            }),
        }
    }
    out
}

/// 读主题并检查程序版本（8.2：不兼容的停用并写明原因）。
pub fn load_checked(dir: &Path) -> Result<Theme, String> {
    let t = Theme::load_dir(dir).map_err(|e| e.to_string())?;
    if !version_ok(&t.manifest.app, rh_core::VERSION) {
        return Err(rh_core::i18n::tf(
            "theme.needs_app",
            &[("need", &t.manifest.app), ("current", rh_core::VERSION)],
        ));
    }
    Ok(t)
}

pub fn load_active(settings: &Settings) -> Theme {
    let id = if settings.theme_follow_system {
        if crate::win::system_dark() { "dark" } else { "light" }.to_string()
    } else {
        settings.theme.clone()
    };
    match id.as_str() {
        "light" => Theme::builtin(false),
        "dark" => Theme::builtin(true),
        other => {
            let dir = rh_core::paths::themes().join(other);
            match load_checked(&dir) {
                Ok(t) => {
                    for e in &t.module_errors {
                        log::warn!("主题 {other}：{e}");
                    }
                    *LAST_ERROR.lock().unwrap() = t.module_errors.first().map(|e| e.to_string());
                    t
                }
                Err(e) => {
                    log::error!("主题 {other} 载入失败，改用浅色：{e}");
                    *LAST_ERROR.lock().unwrap() = Some(e);
                    Theme::builtin(false)
                }
            }
        }
    }
}

pub fn install_fonts(ctx: &egui::Context, theme: &Theme) {
    let mut extra = ui_kit::fonts::ExtraFonts::new();
    if let Some(dir) = &theme.dir {
        for (name, f) in &theme.tokens.fonts {
            if let Some(src) = &f.src
                && let Ok(bytes) = std::fs::read(dir.join(src))
            {
                extra.insert(name.clone(), bytes);
            }
        }
    }
    ui_kit::fonts::install(ctx, &extra);
}

pub fn apply(shared: &Shared, id: &str) {
    shared.edit_settings(|s| s.theme = id.to_string());
    let t = load_active(&shared.settings());
    if let Some(ctx) = shared.ctx.get() {
        install_fonts(ctx, &t);
        crate::window::style::apply(ctx, &t);
    }
    shared.set_theme(t);
    log::info!("应用主题 {id}");
}

fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    let mut newest = None;
    for e in walk(dir) {
        if let Ok(t) = std::fs::metadata(&e).and_then(|m| m.modified()) {
            newest = newest.max(Some(t));
        }
    }
    newest
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// 当前主题的文件改动后自动重新载入。
pub fn watch(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("theme-watch".into())
        .spawn(move || {
            let mut last: Option<(String, Option<SystemTime>)> = None;
            while !shared.quit.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(700));
                let id = shared.settings().theme.clone();
                if id == "light" || id == "dark" {
                    last = None;
                    continue;
                }
                let dir = rh_core::paths::themes().join(&id);
                let m = newest_mtime(&dir);
                match &last {
                    Some((lid, lm)) if *lid == id && *lm == m => continue,
                    Some((lid, _)) if *lid == id => {
                        log::info!("主题 {id} 的文件有改动，重新载入");
                        let t = load_active(&shared.settings());
                        if let Some(ctx) = shared.ctx.get() {
                            install_fonts(ctx, &t);
                            crate::window::style::apply(ctx, &t);
                        }
                        shared.set_theme(t);
                    }
                    _ => {}
                }
                last = Some((id, m));
            }
        })
        .expect("能创建主题监视线程");
}

fn dir_size(dir: &Path) -> u64 {
    walk(dir).iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
}

/// 从 .zip 或文件夹导入主题包。返回主题编号。
pub fn import(src: &Path) -> Result<String, String> {
    let tmp = rh_core::paths::cache().join(format!("theme-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let root = if src.is_dir() {
        copy_dir(src, &tmp)?;
        find_root(&tmp)
    } else {
        let file = std::fs::File::open(src).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(name) = f.enclosed_name() else { continue };
            total += f.size();
            if total > MAX_PACK_BYTES {
                return Err(rh_core::i18n::t("theme.too_big"));
            }
            let out = tmp.join(name);
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
            }
        }
        find_root(&tmp)
    }
    .ok_or_else(|| rh_core::i18n::t("theme.no_xml"))?;
    if dir_size(&root) > MAX_PACK_BYTES {
        return Err(rh_core::i18n::t("theme.too_big"));
    }
    let theme = Theme::load_dir(&root).map_err(|e: ThemeError| e.to_string())?;
    let id = theme.manifest.id.clone();
    let dest = rh_core::paths::themes().join(&id);
    let _ = std::fs::remove_dir_all(&dest);
    copy_dir(&root, &dest)?;
    let _ = std::fs::remove_dir_all(&tmp);
    log::info!("导入主题 {id}（{}）", theme.manifest.name);
    Ok(id)
}

fn find_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("theme.xml").is_file() {
        return Some(dir.to_path_buf());
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.join("theme.xml").is_file())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        let to = dst.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &to)?;
        } else {
            std::fs::copy(&p, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn delete(dir: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())
}
