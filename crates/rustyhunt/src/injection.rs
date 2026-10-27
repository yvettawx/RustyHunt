//! Patching and injection. Both are on by default and can be turned off in settings:
//!
//! - Integrity check patch: the game checksums its own code and crashes if it was changed (by a hook,
//!   for example). Jumps over the two checks given in the address map, after checking the original
//!   bytes; if they don't match nothing is touched. Applied even when Stracker's Loader is present;
//!   only when the loader has already rewritten the checks (so the original bytes are gone) do we rely on it.
//! - Damage hook: injects rustyhunt-native.dll into the game and hooks the damage function to count
//!   the local player's damage and crits on large monsters, returned over a local port (protocol in
//!   `rh_core::native`). Requires the integrity check patch.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use address_map::AddressMap;
use platform::memory::Memory;
use platform::process::Process;
use rh_core::Event;
use rh_core::i18n;
use rh_core::model::{DamageSplit, Game, Snapshot};
use rh_core::native::{DLL_NAME, PORT, Stats};

use crate::scanner::Sink;
use crate::state::Shared;

static SPLIT: Mutex<Option<DamageSplit>> = Mutex::new(None);
static STATUS: Mutex<Option<Status>> = Mutex::new(None);

pub fn damage_split() -> Option<DamageSplit> {
    SPLIT.lock().unwrap().clone()
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemStatus {
    Off,
    Waiting,
    Active(String),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Status {
    pub crc: ItemStatus,
    pub hook: ItemStatus,
    pub dll: String,
    pub ipc: String,
    pub last_failure: Option<String>,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            crc: ItemStatus::Off,
            hook: ItemStatus::Off,
            dll: i18n::t("inj.dll_none"),
            ipc: i18n::t("inj.ipc_none"),
            last_failure: None,
        }
    }
}

pub fn status() -> Status {
    STATUS.lock().unwrap().clone().unwrap_or_default()
}

fn set_status(f: impl FnOnce(&mut Status)) {
    let mut s = STATUS.lock().unwrap();
    f(s.get_or_insert_with(Status::default));
}

/// Patch: (jump source, jump target, original bytes at the source, original bytes at the target)
const CRC_PAIRS: [(&str, &str, &str, &str); 2] = [
    ("CRC_FUNC_1A", "CRC_FUNC_1B", "CRC_ORIGINAL_FUNC_1A", "CRC_ORIGINAL_FUNC_1B"),
    ("CRC_FUNC_2A", "CRC_FUNC_2B", "CRC_ORIGINAL_FUNC_2A", "CRC_ORIGINAL_FUNC_2B"),
];

/// 6 bytes jumping from a to b: jmp rel32 plus a nop
fn jump(a: u64, b: u64) -> [u8; 6] {
    let rel = (b as i64 - (a as i64 + 5)) as i32;
    let r = rel.to_le_bytes();
    [0xE9, r[0], r[1], r[2], r[3], 0x90]
}

struct Conn {
    out: TcpStream,
    reader: BufReader<TcpStream>,
}

impl Conn {
    fn open() -> std::io::Result<Conn> {
        let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
        let s = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
        s.set_read_timeout(Some(Duration::from_secs(2)))?;
        s.set_nodelay(true)?;
        Ok(Conn {
            out: s.try_clone()?,
            reader: BufReader::new(s),
        })
    }

    fn ask(&mut self, req: &str) -> std::io::Result<String> {
        self.out.write_all(req.as_bytes())?;
        self.out.write_all(b"\n")?;
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "连接已关闭"));
        }
        Ok(line.trim().to_string())
    }
}

struct Session {
    pid: u32,
    base: u64,
    map: AddressMap,
    process: Option<Process>,
    stracker: bool,
    /// Our patch couldn't apply because Stracker's Loader had already changed the checks
    loader_patched: bool,
    /// Patches applied: addresses and original bytes, written back when turned off
    patched: Vec<(u64, Vec<u8>)>,
    conn: Option<Conn>,
    injected: bool,
    hooked: bool,
    last_poll: f64,
    retry_at: f64,
}

