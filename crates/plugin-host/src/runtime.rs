//! WebAssembly 运行时（wasmtime）。每个插件一个线程、一个 Store；
//! 单次调用超过时限或者出错（trap）时只停掉这个插件。
//!
//! 插件导出：
//! - `memory`、`rh_alloc(len) -> ptr`（必需）
//! - `rh_init(ptr, len)`：启动时收到设置（JSON）
//! - `rh_event(ptr, len)`：订阅的事件（JSON）
//! - `rh_frame(ptr, len)`：数据快照（JSON，字段同主题模板）
//! - `rh_action(ptr, len)`：热键动作编号
//! - `rh_settings(ptr, len)`：设置改了
//! - `rh_export(ptr, len) -> i64`：导出器，输入狩猎记录 JSON，返回文件内容
//! - `rh_notify(ptr, len)`：通知渠道收到的通知
//!
//! 插件能调用的宿主函数在 `rh` 模块里，见 `docs/plugins.md`。插件拿不到
//! 游戏进程，也没有写游戏内存的函数。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::OnceLock;
use std::time::Duration;

use wasmtime::{AsContextMut, Caller, Config, Engine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

use crate::manifest::{Manifest, Permission};

/// 单次调用的时限
pub const CALL_TIMEOUT: Duration = Duration::from_millis(500);
const TICK: Duration = Duration::from_millis(10);
/// 插件内存上限
pub const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const MAX_FILE: usize = 4 * 1024 * 1024;

/// 宿主发给插件线程的消息。
pub enum Msg {
    Event(String),
    Frame(String),
    Action(String),
    Settings(String),
    Notify(String),
    Export {
        input: String,
        reply: Sender<Result<Vec<u8>, String>>,
    },
    Stop,
}

/// 插件线程发回宿主的消息。
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Log { level: u32, text: String },
    ModuleData { module: String, json: String },
    RecordField { key: String, value: String },
    Notify { title: String, body: String },
    Serve { port: u16, json: String },
    /// 插件出错或超时，已停止
    Failed(String),
}

pub fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| {
        let mut c = Config::new();
        c.epoch_interruption(true);
        let e = Engine::new(&c).expect("wasmtime 引擎");
        let ticker = e.clone();
        std::thread::Builder::new()
            .name("wasm-epoch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(TICK);
                    ticker.increment_epoch();
                }
            })
            .expect("能创建计时线程");
        e
    })
}

static SAVE_READER: OnceLock<fn() -> String> = OnceLock::new();

/// 设置 `save_read` 返回的内容（图鉴 JSON）。
pub fn set_save_reader(f: fn() -> String) {
    let _ = SAVE_READER.set(f);
}

fn deadline_ticks() -> u64 {
    (CALL_TIMEOUT.as_millis() / TICK.as_millis()) as u64
}

pub struct HostCtx {
    pub plugin: String,
    /// 插件自己的数据文件夹
    pub data_dir: PathBuf,
    pub granted: HashSet<Permission>,
    pub out: Sender<(String, Out)>,
    limits: StoreLimits,
}

impl HostCtx {
    fn send(&self, o: Out) {
        let _ = self.out.send((self.plugin.clone(), o));
    }

    fn allowed(&self, p: Permission) -> bool {
        self.granted.contains(&p)
    }
}

fn read_bytes(caller: &mut Caller<'_, HostCtx>, ptr: i32, len: i32) -> Option<Vec<u8>> {
    let mem = caller.get_export("memory").and_then(Extern::into_memory)?;
    let data = mem.data(&caller);
    let (p, l) = (usize::try_from(ptr).ok()?, usize::try_from(len).ok()?);
    data.get(p..p.checked_add(l)?).map(<[u8]>::to_vec)
}

