//! Updates: app packages and address maps on GitHub Releases.
//!
//! - App: tags look like `v1.2.0`, with assets `RustyHunt-<version>-portable.zip` and `SHA256SUMS`.
//!   Downloaded and verified, then extracted into updates and swapped in on the next start; rolls back if the new version fails to start.
//! - Address maps: tags look like `maps-20261101`, with assets `MonsterHunterWorld.<version>.map`
//!   and `SHA256SUMS`; updates only the maps, not the app.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::notify::{Kind, Notification};
use crate::state::Shared;

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

impl Release {
    pub fn version(&self) -> Option<String> {
        self.tag_name.strip_prefix('v').map(str::to_string)
    }

    pub fn date(&self) -> String {
        self.published_at.as_deref().unwrap_or("").chars().take(10).collect()
    }

    /// Release notes split into lines, like "Added xxx"
    pub fn notes(&self) -> Vec<String> {
        self.body
            .as_deref()
            .unwrap_or("")
            .lines()
            .map(|l| l.trim().trim_start_matches(['-', '*']).trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    }

    fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name.eq_ignore_ascii_case(name))
    }

    pub fn package(&self) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name.ends_with("-portable.zip"))
    }
}

#[derive(Debug, Clone, Default)]
pub enum Download {
    #[default]
    Idle,
    Running {
        done: u64,
        total: u64,
    },
    Ready {
        version: String,
    },
    Failed(String),
}

/// State read by the update panel in the main window.
#[derive(Default)]
pub struct UpdateState {
    pub releases: Vec<Release>,
    pub latest: Option<Release>,
    pub checked: bool,
    pub error: Option<String>,
    pub download: Download,
}

pub static STATE: Mutex<Option<UpdateState>> = Mutex::new(None);

pub fn with_state<R>(f: impl FnOnce(&mut UpdateState) -> R) -> R {
    let mut g = STATE.lock().unwrap();
    f(g.get_or_insert_with(UpdateState::default))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .user_agent(format!("RustyHunt/{}", rh_core::VERSION))
        .build()
        .into()
}

pub fn fetch_releases(repo: &str) -> Result<Vec<Release>, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    resp.body_mut().read_json::<Vec<Release>>().map_err(|e| e.to_string())
}

/// Compares versions like "1.2.10" and "1.10.0".
pub fn newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.split(['.', '-']).map(|x| x.parse().unwrap_or(0)).collect() };
    parse(a) > parse(b)
}

fn sha256(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// A `SHA256SUMS` file: each line is "<hex>  <file name>".
fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let h = it.next()?;
        let n = it.next()?.trim_start_matches('*');
        n.eq_ignore_ascii_case(name).then(|| h.to_ascii_lowercase())
    })
}

fn download(url: &str, dest: &Path, progress: impl Fn(u64, u64)) -> Result<(), String> {
    let mut resp = agent().get(url).call().map_err(|e| e.to_string())?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.body_mut().as_reader();
    let tmp = dest.with_extension("part");
    let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done, total);
    }
    drop(out);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

fn fetch_text(url: &str) -> Result<String, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Check at startup: a new version only triggers a notification, nothing is downloaded. Maps are checked too.
pub fn check_on_start(shared: Arc<Shared>) {
    if !shared.settings().update.check_on_start {
        return;
    }
    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            check(&shared, true);
        })
        .ok();
}

pub fn check(shared: &Shared, quiet: bool) {
    let repo = shared.settings().update.repo.clone();
    match fetch_releases(&repo) {
        Ok(list) => {
            let latest = list
                .iter()
                .filter(|r| !r.prerelease)
                .filter_map(|r| r.version().map(|v| (v, r)))
                .max_by(|a, b| {
                    if newer(&a.0, &b.0) {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
                    }
                })
                .map(|(_, r)| r.clone());
            let new = latest.as_ref().and_then(|r| r.version()).filter(|v| newer(v, rh_core::VERSION));
            if let Some(v) = &new
                && shared.settings().notify.update
                && shared.settings().update.skipped.as_deref() != Some(v)
            {
                shared.notify(Notification::new(
                    Kind::Update { version: v.clone() },
                    rh_core::i18n::tf("notify.update", &[("version", v)]),
                    rh_core::i18n::tf("notify.update_body", &[("version", rh_core::VERSION)]),
                ));
            }
            log::info!(
                "update check: current {}, latest {}",
                rh_core::VERSION,
                latest.as_ref().map(|r| r.tag_name.as_str()).unwrap_or("-")
            );
            update_maps(shared, &list);
            with_state(|s| {
                s.releases = list;
                s.latest = latest;
                s.checked = true;
                s.error = None;
            });
        }
        Err(e) => {
            if !quiet {
                log::warn!("update check failed: {e}");
            } else {
                log::info!("update check failed: {e}");
            }
            with_state(|s| {
                s.checked = true;
                s.error = Some(e);
            });
        }
    }
    shared.repaint();
}