fn dll_path() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    // In development cargo names the file with underscores
    [DLL_NAME, "rustyhunt_native.dll"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

impl Session {
    fn new(shared: &Shared, pid: u32) -> Option<Session> {
        let version = shared.live().version.clone()?;
        let (map, _) = game_world::attach::load_map(&crate::scanner::map_dirs(), &version).ok()?;
        let modules = platform::process::modules(pid).ok()?;
        let exe = Game::World.exe();
        let main = modules.iter().find(|m| m.name.eq_ignore_ascii_case(exe))?;
        // Stracker's Loader is loaded into the game as loader.dll
        let stracker = modules.iter().any(|m| m.name.eq_ignore_ascii_case("loader.dll"));
        Some(Session {
            pid,
            base: main.base,
            map,
            process: None,
            stracker,
            loader_patched: false,
            patched: Vec::new(),
            conn: None,
            injected: false,
            hooked: false,
            last_poll: -100.0,
            retry_at: 0.0,
        })
    }

    fn process(&mut self) -> Result<&Process, String> {
        if self.process.is_none() {
            self.process = Some(Process::open_writable(self.pid).map_err(|e| e.to_string())?);
        }
        Ok(self.process.as_ref().unwrap())
    }

    fn abs(&self, name: &str) -> Result<u64, String> {
        self.map.address(name).map(|a| self.base + a).ok_or_else(|| format!("地址表里没有 {name}"))
    }

    fn crc_on(&mut self) -> Result<(), String> {
        let mut plan = Vec::new();
        for (a, b, orig_a, orig_b) in CRC_PAIRS {
            let (pa, pb) = (self.abs(a)?, self.abs(b)?);
            let want_a = self.map.bytes(orig_a).ok_or_else(|| format!("地址表里没有 {orig_a}"))?;
            let want_b = self.map.bytes(orig_b).ok_or_else(|| format!("地址表里没有 {orig_b}"))?;
            let p = self.process()?;
            let mut have_a = vec![0u8; want_a.len()];
            let mut have_b = vec![0u8; want_b.len()];
            p.read_bytes(pa, &mut have_a).map_err(|e| e.to_string())?;
            p.read_bytes(pb, &mut have_b).map_err(|e| e.to_string())?;
            let patch = jump(pa, pb);
            if have_a.starts_with(&patch) {
                // Already patched by a previous run (the main program restarted, the game didn't)
                plan.push((pa, None, want_a));
                continue;
            }
            if have_a != want_a || have_b != want_b {
                return Err(i18n::t("inj.bytes_mismatch"));
            }
            plan.push((pa, Some(patch), want_a));
        }
        for (addr, patch, orig) in plan {
            if let Some(patch) = patch {
                self.process()?.patch_code(addr, &patch).map_err(|e| e.to_string())?;
            }
            self.patched.push((addr, orig));
        }
        log::info!("完整性检查补丁已生效");
        Ok(())
    }

    fn crc_off(&mut self) {
        let patched = std::mem::take(&mut self.patched);
        if let Ok(p) = self.process() {
            for (addr, orig) in patched {
                if let Err(e) = p.patch_code(addr, &orig) {
                    log::warn!("恢复 0x{addr:X} 失败：{e}");
                }
            }
        }
        log::info!("已恢复完整性检查");
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.conn.is_some() {
            return Ok(());
        }
        if let Ok(mut c) = Conn::open() {
            let hello = c.ask("HELLO").map_err(|e| e.to_string())?;
            self.conn = Some(c);
            self.injected = true;
            set_status(|s| s.dll = hello.replace("RUSTYHUNT-NATIVE", DLL_NAME));
            return Ok(());
        }
        if self.injected {
            return Err(i18n::t("inj.ipc_none"));
        }
        // Inject only once; if it can't connect afterwards, retry next time
        self.injected = true;
        let dll = dll_path().ok_or_else(|| format!("找不到 {DLL_NAME}"))?;
        log::info!("注入 {}", dll.display());
        self.process()?.inject_dll(&dll).map_err(|e| e.to_string())?;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if Conn::open().is_ok() {
                self.injected = false;
                return self.connect();
            }
        }
        Err(i18n::t("inj.ipc_none"))
    }

    fn hook(&mut self) -> Result<(), String> {
        self.connect()?;
        let addr = self.abs("FUN_DEAL_DAMAGE")?;
        let reply = self.conn.as_mut().unwrap().ask(&format!("HOOK {addr:X}")).map_err(|e| e.to_string())?;
        if reply != "OK" {
            return Err(reply.trim_start_matches("ERR ").to_string());
        }
        log::info!("伤害 hook 已挂上 0x{addr:X}");
        self.hooked = true;
        Ok(())
    }

    fn ask(&mut self, req: &str) -> Option<String> {
        let c = self.conn.as_mut()?;
        match c.ask(req) {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("和 {DLL_NAME} 的连接断开：{e}");
                self.conn = None;
                self.hooked = false;
                set_status(|s| s.ipc = i18n::t("inj.ipc_none"));
                None
            }
        }
    }
}

pub struct Injector {
    session: Option<Session>,
}