fn read_str(caller: &mut Caller<'_, HostCtx>, ptr: i32, len: i32) -> Option<String> {
    read_bytes(caller, ptr, len).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// 把字节写进插件内存（调用插件的 rh_alloc），返回 (ptr << 32) | len；失败返回 -1。
fn give(caller: &mut Caller<'_, HostCtx>, bytes: &[u8]) -> i64 {
    let Some(alloc) = caller.get_export("rh_alloc").and_then(Extern::into_func) else { return -1 };
    let Ok(alloc) = alloc.typed::<i32, i32>(&caller) else { return -1 };
    let Ok(ptr) = alloc.call(&mut *caller, bytes.len() as i32) else { return -1 };
    let Some(mem) = caller.get_export("memory").and_then(Extern::into_memory) else { return -1 };
    if mem.write(&mut *caller, ptr as usize, bytes).is_err() {
        return -1;
    }
    ((ptr as i64) << 32) | bytes.len() as i64
}

/// 插件文件夹里的相对路径；不能跳出去。
fn sandbox(dir: &Path, rel: &str) -> Option<PathBuf> {
    if rel.contains("..") || rel.starts_with(['/', '\\']) || rel.contains(':') {
        return None;
    }
    Some(dir.join(rel))
}

fn link(linker: &mut Linker<HostCtx>) -> wasmtime::Result<()> {
    linker.func_wrap("rh", "log", |mut c: Caller<'_, HostCtx>, level: i32, ptr: i32, len: i32| {
        if let Some(text) = read_str(&mut c, ptr, len) {
            c.data().send(Out::Log {
                level: level.clamp(0, 4) as u32,
                text: text.chars().take(2000).collect(),
            });
        }
    })?;
    linker.func_wrap(
        "rh",
        "set_module_data",
        |mut c: Caller<'_, HostCtx>, mp: i32, ml: i32, jp: i32, jl: i32| {
            if let (Some(module), Some(json)) = (read_str(&mut c, mp, ml), read_str(&mut c, jp, jl)) {
                c.data().send(Out::ModuleData { module, json });
            }
        },
    )?;
    linker.func_wrap(
        "rh",
        "set_record_field",
        |mut c: Caller<'_, HostCtx>, kp: i32, kl: i32, vp: i32, vl: i32| {
            if let (Some(key), Some(value)) = (read_str(&mut c, kp, kl), read_str(&mut c, vp, vl)) {
                c.data().send(Out::RecordField { key, value });
            }
        },
    )?;
    linker.func_wrap("rh", "notify", |mut c: Caller<'_, HostCtx>, tp: i32, tl: i32, bp: i32, bl: i32| {
        if let (Some(title), Some(body)) = (read_str(&mut c, tp, tl), read_str(&mut c, bp, bl)) {
            c.data().send(Out::Notify { title, body });
        }
    })?;
    linker.func_wrap("rh", "now_ms", |_c: Caller<'_, HostCtx>| -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    })?;
    linker.func_wrap("rh", "fs_read", |mut c: Caller<'_, HostCtx>, pp: i32, pl: i32| -> i64 {
        if !c.data().allowed(Permission::Files) {
            return -2;
        }
        let Some(rel) = read_str(&mut c, pp, pl) else { return -1 };
        let Some(path) = sandbox(&c.data().data_dir, &rel) else { return -1 };
        match std::fs::read(path) {
            Ok(b) if b.len() <= MAX_FILE => give(&mut c, &b),
            _ => -1,
        }
    })?;
    linker.func_wrap(
        "rh",
        "fs_write",
        |mut c: Caller<'_, HostCtx>, pp: i32, pl: i32, dp: i32, dl: i32| -> i32 {
            if !c.data().allowed(Permission::Files) {
                return -2;
            }
            if dl as usize > MAX_FILE {
                return -1;
            }
            let (Some(rel), Some(data)) = (read_str(&mut c, pp, pl), read_bytes(&mut c, dp, dl)) else { return -1 };
            let Some(path) = sandbox(&c.data().data_dir, &rel) else { return -1 };
            if let Some(p) = path.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            if std::fs::write(path, data).is_ok() { 0 } else { -1 }
        },
    )?;
    linker.func_wrap("rh", "http_get", |mut c: Caller<'_, HostCtx>, up: i32, ul: i32| -> i64 {
        if !c.data().allowed(Permission::Net) {
            return -2;
        }
        let Some(url) = read_str(&mut c, up, ul) else { return -1 };
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return -1;
        }
        // 网络请求不算进调用时限
        c.as_context_mut().set_epoch_deadline(deadline_ticks() + 600);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        match agent.get(&url).call().and_then(|mut r| r.body_mut().with_config().limit(MAX_FILE as u64).read_to_vec()) {
            Ok(b) => give(&mut c, &b),
            Err(_) => -1,
        }
    })?;
    linker.func_wrap("rh", "save_read", |mut c: Caller<'_, HostCtx>| -> i64 {
        if !c.data().allowed(Permission::Save) {
            return -2;
        }
        let text = SAVE_READER.get().map(|f| f()).unwrap_or_default();
        give(&mut c, text.as_bytes())
    })?;
    linker.func_wrap("rh", "serve_json", |mut c: Caller<'_, HostCtx>, port: i32, jp: i32, jl: i32| -> i32 {
        if !c.data().allowed(Permission::Port) {
            return -2;
        }
        if !(1024..=65535).contains(&port) {
            return -1;
        }
        let Some(json) = read_str(&mut c, jp, jl) else { return -1 };
        c.data().send(Out::Serve { port: port as u16, json });
        0
    })?;
    Ok(())
}

pub struct Running {
    pub tx: Sender<Msg>,
    pub handle: std::thread::JoinHandle<()>,
}

struct Guest {
    store: Store<HostCtx>,
    instance: Instance,
    alloc: TypedFunc<i32, i32>,
}

