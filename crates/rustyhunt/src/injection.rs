//! 补丁与注入。两项都默认关闭，开启前弹窗确认：
//!
//! - 完整性检查补丁：游戏会校验自己的代码段，改过代码（比如挂 hook）就会崩溃。
//!   按地址表把两处校验跳过去；先核对原始字节，对不上就不动。游戏目录里有
//!   Stracker's Loader 时它已经做了同样的事，这里跳过。
//! - 伤害 hook：把 rustyhunt-native.dll 注入游戏，挂在伤害函数上，统计本地玩家
//!   打大型怪物的伤害和会心，经本机端口交回来（协议见 `rh_core::native`）。
//!   必须先有完整性检查补丁。

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
    pub stracker: bool,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            crc: ItemStatus::Off,
            hook: ItemStatus::Off,
            dll: i18n::t("inj.dll_none"),
            ipc: i18n::t("inj.ipc_none"),
            last_failure: None,
            stracker: false,
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

/// 补丁：(跳转起点, 跳转终点, 起点的原始字节, 终点的原始字节)
const CRC_PAIRS: [(&str, &str, &str, &str); 2] = [
    ("CRC_FUNC_1A", "CRC_FUNC_1B", "CRC_ORIGINAL_FUNC_1A", "CRC_ORIGINAL_FUNC_1B"),
    ("CRC_FUNC_2A", "CRC_FUNC_2B", "CRC_ORIGINAL_FUNC_2A", "CRC_ORIGINAL_FUNC_2B"),
];

/// 从 a 跳到 b 的 6 字节：jmp rel32，再补一个 nop
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
    /// 已经打上的补丁：地址和原始字节，关掉时写回去
    patched: Vec<(u64, Vec<u8>)>,
    conn: Option<Conn>,
    injected: bool,
    hooked: bool,
    last_poll: f64,
    retry_at: f64,
}

fn dll_path() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    // 开发时 cargo 生成的文件名是下划线
    [DLL_NAME, "rustyhunt_native.dll"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

impl Session {
    fn new(shared: &Shared, pid: u32) -> Option<Session> {
        let version = shared.live().version.clone()?;
        let (map, _) = game_world::attach::load_map(&crate::scanner::map_dirs(), &version).ok()?;
        let modules = platform::process::modules(pid).ok()?;
        let exe = Game::World.exe();
        let main = modules.iter().find(|m| m.name.eq_ignore_ascii_case(exe))?;
        // Stracker's Loader 以 loader.dll 的名字载入游戏
        let stracker = modules.iter().any(|m| m.name.eq_ignore_ascii_case("loader.dll"));
        set_status(|s| s.stracker = stracker);
        Some(Session {
            pid,
            base: main.base,
            map,
            process: None,
            stracker,
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
                // 上次运行已经打过（主程序重启了，游戏没重启）
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
        // 只注入一次；之后连不上就等下次重试
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

        // 完整性检查补丁
        if inj.crc_patch && s.patched.is_empty() && !s.stracker && now >= s.retry_at {
            match s.crc_on() {
                Ok(()) => set_status(|st| st.crc = ItemStatus::Active(i18n::t("inj.patched"))),
                Err(e) => {
                    fail(&i18n::t("inj.crc"), &e);
                    set_status(|st| st.crc = ItemStatus::Failed(e));
                    // 字节对不上时不再反复尝试，等游戏或地址表变化
                    s.retry_at = f64::INFINITY;
                }
            }
        }
        if s.stracker {
            set_status(|st| st.crc = ItemStatus::Active(i18n::t("inj.handled")));
        }
        // hook 挂着时不能撤补丁，否则游戏校验到改动会崩溃；等游戏重启
        if !inj.crc_patch && !s.patched.is_empty() && !s.hooked {
            s.crc_off();
            set_status(|st| st.crc = ItemStatus::Off);
        }
        let crc_ok = !s.patched.is_empty() || s.stracker;

        // 伤害 hook
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
                        // 伤害函数分不出物理和属性
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
