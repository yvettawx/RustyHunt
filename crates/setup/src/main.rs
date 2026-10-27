//! Installer for the installed build of RustyHunt: installs into the current user's folder, no admin rights needed;
//! data lives in `%APPDATA%\RustyHunt\`. Run with `--uninstall` to uninstall.
//!
//! The payload is the portable zip (without the `portable` marker), so both forms have exactly the same program files
//! and updates use the same package.

#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::ffi::OsStr;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW,
    RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_DEFBUTTON2, MB_ICONERROR, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW};

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.zip"));
const VERSION: &str = env!("CARGO_PKG_VERSION");
const APP: &str = "RustyHunt";
const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\RustyHunt";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy)]
enum Lang {
    Hans,
    Hant,
    Ja,
    En,
}

fn lang() -> Lang {
    let id = unsafe { GetUserDefaultUILanguage() };
    match (id & 0x3FF, id) {
        (0x04, 0x0404 | 0x0C04 | 0x1404 | 0x7C04) => Lang::Hant,
        (0x04, _) => Lang::Hans,
        (0x11, _) => Lang::Ja,
        _ => Lang::En,
    }
}

/// The installer's own few lines of text, picked by system language.
fn t(key: &str) -> &'static str {
    let l = lang();
    let row: [&str; 4] = match key {
        "install_to" => ["安装到", "安裝到", "インストール先", "Install to"],
        "installed" => ["已安装", "已安裝", "インストールしました", "Installed"],
        "uninstall" => ["卸载 RustyHunt", "解除安裝 RustyHunt", "RustyHunt をアンインストール", "Uninstall RustyHunt"],
        "keep_data" => [
            "同时删除设置、狩猎记录、主题和插件",
            "同時刪除設定、狩獵紀錄、主題和插件",
            "設定、狩猟記録、テーマ、プラグインも削除",
            "Also delete settings, hunt records, themes and plugins",
        ],
        "running" => [
            "请先退出 RustyHunt",
            "請先結束 RustyHunt",
            "RustyHunt を終了してください",
            "Quit RustyHunt first",
        ],
        "empty" => [
            "安装包里没有程序文件",
            "安裝包裡沒有程式檔案",
            "インストーラーにプログラムが含まれていません",
            "This installer has no program files",
        ],
        "failed" => ["安装失败", "安裝失敗", "インストールに失敗しました", "Installation failed"],
        _ => ["", "", "", ""],
    };
    row[l as usize]
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn message(text: &str, flags: u32) -> i32 {
    let title = wide(&format!("{APP} {VERSION}"));
    let body = wide(text);
    unsafe { MessageBoxW(std::ptr::null_mut(), body.as_ptr(), title.as_ptr(), flags) }
}

fn install_dir() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    local.join("Programs").join(APP)
}

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(APP))
}

fn shortcut() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Microsoft\\Windows\\Start Menu\\Programs").join(format!("{APP}.lnk")))
}

/// Path inside the zip with the common top-level folder removed; None for unsafe paths.
fn entry_path(name: &Path, strip: Option<&Path>) -> Option<PathBuf> {
    let p = match strip {
        Some(s) => name.strip_prefix(s).ok()?.to_path_buf(),
        None => name.to_path_buf(),
    };
    (!p.as_os_str().is_empty()).then_some(p)
}

fn common_root(z: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Option<PathBuf> {
    let mut root: Option<PathBuf> = None;
    for i in 0..z.len() {
        let e = z.by_index(i).ok()?;
        let name = e.enclosed_name()?;
        let mut comps = name.components();
        let first = comps.next()?.as_os_str().to_owned();
        // A file at the top level means there's no common folder (extensions can't tell: RustyHunt-1.0.0 has an "extension" too)
        if comps.next().is_none() && !e.is_dir() {
            return None;
        }
        match &root {
            None => root = Some(PathBuf::from(first)),
            Some(r) if r.as_os_str() != first => return None,
            _ => {}
        }
    }
    root
}

fn extract(payload: &[u8], to: &Path) -> Result<u64, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(payload)).map_err(|e| e.to_string())?;
    let strip = common_root(&mut z);
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = e.enclosed_name() else { continue };
        let Some(rel) = entry_path(&name, strip.as_deref()) else { continue };
        // The installed build doesn't want the portable marker
        if rel == Path::new("portable") {
            continue;
        }
        let out = to.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(e.size() as usize);
        e.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out, &buf).map_err(|err| format!("{}：{err}", out.display()))?;
        total += buf.len() as u64;
    }
    Ok(total)
}

fn reg_set(h: HKEY, name: &str, value: &str) {
    let n = wide(name);
    let v = wide(value);
    unsafe {
        RegSetValueExW(h, n.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32);
    }
}

fn reg_dword(h: HKEY, name: &str, value: u32) {
    let n = wide(name);
    unsafe {
        RegSetValueExW(h, n.as_ptr(), 0, REG_DWORD, &value as *const u32 as *const u8, 4);
    }
}