impl Injector {
    pub fn new() -> Injector {
        Injector { session: None }
    }
}

impl Default for Injector {
    fn default() -> Self {
        Injector::new()
    }
}

fn fail(what: &str, e: &str) {
    log::warn!("{what}失败：{e}");
    let e = e.to_string();
    set_status(|s| s.last_failure = Some(format!("{} {e}", chrono::Local::now().format("%H:%M:%S"))));
}

impl Sink for Injector {
    fn frame(&mut self, shared: &Shared, snap: &Arc<Snapshot>, events: &[Event]) {
        let inj = shared.settings().injection.clone();
        let now = shared.now();
        if self.session.is_none() {
            if !inj.crc_patch {
                set_status(|s| {
                    s.crc = ItemStatus::Off;
                    s.hook = ItemStatus::Off;
                });
                return;
            }
            let Some(pid) = shared.live().pid else { return };
            self.session = Session::new(shared, pid);
        }
        let Some(s) = self.session.as_mut() else { return };

        // Integrity check patch: always our own, even with Stracker's Loader present
        if inj.crc_patch && s.patched.is_empty() && !s.loader_patched && now >= s.retry_at {
            match s.crc_on() {
                Ok(()) => set_status(|st| st.crc = ItemStatus::Active(i18n::t("inj.patched"))),
                Err(e) if s.stracker => {
                    // The loader already rewrote the checks, so the original bytes are gone; rely on it
                    log::info!("完整性检查已被 Stracker's Loader 改过：{e}");
                    s.loader_patched = true;
                    set_status(|st| st.crc = ItemStatus::Active(i18n::t("inj.handled")));
                }
                Err(e) => {
                    fail(&i18n::t("inj.crc"), &e);
                    set_status(|st| st.crc = ItemStatus::Failed(e));
                    // When the bytes don't match, stop retrying until the game or the map changes
                    s.retry_at = f64::INFINITY;
                }
            }
        }
        // The patch can't be removed while the hook is installed, or the game crashes when it sees the change; wait for a game restart
        if !inj.crc_patch && !s.patched.is_empty() && !s.hooked {
            s.crc_off();
            set_status(|st| st.crc = ItemStatus::Off);
        }
        let crc_ok = !s.patched.is_empty() || s.loader_patched;

        // Damage hook
        let want = inj.damage_hook && crc_ok && !(inj.solo_only && snap.online);
        if want && !s.hooked && now >= s.retry_at {
            match s.hook() {
                Ok(()) => set_status(|st| {
                    st.hook = ItemStatus::Active(i18n::t("inj.injected"));
                    st.ipc = i18n::t("inj.ipc_ok");
                }),
                Err(e) => {
                    fail(&i18n::t("inj.hook"), &e);
                    set_status(|st| st.hook = ItemStatus::Failed(e));
                    s.retry_at = now + 30.0;
                }
            }
        } else if !want && s.hooked {
            s.ask("UNHOOK");
            s.hooked = false;
            *SPLIT.lock().unwrap() = None;
            set_status(|st| st.hook = if inj.damage_hook { ItemStatus::Waiting } else { ItemStatus::Off });
        } else if !want && inj.damage_hook {
            set_status(|st| st.hook = ItemStatus::Waiting);
        } else if !inj.damage_hook {
            set_status(|st| st.hook = ItemStatus::Off);
        }

        if s.hooked {
            if events.iter().any(|e| matches!(e, Event::QuestStart { .. })) {
                s.ask("CLEAR");
            }
            if now - s.last_poll >= 0.5 {
                s.last_poll = now;
                if let Some(st) = s.ask("STATS").as_deref().and_then(Stats::parse) {
                    *SPLIT.lock().unwrap() = Some(DamageSplit {
                        total: st.total as f32,
                        // The damage function can't tell raw from elemental
                        physical: None,
                        elemental: None,
                        critical: st.critical as f32,
                        hits: st.hits,
                        critical_hits: st.critical_hits,
                    });
                }
            }
        }
    }

    fn detached(&mut self, _shared: &Shared) {
        self.session = None;
        *SPLIT.lock().unwrap() = None;
        set_status(|s| *s = Status::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_is_relative_to_next_instruction() {
        let j = jump(0x1000, 0x1200);
        assert_eq!(j[0], 0xE9);
        assert_eq!(i32::from_le_bytes([j[1], j[2], j[3], j[4]]), 0x1FB);
        assert_eq!(j[5], 0x90);
        let back = jump(0x2000, 0x1000);
        assert_eq!(i32::from_le_bytes([back[1], back[2], back[3], back[4]]), -0x1005);
    }
}