/// Downloads the latest address maps. Only versions not present locally; verified, then put into maps in the data directory.
pub fn update_maps(shared: &Shared, releases: &[Release]) {
    let Some(rel) = releases.iter().filter(|r| r.tag_name.starts_with("maps-")).max_by_key(|r| r.tag_name.clone()) else {
        return;
    };
    let Some(sums) = rel.asset("SHA256SUMS").and_then(|a| fetch_text(&a.browser_download_url).ok()) else {
        return;
    };
    let dir = rh_core::paths::maps();
    let bundled = rh_core::paths::bundled("maps");
    for a in rel.assets.iter().filter(|a| a.name.ends_with(".map")) {
        if dir.join(&a.name).is_file() || bundled.join(&a.name).is_file() {
            continue;
        }
        let dest = dir.join(&a.name);
        if let Err(e) = download(&a.browser_download_url, &dest, |_, _| {}) {
            log::warn!("failed to download address map {}: {e}", a.name);
            continue;
        }
        let ok = expected_hash(&sums, &a.name).is_some_and(|h| sha256(&dest).is_ok_and(|got| got == h));
        if !ok {
            log::warn!("address map {} failed verification, deleted", a.name);
            let _ = std::fs::remove_file(&dest);
            continue;
        }
        let version = a.name.trim_start_matches("MonsterHunterWorld.").trim_end_matches(".map").to_string();
        log::info!("updated address map {version}");
        shared.notify(Notification::new(
            Kind::MapUpdated { version: version.clone() },
            rh_core::i18n::tf("notify.map_updated", &[("version", &version)]),
            String::new(),
        ));
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    version: String,
    dir: PathBuf,
    /// prepared: waiting to be swapped in; applied: swapped in, the new version hasn't been confirmed to start yet
    state: String,
    attempts: u32,
    backup: Option<PathBuf>,
}

fn pending_file() -> PathBuf {
    rh_core::paths::updates().join("pending.json")
}

/// Downloads and verifies a new version in the background.
pub fn start_download(shared: Arc<Shared>, release: Release) {
    std::thread::Builder::new()
        .name("update-download".into())
        .spawn(move || {
            let result = (|| -> Result<String, String> {
                let version = release.version().ok_or("the tag is not a version")?;
                let pkg = release.package().ok_or("no package")?;
                let sums = release.asset("SHA256SUMS").ok_or("no SHA256SUMS")?;
                let sums = fetch_text(&sums.browser_download_url)?;
                let want = expected_hash(&sums, &pkg.name).ok_or("the package is not in SHA256SUMS")?;
                let dir = rh_core::paths::updates();
                let zip_path = dir.join(&pkg.name);
                let s2 = shared.clone();
                download(&pkg.browser_download_url, &zip_path, move |done, total| {
                    with_state(|s| s.download = Download::Running { done, total });
                    s2.repaint();
                })?;
                let got = sha256(&zip_path).map_err(|e| e.to_string())?;
                if got != want {
                    let _ = std::fs::remove_file(&zip_path);
                    return Err(rh_core::i18n::t("update.hash_mismatch"));
                }
                let out = dir.join(format!("v{version}"));
                let _ = std::fs::remove_dir_all(&out);
                let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
                let mut z = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                z.extract(&out).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&zip_path);
                let pending = Pending {
                    version: version.clone(),
                    dir: out,
                    state: "prepared".into(),
                    attempts: 0,
                    backup: None,
                };
                std::fs::write(pending_file(), serde_json::to_string_pretty(&pending).unwrap()).map_err(|e| e.to_string())?;
                log::info!("version {version} downloaded and verified, installing on next start");
                Ok(version)
            })();
            with_state(|s| {
                s.download = match result {
                    Ok(v) => Download::Ready { version: v },
                    Err(e) => {
                        log::warn!("failed to download the update: {e}");
                        Download::Failed(e)
                    }
                }
            });
            shared.repaint();
        })
        .ok();
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        let to = dst.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &to)?;
        } else if e.file_name() == "portable" {
            // Update packages are portable packages: don't turn an installed build into a portable one
            continue;
        } else {
            std::fs::copy(&p, &to)?;
        }
    }
    Ok(())
}