impl Guest {
    fn input(&mut self, s: &[u8]) -> wasmtime::Result<(i32, i32)> {
        self.store.set_epoch_deadline(deadline_ticks());
        let ptr = self.alloc.call(&mut self.store, s.len() as i32)?;
        let mem = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| wasmtime::Error::msg("没有导出 memory"))?;
        mem.write(&mut self.store, ptr as usize, s)?;
        Ok((ptr, s.len() as i32))
    }

    /// 调用一个 (ptr, len) 参数的导出；插件没有导出这个函数时什么也不做。
    fn call(&mut self, name: &str, arg: &str) -> wasmtime::Result<()> {
        let Ok(f) = self.instance.get_typed_func::<(i32, i32), ()>(&mut self.store, name) else {
            return Ok(());
        };
        let (p, l) = self.input(arg.as_bytes())?;
        self.store.set_epoch_deadline(deadline_ticks());
        f.call(&mut self.store, (p, l))
    }

    fn export(&mut self, input: &str) -> wasmtime::Result<Vec<u8>> {
        let f = self.instance.get_typed_func::<(i32, i32), i64>(&mut self.store, "rh_export")?;
        let (p, l) = self.input(input.as_bytes())?;
        self.store.set_epoch_deadline(deadline_ticks());
        let packed = f.call(&mut self.store, (p, l))?;
        if packed < 0 {
            return Err(wasmtime::Error::msg("导出器返回错误"));
        }
        let (ptr, len) = ((packed >> 32) as usize, (packed & 0xFFFF_FFFF) as usize);
        let mem = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| wasmtime::Error::msg("没有导出 memory"))?;
        mem.data(&self.store)
            .get(ptr..ptr + len)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| wasmtime::Error::msg("返回的范围不对"))
    }
}

fn describe(e: &wasmtime::Error) -> String {
    if let Some(t) = e.downcast_ref::<wasmtime::Trap>()
        && *t == wasmtime::Trap::Interrupt
    {
        return format!("超时 {} ms", CALL_TIMEOUT.as_millis());
    }
    let s = e.to_string();
    s.lines().next().unwrap_or(&s).to_string()
}