/// Registers under Settings > Apps so it can be uninstalled from there.
fn register(dir: &Path, bytes: u64) {
    let key = wide(UNINSTALL_KEY);
    let mut h: HKEY = std::ptr::null_mut();
    let rc = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut h,
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return;
    }
    let exe = dir.join("rustyhunt.exe");
    reg_set(h, "DisplayName", APP);
    reg_set(h, "DisplayVersion", VERSION);
    reg_set(h, "Publisher", APP);
    reg_set(h, "InstallLocation", &dir.display().to_string());
    reg_set(h, "DisplayIcon", &exe.display().to_string());
    reg_set(h, "UninstallString", &format!("\"{}\" --uninstall", dir.join("uninstall.exe").display()));
    reg_dword(h, "NoModify", 1);
    reg_dword(h, "NoRepair", 1);
    reg_dword(h, "EstimatedSize", (bytes / 1024) as u32);
    unsafe { RegCloseKey(h) };
}

fn powershell(script: &str) {
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

fn make_shortcut(dir: &Path) {
    let Some(lnk) = shortcut() else { return };
    let exe = dir.join("rustyhunt.exe");
    let q = |p: &Path| p.display().to_string().replace('\'', "''");
    powershell(&format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
        q(&lnk),
        q(&exe),
        q(dir)
    ));
}

fn install() {
    if PAYLOAD.is_empty() {
        message(t("empty"), MB_OK | MB_ICONERROR);
        return;
    }
    let dir = install_dir();
    if message(&format!("{}\n{}", t("install_to"), dir.display()), MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    // A running exe can't be deleted: try renaming it first; if that fails the program is open
    let exe = dir.join("rustyhunt.exe");
    if exe.exists() {
        let probe = dir.join("rustyhunt.exe.setup");
        if std::fs::rename(&exe, &probe).is_err() {
            message(t("running"), MB_OK | MB_ICONERROR);
            return;
        }
        let _ = std::fs::rename(&probe, &exe);
    }
    match extract(PAYLOAD, &dir) {
        Ok(bytes) => {
            if let Ok(me) = std::env::current_exe() {
                let _ = std::fs::copy(me, dir.join("uninstall.exe"));
            }
            register(&dir, bytes);
            make_shortcut(&dir);
            let _ = Command::new(&exe).current_dir(&dir).spawn();
        }
        Err(e) => {
            message(&format!("{}\n{e}", t("failed")), MB_OK | MB_ICONERROR);
        }
    }
}

fn remove_autostart() {
    let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let mut h: HKEY = std::ptr::null_mut();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut h) } == 0 {
        let name = wide(APP);
        unsafe {
            RegDeleteValueW(h, name.as_ptr());
            RegCloseKey(h);
        }
    }
}

fn uninstall() {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(install_dir);
    if message(t("uninstall"), MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    let exe = dir.join("rustyhunt.exe");
    if exe.exists() && std::fs::remove_file(&exe).is_err() {
        message(t("running"), MB_OK | MB_ICONERROR);
        return;
    }
    if let Some(d) = data_dir()
        && d.exists()
        && message(t("keep_data"), MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES
    {
        let _ = std::fs::remove_dir_all(d);
    }
    remove_autostart();
    if let Some(l) = shortcut() {
        let _ = std::fs::remove_file(l);
    }
    let key = wide(UNINSTALL_KEY);
    unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    // We're still running and can't delete our own folder: let cmd delete it after we exit
    let _ = Command::new("cmd")
        .raw_arg(format!("/c ping 127.0.0.1 -n 3 >nul & rmdir /s /q \"{}\"", dir.display()))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

fn main() {
    if std::env::args().any(|a| a == "--uninstall") {
        uninstall();
    } else {
        install();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_payload_without_portable_marker() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            for (name, data) in [
                ("RustyHunt-1.0.0/rustyhunt.exe", "exe"),
                ("RustyHunt-1.0.0/portable", ""),
                ("RustyHunt-1.0.0/locales/ui/en.toml", "x = 1"),
            ] {
                w.start_file(name, o).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("rh-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let n = extract(&buf, &dir).unwrap();
        assert_eq!(n, 8);
        assert_eq!(std::fs::read_to_string(dir.join("rustyhunt.exe")).unwrap(), "exe");
        assert!(dir.join("locales/ui/en.toml").is_file());
        assert!(!dir.join("portable").exists(), "安装版不能带便携版标记");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn strips_top_folder() {
        let p = entry_path(Path::new("RustyHunt-1.0.0/locales/ui/en.toml"), Some(Path::new("RustyHunt-1.0.0"))).unwrap();
        assert_eq!(p, Path::new("locales/ui/en.toml"));
        assert!(entry_path(Path::new("RustyHunt-1.0.0"), Some(Path::new("RustyHunt-1.0.0"))).is_none());
        assert_eq!(entry_path(Path::new("rustyhunt.exe"), None).unwrap(), Path::new("rustyhunt.exe"));
    }
}