/// Handles a pending update at startup. Returns true when the new version is in place and started, so this process should exit.
pub fn apply_pending() -> bool {
    let path = pending_file();
    let Some(mut p) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Pending>(&t).ok()) else {
        return false;
    };
    let exe = std::env::current_exe().unwrap_or_default();
    let install = rh_core::paths::exe_dir();
    match p.state.as_str() {
        "prepared" => {
            // The package may have an extra directory level
            let root = if p.dir.join("rustyhunt.exe").is_file() {
                p.dir.clone()
            } else {
                std::fs::read_dir(&p.dir)
                    .ok()
                    .and_then(|mut r| r.find_map(|e| e.ok().map(|e| e.path()).filter(|x| x.join("rustyhunt.exe").is_file())))
                    .unwrap_or(p.dir.clone())
            };
            let backup = rh_core::paths::updates().join("backup");
            let _ = std::fs::remove_dir_all(&backup);
            // A running exe can't be overwritten, but it can be renamed
            let old = exe.with_extension("old.exe");
            let _ = std::fs::remove_file(&old);
            if std::fs::rename(&exe, &old).is_err() {
                log::warn!("cannot replace the program files, update skipped");
                let _ = std::fs::remove_file(&path);
                return false;
            }
            let _ = std::fs::copy(&old, backup.join("rustyhunt.exe")).or_else(|_| {
                std::fs::create_dir_all(&backup)?;
                std::fs::copy(&old, backup.join("rustyhunt.exe"))
            });
            let dll = rh_core::native::DLL_NAME;
            let _ = std::fs::copy(install.join(dll), backup.join(dll));
            if let Err(e) = copy_tree(&root, &install) {
                log::error!("failed to copy the new version, rolling back: {e}");
                let _ = std::fs::rename(&old, &exe);
                let _ = std::fs::remove_file(&path);
                return false;
            }
            p.state = "applied".into();
            p.attempts = 0;
            p.backup = Some(backup);
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&p).unwrap());
            log::info!("installed {}, restarting", p.version);
            let _ = std::process::Command::new(&exe).args(std::env::args().skip(1)).spawn();
            true
        }
        "applied" => {
            if p.version == rh_core::VERSION {
                // The new version is running: confirm_started confirms it once the main window is up
                p.attempts += 1;
                if p.attempts > 2 {
                    rollback(&p, &exe);
                    let _ = std::fs::remove_file(&path);
                    let _ = std::process::Command::new(&exe).spawn();
                    return true;
                }
                let _ = std::fs::write(&path, serde_json::to_string_pretty(&p).unwrap());
            } else {
                let _ = std::fs::remove_file(&path);
            }
            false
        }
        _ => false,
    }
}

fn rollback(p: &Pending, exe: &Path) {
    log::error!("version {} failed to start repeatedly, rolled back", p.version);
    if let Some(b) = &p.backup {
        let _ = copy_tree(b, &rh_core::paths::exe_dir());
    }
    let _ = exe;
}

/// Called once the new version runs normally: deletes the old files and the marker.
pub fn confirm_started() {
    let path = pending_file();
    if let Some(p) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Pending>(&t).ok())
        && p.state == "applied"
        && p.version == rh_core::VERSION
    {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&p.dir);
        if let Some(b) = p.backup {
            let _ = std::fs::remove_dir_all(b);
        }
        let exe = std::env::current_exe().unwrap_or_default();
        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
        log::info!("updated to {}", rh_core::VERSION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(newer("1.0.1", "1.0.0"));
        assert!(newer("1.10.0", "1.9.9"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("0.9", "1.0"));
    }

    #[test]
    fn sums_lookup() {
        let s = "abc123  RustyHunt-1.1.0-portable.zip\nDEF  *MonsterHunterWorld.421900.map\n";
        assert_eq!(expected_hash(s, "RustyHunt-1.1.0-portable.zip").as_deref(), Some("abc123"));
        assert_eq!(expected_hash(s, "MonsterHunterWorld.421900.map").as_deref(), Some("def"));
        assert_eq!(expected_hash(s, "x"), None);
    }

    #[test]
    fn release_notes() {
        let r = Release {
            tag_name: "v1.1.0".into(),
            body: Some("## 1.1.0\n- 新增 伤害拆分\n- 修复 部位名错位\n\n".into()),
            published_at: Some("2026-11-02T10:00:00Z".into()),
            prerelease: false,
            assets: vec![],
        };
        assert_eq!(r.version().as_deref(), Some("1.1.0"));
        assert_eq!(r.date(), "2026-11-02");
        assert_eq!(r.notes(), vec!["新增 伤害拆分", "修复 部位名错位"]);
    }
}