/// 启动一个插件。编译、实例化失败时直接返回错误；之后的错误通过 `Out::Failed` 报告。
pub fn spawn(
    manifest: &Manifest,
    plugin_dir: &Path,
    data_dir: &Path,
    granted: HashSet<Permission>,
    settings_json: String,
    out: Sender<(String, Out)>,
) -> Result<Running, String> {
    let engine = engine().clone();
    let wasm = std::fs::read(plugin_dir.join(&manifest.entry)).map_err(|e| format!("{}：{e}", manifest.entry))?;
    let module = Module::new(&engine, &wasm).map_err(|e| describe(&e))?;
    let id = manifest.id.clone();
    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let data_dir = data_dir.to_path_buf();
    let _ = std::fs::create_dir_all(&data_dir);
    let (ready_tx, ready_rx) = channel::<Result<(), String>>();
    let handle = std::thread::Builder::new()
        .name(format!("plugin-{id}"))
        .spawn(move || {
            let ctx = HostCtx {
                plugin: id.clone(),
                data_dir,
                granted,
                out: out.clone(),
                limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).instances(1).build(),
            };
            let mut store = Store::new(&engine, ctx);
            store.limiter(|c| &mut c.limits);
            store.set_epoch_deadline(deadline_ticks());
            let mut linker = Linker::new(&engine);
            let setup = (|| -> wasmtime::Result<Guest> {
                link(&mut linker)?;
                let instance = linker.instantiate(&mut store, &module)?;
                let alloc = instance.get_typed_func::<i32, i32>(&mut store, "rh_alloc")?;
                Ok(Guest { store, instance, alloc })
            })();
            let mut guest = match setup {
                Ok(g) => g,
                Err(e) => {
                    let _ = ready_tx.send(Err(describe(&e)));
                    return;
                }
            };
            if let Err(e) = guest.call("rh_init", &settings_json) {
                let _ = ready_tx.send(Err(describe(&e)));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            let fail = |e: wasmtime::Error| {
                let _ = out.send((id.clone(), Out::Failed(describe(&e))));
            };
            while let Ok(msg) = rx.recv() {
                let r = match msg {
                    Msg::Event(j) => guest.call("rh_event", &j),
                    Msg::Frame(j) => guest.call("rh_frame", &j),
                    Msg::Action(a) => guest.call("rh_action", &a),
                    Msg::Settings(j) => guest.call("rh_settings", &j),
                    Msg::Notify(j) => guest.call("rh_notify", &j),
                    Msg::Export { input, reply } => {
                        let r = guest.export(&input);
                        let failed = r.as_ref().err().map(describe);
                        let _ = reply.send(r.map_err(|e| describe(&e)));
                        match failed {
                            Some(f) if f.starts_with("超时") => Err(wasmtime::Error::msg(f)),
                            _ => Ok(()),
                        }
                    }
                    Msg::Stop => break,
                };
                if let Err(e) = r {
                    fail(e);
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(Running { tx, handle }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("启动超时".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &Path, wat: &str, extra: &str) -> Manifest {
        std::fs::create_dir_all(dir).unwrap();
        let wasm = wat::parse_str(wat).unwrap();
        std::fs::write(dir.join("plugin.wasm"), wasm).unwrap();
        let m: Manifest = toml::from_str(&format!("id = \"t\"\nname = \"t\"\nversion = \"0.1.0\"\n{extra}")).unwrap();
        m
    }

    const BASE: &str = r#"
  (import "rh" "log" (func $log (param i32 i32 i32)))
  (import "rh" "set_module_data" (func $data (param i32 i32 i32 i32)))
  (memory (export "memory") 1)
  (global $top (mut i32) (i32.const 1024))
  (func (export "rh_alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $top))
    (global.set $top (i32.add (global.get $top) (local.get $n)))
    (local.get $p))
  (data (i32.const 0) "split{\"t\":1}")
"#;

    #[test]
    fn runs_plugin_and_receives_messages() {
        let dir = std::env::temp_dir().join(format!("rh-plugin-ok-{}", std::process::id()));
        let wat = format!(
            r#"(module {BASE}
  (func (export "rh_event") (param i32 i32)
    (call $log (i32.const 2) (local.get 0) (local.get 1))
    (call $data (i32.const 0) (i32.const 5) (i32.const 5) (i32.const 7))))"#
        );
        let m = write_plugin(&dir, &wat, "");
        let (otx, orx) = channel();
        let r = spawn(&m, &dir, &dir.join("data"), HashSet::new(), "{}".into(), otx).unwrap();
        r.tx.send(Msg::Event("{\"type\":\"enrage\"}".into())).unwrap();
        let (_, a) = orx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(a, Out::Log { level: 2, text: "{\"type\":\"enrage\"}".into() });
        let (_, b) = orx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(b, Out::ModuleData { module: "split".into(), json: "{\"t\":1}".into() });
        r.tx.send(Msg::Stop).unwrap();
        r.handle.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endless_loop_is_stopped() {
        let dir = std::env::temp_dir().join(format!("rh-plugin-loop-{}", std::process::id()));
        let wat = format!(r#"(module {BASE} (func (export "rh_event") (param i32 i32) (loop (br 0))))"#);
        let m = write_plugin(&dir, &wat, "");
        let (otx, orx) = channel();
        let r = spawn(&m, &dir, &dir.join("data"), HashSet::new(), "{}".into(), otx).unwrap();
        r.tx.send(Msg::Event("{}".into())).unwrap();
        let (_, o) = orx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(o, Out::Failed(ref s) if s.contains("超时")), "{o:?}");
        r.handle.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn trap_is_reported() {
        let dir = std::env::temp_dir().join(format!("rh-plugin-trap-{}", std::process::id()));
        let wat = format!(r#"(module {BASE} (func (export "rh_event") (param i32 i32) unreachable))"#);
        let m = write_plugin(&dir, &wat, "");
        let (otx, orx) = channel();
        let r = spawn(&m, &dir, &dir.join("data"), HashSet::new(), "{}".into(), otx).unwrap();
        r.tx.send(Msg::Event("{}".into())).unwrap();
        let (_, o) = orx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(o, Out::Failed(_)));
        r.handle.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_access_needs_permission() {
        let dir = std::env::temp_dir().join(format!("rh-plugin-perm-{}", std::process::id()));
        let wat = format!(
            r#"(module
  (import "rh" "fs_write" (func $w (param i32 i32 i32 i32) (result i32)))
  {BASE}
  (func (export "rh_event") (param i32 i32)
    (call $log (i32.const 1)
      (i32.const 0)
      (i32.add (i32.const 0) (i32.add (call $w (i32.const 0) (i32.const 5) (i32.const 5) (i32.const 7)) (i32.const 2))))))"#
        );
        let m = write_plugin(&dir, &wat, "");
        let (otx, orx) = channel();
        let r = spawn(&m, &dir, &dir.join("data"), HashSet::new(), "{}".into(), otx).unwrap();
        r.tx.send(Msg::Event("{}".into())).unwrap();
        // 没权限时 fs_write 返回 -2，log 的长度就是 0
        let (_, o) = orx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(o, Out::Log { level: 1, text: String::new() });
        assert!(!dir.join("data").join("split").exists());
        r.tx.send(Msg::Stop).unwrap();
        r.handle.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
