//! Plugins: scans the `plugins/` folder, starts plugins according to their permissions, sends them
//! snapshots and events, and receives their module data, record fields and notifications. The runtime is in `plugin-host`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use plugin_host::manifest::{API_VERSION, SettingDecl, SettingKind};
use plugin_host::runtime::{self, Msg, Out, Running};
use plugin_host::{Manifest, Permission};
use rh_core::i18n;
use rh_core::model::{Game, Snapshot};
use rh_core::modules::{ElementInfo, ModuleInfo};
use rh_core::paths;
use rh_core::settings::{ModuleConfig, PluginState};
use serde_json::Value;
use ui_kit::render::Images;
use ui_kit::template::Template;

use crate::notify::{Kind, Notification};
use crate::overlay::{Frame, State};
use crate::scanner::Sink;
use crate::state::Shared;

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Off,
    Running,
    /// Failed or timed out while running; disabled
    Failed(String),
    /// Incompatible version
    Incompatible(String),
    /// The manifest can't be read
    Broken(String),
}

pub struct Plugin {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub status: Status,
    running: Option<Running>,
    /// Module templates parsed for the current theme; parsed again when the theme changes
    templates: HashMap<String, Result<Template, String>>,
    templates_for: usize,
    data: HashMap<String, Value>,
    images: Images,
    last_frame: f64,
}

impl Plugin {
    fn send(&self, m: Msg) {
        if let Some(r) = &self.running {
            let _ = r.tx.send(m);
        }
    }

    pub fn label(&self) -> String {
        format!("{} {}", self.manifest.name, self.manifest.version)
    }
}

/// Lock order: REG first, then the settings lock. Never call these functions while holding the settings lock.
struct Registry {
    plugins: Vec<Plugin>,
    out: Option<Sender<(String, Out)>>,
    servers: BTreeMap<u16, Arc<Mutex<(String, String)>>>,
}

static REG: Mutex<Registry> = Mutex::new(Registry {
    plugins: Vec::new(),
    out: None,
    servers: BTreeMap::new(),
});
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

fn reg() -> std::sync::MutexGuard<'static, Registry> {
    REG.lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts only when both the app version and the plugin API version are satisfied (8.2).
fn compatible(m: &Manifest) -> Result<(), String> {
    if !ui_kit::theme::version_ok(&m.app, rh_core::VERSION) {
        return Err(i18n::tf("plugin.needs_app", &[("need", &m.app), ("current", rh_core::VERSION)]));
    }
    let major = |v: &str| v.split('.').next().unwrap_or("0").trim().to_string();
    if major(&m.api) != major(API_VERSION) || !ui_kit::theme::version_ok(&m.api, API_VERSION) {
        return Err(i18n::tf("plugin.needs_api", &[("need", &m.api), ("current", API_VERSION)]));
    }
    Ok(())
}

fn setting_default(s: &SettingDecl) -> Value {
    match &s.kind {
        SettingKind::Bool { default } => Value::Bool(*default),
        SettingKind::Number { default, .. } => serde_json::json!(default),
        SettingKind::Text { default } => Value::String(default.clone()),
    }
}

/// Current values of a plugin's settings; unchanged ones use the manifest defaults.
pub fn setting_value(st: Option<&PluginState>, s: &SettingDecl) -> Value {
    st.and_then(|st| st.settings.get(&s.key))
        .and_then(|v| serde_json::to_value(v).ok())
        .unwrap_or_else(|| setting_default(s))
}

fn settings_json(m: &Manifest, st: Option<&PluginState>) -> String {
    let o: serde_json::Map<String, Value> = m.settings.iter().map(|s| (s.key.clone(), setting_value(st, s))).collect();
    Value::Object(o).to_string()
}

fn module_info(m: &Manifest, d: &plugin_host::manifest::ModuleDecl) -> ModuleInfo {
    ModuleInfo {
        id: m.module_key(&d.id),
        name: d.name.clone(),
        default_on: d.default_on,
        elements: d
            .elements
            .iter()
            .map(|e| ElementInfo {
                key: e.key.clone(),
                name: e.name.clone(),
                default_on: e.default_on,
            })
            .collect(),
        games: vec![Game::World],
        plugin: Some(m.id.clone()),
    }
}

/// Reads the plugin folder and starts enabled plugins. Called once at startup.
pub fn init(shared: Arc<Shared>) {
    if SHARED.set(shared.clone()).is_err() {
        return;
    }
    runtime::set_save_reader(crate::bestiary::export_json);
    let (tx, rx) = channel::<(String, Out)>();
    reg().out = Some(tx);
    std::thread::Builder::new()
        .name("plugin-out".into())
        .spawn(move || {
            while let Ok((id, o)) = rx.recv() {
                handle(&shared, &id, o);
            }
        })
        .expect("能创建插件线程");
    scan();
}

fn load_dir(dir: &Path) -> Plugin {
    let (manifest, status) = match Manifest::load(dir) {
        Ok(m) => {
            let st = compatible(&m).err().map_or(Status::Off, Status::Incompatible);
            (m, st)
        }
        Err(e) => {
            let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let m = Manifest {
                id: name.clone(),
                name,
                description: String::new(),
                version: String::new(),
                author: String::new(),
                app: String::new(),
                api: String::new(),
                entry: String::new(),
                permissions: Vec::new(),
                events: Vec::new(),
                frame_hz: 0,
                modules: Vec::new(),
                actions: Vec::new(),
                settings: Vec::new(),
                exporters: Vec::new(),
                channels: Vec::new(),
            };
            (m, Status::Broken(e))
        }
    };
    Plugin {
        manifest,
        dir: dir.to_path_buf(),
        status,
        running: None,
        templates: HashMap::new(),
        templates_for: 0,
        data: HashMap::new(),
        images: Images::new(Some(dir.to_path_buf())),
        last_frame: -100.0,
    }
}

/// Rescans the plugin folder.
pub fn scan() {
    let Some(shared) = SHARED.get() else { return };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(paths::plugins())
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && p.join("plugin.toml").exists())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    let mut r = reg();
    for p in r.plugins.drain(..) {
        p.send(Msg::Stop);
    }
    let out = r.out.clone();
    for d in dirs {
        let mut p = load_dir(&d);
        let enabled = shared.settings().plugins.get(&p.manifest.id).is_some_and(|s| s.enabled);
        if enabled
            && p.status == Status::Off
            && let Some(out) = &out
        {
            start(shared, &mut p, out.clone());
        }
        r.plugins.push(p);
    }
}

fn start(shared: &Shared, p: &mut Plugin, out: Sender<(String, Out)>) {
    let m = p.manifest.clone();
    let st = shared.settings().plugins.get(&m.id).cloned().unwrap_or_default();
    let granted: HashSet<Permission> = m.permissions.iter().copied().filter(|x| st.granted.contains(x.key())).collect();
    let data_dir = p.dir.join("data");
    match runtime::spawn(&m, &p.dir, &data_dir, granted, settings_json(&m, Some(&st)), out) {
        Ok(r) => {
            p.running = Some(r);
            p.status = Status::Running;
            p.data.clear();
            log::info!(target: "plugin", "{} 已启动", p.label());
            // Add plugin modules to every profile
            let infos: Vec<ModuleInfo> = m.modules.iter().map(|d| module_info(&m, d)).collect();
            shared.edit_settings(|s| {
                for l in s.games.values_mut() {
                    for prof in l.profiles.values_mut() {
                        for i in &infos {
                            prof.ensure(Game::World, i);
                        }
                    }
                }
            });
        }
        Err(e) => {
            log::error!(target: "plugin", "{} 启动失败：{e}", p.label());
            p.status = Status::Failed(e);
        }
    }
}

fn stop(p: &mut Plugin) {
    p.send(Msg::Stop);
    // Don't wait for the thread: a stuck plugin gets interrupted by the timeout
    p.running = None;
    p.data.clear();
    if p.status == Status::Running {
        p.status = Status::Off;
    }
}

fn handle(shared: &Arc<Shared>, id: &str, o: Out) {
    let name = || {
        reg()
            .plugins
            .iter()
            .find(|p| p.manifest.id == id)
            .map(Plugin::label)
            .unwrap_or_else(|| id.to_string())
    };
    match o {
        Out::Log { level, text } => {
            let lv = match level {
                0 => log::Level::Error,
                1 => log::Level::Warn,
                2 => log::Level::Info,
                3 => log::Level::Debug,
                _ => log::Level::Trace,
            };
            log::log!(target: "plugin", lv, "{}：{text}", name());
        }
        Out::ModuleData { module, json } => {
            let v = serde_json::from_str::<Value>(&json).unwrap_or(Value::Null);
            if let Some(p) = reg().plugins.iter_mut().find(|p| p.manifest.id == id) {
                p.data.insert(module, v);
            }
            shared.repaint_overlay();
        }
        Out::RecordField { key, value } => {
            crate::records::PENDING_FIELDS.lock().unwrap().insert(format!("{id}.{key}"), value);
        }
        Out::Notify { title, body } => {
            shared.notify(Notification::new(Kind::Plugin { plugin: id.to_string() }, title, body));
        }
        Out::Serve { port, json } => serve(id, port, json),
        Out::Failed(e) => {
            let label = name();
            log::error!(target: "plugin", "{label} {e}，已停用");
            {
                let mut r = reg();
                if let Some(p) = r.plugins.iter_mut().find(|p| p.manifest.id == id) {
                    p.running = None;
                    p.status = Status::Failed(e.clone());
                }
            }
            shared.edit_settings(|s| {
                if let Some(st) = s.plugins.get_mut(id) {
                    st.enabled = false;
                }
            });
            shared.notify(Notification::new(Kind::Plugin { plugin: id.to_string() }, label, i18n::t("plugin.errored")));
        }
    }
}

/// A local read-only port requested by a plugin: serves the latest JSON on 127.0.0.1 (permission port).
fn serve(id: &str, port: u16, json: String) {
    let mut r = reg();
    if let Some(slot) = r.servers.get(&port) {
        let mut s = slot.lock().unwrap();
        if s.0 == id {
            s.1 = json;
        }
        return;
    }
    let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::warn!(target: "plugin", "{id} 端口 {port}：{e}");
            return;
        }
    };
    let slot = Arc::new(Mutex::new((id.to_string(), json)));
    r.servers.insert(port, slot.clone());
    let owner = id.to_string();
    let _ = std::thread::Builder::new().name(format!("plugin-port-{port}")).spawn(move || {
        for mut s in listener.incoming().flatten() {
            let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let alive = reg().plugins.iter().any(|p| p.manifest.id == owner && p.running.is_some());
            let body = if alive { slot.lock().unwrap().1.clone() } else { "{}".to_string() };
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(body.as_bytes());
        }
    });
}

/// The scanner thread hands over each frame's snapshot and events.
pub struct Bridge;

impl Bridge {
    pub fn new(shared: Arc<Shared>) -> Bridge {
        init(shared);
        Bridge
    }
}

fn frame_json(shared: &Shared, snap: &Snapshot) -> String {
    let (low_hp, target) = {
        let s = shared.settings();
        (s.profile(Game::World).player.low_hp / 100.0, shared.live().target)
    };
    let i = i18n::current();
    let o = ui_kit::fields::ContextOptions {
        i18n: &i,
        density: "full",
        time: shared.now(),
        hot_secs: 8.0,
        activity: None,
        target,
        low_hp,
        damage_recent: 0.0,
    };
    ui_kit::fields::context(snap, &o).to_string()
}

impl Sink for Bridge {
    fn frame(&mut self, shared: &Shared, snap: &Arc<Snapshot>, events: &[rh_core::Event]) {
        let now = shared.now();
        let mut r = reg();
        let mut frame: Option<String> = None;
        for p in r.plugins.iter_mut().filter(|p| p.running.is_some()) {
            for e in events {
                if p.manifest.events.iter().any(|x| x == e.name())
                    && let Ok(mut j) = serde_json::to_value(e)
                {
                    // Quest time when the event happened
                    j["elapsed"] = serde_json::json!(snap.elapsed);
                    p.send(Msg::Event(j.to_string()));
                }
            }
            let hz = p.manifest.frame_hz.min(30);
            if hz > 0 && now - p.last_frame >= 1.0 / hz as f64 {
                p.last_frame = now;
                let j = frame.get_or_insert_with(|| frame_json(shared, snap)).clone();
                p.send(Msg::Frame(j));
            }
        }
    }
}

fn split_key(id: &str) -> Option<(&str, &str)> {
    id.split_once('.')
}

/// Data sent by plugin modules, placed under `data` in the template context.
pub fn module_data(id: &str) -> Option<Value> {
    let (pid, mid) = split_key(id)?;
    reg()
        .plugins
        .iter()
        .find(|p| p.manifest.id == pid && p.running.is_some())?
        .data
        .get(mid)
        .cloned()
}

/// Draws a plugin module: parses the plugin's template for the current theme; data is the snapshot fields plus the plugin's `data`.
pub fn draw_module(id: &str, cv: &ui_kit::Canvas, f: &Frame, cfg: &ModuleConfig, st: &mut State, ctx: &egui::Context) -> egui::Vec2 {
    let Some((pid, mid)) = split_key(id) else { return egui::Vec2::ZERO };
    let mut r = reg();
    let Some(p) = r.plugins.iter_mut().find(|p| p.manifest.id == pid && p.running.is_some()) else {
        return egui::Vec2::ZERO;
    };
    let theme_id = cv.theme as *const ui_kit::Theme as usize;
    if p.templates_for != theme_id {
        p.templates.clear();
        p.templates_for = theme_id;
    }
    let dir = p.dir.clone();
    let decl = p.manifest.modules.iter().find(|m| m.id == mid).cloned();
    let elements = decl.as_ref().map(|d| module_info(&p.manifest, d).elements).unwrap_or_default();
    let t = p.templates.entry(mid.to_string()).or_insert_with(|| {
        let Some(d) = decl else { return Err("没有这个模块".into()) };
        let text = std::fs::read_to_string(dir.join(&d.template)).map_err(|e| format!("{}：{e}", d.template))?;
        ui_kit::template::parse_str(&text, &cv.theme.tokens).map_err(|e| {
            log::warn!(target: "plugin", "{pid} {}：{e}", d.template);
            e.to_string()
        })
    });
    let Ok(t) = t else { return egui::Vec2::ZERO };
    let mut data = st.context.get_or_insert_with(|| crate::overlay::template_context(f, &st.activity)).clone();
    data["data"] = p.data.get(mid).cloned().unwrap_or(Value::Null);
    data["module"] = crate::overlay::module_context(&elements, cfg);
    let (size, anim) = ui_kit::render::render(t, cv, &mut p.images, ctx, &data, f.now);
    st.animating |= anim;
    size
}

/// Built-in modules plus modules registered by plugins.
pub fn all_modules() -> Vec<ModuleInfo> {
    let mut v = rh_core::modules::builtin();
    v.extend(module_infos());
    v
}

/// In-game modules registered by plugins (enabled plugins only).
pub fn module_infos() -> Vec<ModuleInfo> {
    reg()
        .plugins
        .iter()
        .filter(|p| p.running.is_some())
        .flat_map(|p| p.manifest.modules.iter().map(|d| module_info(&p.manifest, d)))
        .collect()
}

pub fn module_name(id: &str) -> String {
    if let Some((pid, mid)) = split_key(id)
        && let Some(m) = reg()
            .plugins
            .iter()
            .find(|p| p.manifest.id == pid)
            .and_then(|p| p.manifest.modules.iter().find(|m| m.id == mid))
    {
        return m.name.clone();
    }
    i18n::t(&format!("module.{id}"))
}

pub fn run_action(_shared: &Shared, plugin: &str, action: &str) {
    if let Some(p) = reg().plugins.iter().find(|p| p.manifest.id == plugin) {
        p.send(Msg::Action(action.to_string()));
    }
}

pub struct ActionInfo {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
    pub default_key: String,
}

pub fn actions() -> Vec<ActionInfo> {
    reg()
        .plugins
        .iter()
        .filter(|p| p.running.is_some())
        .flat_map(|p| {
            p.manifest.actions.iter().map(|a| ActionInfo {
                plugin: p.manifest.id.clone(),
                plugin_name: p.manifest.name.clone(),
                id: a.id.clone(),
                name: a.name.clone(),
                default_key: a.key.clone(),
            })
        })
        .collect()
}

/// Default hotkey of a plugin action, used by the hotkey thread when the user hasn't changed it.
pub fn default_keys() -> Vec<(String, String)> {
    actions()
        .into_iter()
        .filter(|a| !a.default_key.is_empty())
        .map(|a| (format!("{}.{}", a.plugin, a.id), a.default_key))
        .collect()
}

pub struct ChannelInfo {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
}

pub fn channels() -> Vec<ChannelInfo> {
    reg()
        .plugins
        .iter()
        .filter(|p| p.running.is_some())
        .flat_map(|p| {
            p.manifest.channels.iter().map(|c| ChannelInfo {
                plugin: p.manifest.id.clone(),
                plugin_name: p.manifest.name.clone(),
                id: c.id.clone(),
                name: c.name.clone(),
            })
        })
        .collect()
}

/// Forwards a notification to the plugin channels that are on (8.1 notification methods).
pub fn forward(n: &Notification, enabled: &std::collections::BTreeMap<String, bool>) {
    let r = reg();
    for p in r.plugins.iter().filter(|p| p.running.is_some()) {
        if matches!(&n.kind, Kind::Plugin { plugin } if *plugin == p.manifest.id) {
            continue;
        }
        for c in &p.manifest.channels {
            if enabled.get(&format!("{}.{}", p.manifest.id, c.id)).copied().unwrap_or(false) {
                let j = serde_json::json!({ "channel": c.id, "title": n.title, "body": n.body, "time": n.time });
                p.send(Msg::Notify(j.to_string()));
            }
        }
    }
}

pub struct ExporterInfo {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
    pub ext: String,
}

pub fn exporters() -> Vec<ExporterInfo> {
    reg()
        .plugins
        .iter()
        .filter(|p| p.running.is_some())
        .flat_map(|p| {
            p.manifest.exporters.iter().map(|e| ExporterInfo {
                plugin: p.manifest.id.clone(),
                plugin_name: p.manifest.name.clone(),
                id: e.id.clone(),
                name: e.name.clone(),
                ext: e.ext.clone(),
            })
        })
        .collect()
}

/// Plugin exporter: takes the hunt records JSON, the plugin returns the file contents, then the user picks where to save.
pub fn run_exporter(e: &ExporterInfo, hunts: &[crate::records::Hunt]) {
    let input = serde_json::json!({ "exporter": e.id, "hunts": serde_json::from_str::<Value>(&crate::records::to_json(hunts)).unwrap_or(Value::Null) });
    let (tx, rx) = channel();
    {
        let r = reg();
        let Some(p) = r.plugins.iter().find(|p| p.manifest.id == e.plugin) else {
            return;
        };
        p.send(Msg::Export {
            input: input.to_string(),
            reply: tx,
        });
    }
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(bytes)) => {
            let ext = e.ext.trim_start_matches('.');
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!("rustyhunt-hunts.{ext}"))
                .add_filter(&e.name, &[ext])
                .save_file()
            {
                match std::fs::write(&path, bytes) {
                    Ok(()) => log::info!("导出到 {}", path.display()),
                    Err(err) => log::warn!("导出失败：{err}"),
                }
            }
        }
        Ok(Err(err)) => log::warn!(target: "plugin", "{} 导出失败：{err}", e.plugin_name),
        Err(_) => log::warn!(target: "plugin", "{} 导出超时", e.plugin_name),
    }
}

// ---- For the Plugins page ----

/// A copy for display on the Plugins page.
#[derive(Clone)]
pub struct View {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub status: Status,
    pub enabled: bool,
    pub granted: BTreeSet<String>,
}

pub fn views() -> Vec<View> {
    let Some(shared) = SHARED.get() else { return Vec::new() };
    let r = reg();
    let s = shared.settings();
    r.plugins
        .iter()
        .map(|p| {
            let st = s.plugins.get(&p.manifest.id);
            View {
                manifest: p.manifest.clone(),
                dir: p.dir.clone(),
                status: p.status.clone(),
                enabled: st.is_some_and(|x| x.enabled) && p.running.is_some(),
                granted: st.map(|x| x.granted.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

pub fn set_enabled(id: &str, on: bool) {
    let Some(shared) = SHARED.get() else { return };
    shared.edit_settings(|s| s.plugins.entry(id.to_string()).or_default().enabled = on);
    let mut r = reg();
    let out = r.out.clone();
    let Some(p) = r.plugins.iter_mut().find(|p| p.manifest.id == id) else {
        return;
    };
    if on {
        if p.running.is_none()
            && let Some(out) = out
        {
            if let Err(e) = compatible(&p.manifest) {
                p.status = Status::Incompatible(e);
                return;
            }
            p.status = Status::Off;
            start(shared, p, out);
        }
    } else {
        stop(p);
        p.status = Status::Off;
        log::info!(target: "plugin", "{} 已停用", p.label());
    }
}

/// Grants or revokes a permission; a running plugin is restarted so it takes effect.
pub fn set_permission(id: &str, perm: Permission, on: bool) {
    let Some(shared) = SHARED.get() else { return };
    shared.edit_settings(|s| {
        let st = s.plugins.entry(id.to_string()).or_default();
        if on {
            st.granted.insert(perm.key().to_string());
            st.confirmed.insert(perm.key().to_string());
        } else {
            st.granted.remove(perm.key());
        }
    });
    restart(id);
}

fn restart(id: &str) {
    let Some(shared) = SHARED.get() else { return };
    let mut r = reg();
    let out = r.out.clone();
    if let Some(p) = r.plugins.iter_mut().find(|p| p.manifest.id == id)
        && p.running.is_some()
        && let Some(out) = out
    {
        stop(p);
        start(shared, p, out);
    }
}

pub fn set_setting(id: &str, key: &str, value: toml::Value) {
    let Some(shared) = SHARED.get() else { return };
    let st = shared.edit_settings(|s| {
        let st = s.plugins.entry(id.to_string()).or_default();
        st.settings.insert(key.to_string(), value);
        st.clone()
    });
    let r = reg();
    if let Some(p) = r.plugins.iter().find(|p| p.manifest.id == id) {
        p.send(Msg::Settings(settings_json(&p.manifest, Some(&st))));
    }
}

/// Deletes a plugin: stops it, deletes its folder and removes its modules from the profiles.
pub fn uninstall(id: &str) -> Result<(), String> {
    let Some(shared) = SHARED.get() else { return Ok(()) };
    let dir = {
        let mut r = reg();
        let Some(i) = r.plugins.iter().position(|p| p.manifest.id == id) else {
            return Ok(());
        };
        let mut p = r.plugins.remove(i);
        stop(&mut p);
        p.dir
    };
    // Files may still be open until the thread exits
    let mut last = Ok(());
    for _ in 0..10 {
        last = std::fs::remove_dir_all(&dir);
        if last.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let prefix = format!("{id}.");
    shared.edit_settings(|s| {
        s.plugins.remove(id);
        s.hotkeys.plugin.retain(|k, _| !k.starts_with(&prefix));
        s.notify.channels.retain(|k, _| !k.starts_with(&prefix));
        for l in s.games.values_mut() {
            for prof in l.profiles.values_mut() {
                prof.modules.retain(|k, _| !k.starts_with(&prefix));
            }
        }
    });
    log::info!(target: "plugin", "已删除 {id}");
    last.map_err(|e| e.to_string())
}

/// A plugin read from a zip or folder; its permissions are shown to the user before installing.
pub struct Staged {
    pub manifest: Manifest,
    /// Folder containing plugin.toml
    root: PathBuf,
    /// Temporary folder used for extraction
    dir: PathBuf,
    temp: bool,
}

impl Drop for Staged {
    fn drop(&mut self) {
        if self.temp {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn unzip(file: &Path, to: &Path) -> Result<(), String> {
    let f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = e.enclosed_name() else {
            return Err("压缩包里有不安全的路径".into());
        };
        total += e.size();
        if total > 64 * 1024 * 1024 {
            return Err("插件包超过 64 MB".into());
        }
        let out = to.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut e, &mut w).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Finds the manifest: at the package root, or in the only folder under the root.
fn manifest_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("plugin.toml").exists() {
        return Some(dir.to_path_buf());
    }
    let subs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    match subs.as_slice() {
        [one] if one.join("plugin.toml").exists() => Some(one.clone()),
        _ => None,
    }
}

pub fn prepare(path: &Path) -> Result<Staged, String> {
    let (dir, temp) = if path.is_dir() {
        (path.to_path_buf(), false)
    } else {
        let tmp = paths::plugins().join(format!(".staging-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        (tmp, true)
    };
    let fail = |e: String| -> Result<Staged, String> {
        if temp {
            let _ = std::fs::remove_dir_all(&dir);
        }
        Err(e)
    };
    if temp && let Err(e) = unzip(path, &dir) {
        return fail(e);
    }
    let Some(root) = manifest_root(&dir) else {
        return fail("没有 plugin.toml".into());
    };
    let manifest = match Manifest::load(&root) {
        Ok(m) => m,
        Err(e) => return fail(e),
    };
    if !root.join(&manifest.entry).exists() {
        return fail(format!("没有 {}", manifest.entry));
    }
    if let Err(e) = compatible(&manifest) {
        return fail(e);
    }
    Ok(Staged { manifest, root, dir, temp })
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()))?;
        } else {
            std::fs::copy(&p, to.join(e.file_name()))?;
        }
    }
    Ok(())
}

/// After the user confirms the permissions, installs into `plugins/<id>`, grants all permissions in the
/// manifest and enables it. An older version with the same id is replaced; its `data` folder is kept.
pub fn install(staged: Staged) -> Result<(), String> {
    let Some(shared) = SHARED.get() else {
        return Err("插件系统没有启动".into());
    };
    let m = staged.manifest.clone();
    let src = staged.root.clone();
    let dest = paths::plugins().join(&m.id);
    {
        let mut r = reg();
        if let Some(i) = r.plugins.iter().position(|p| p.manifest.id == m.id) {
            let mut p = r.plugins.remove(i);
            stop(&mut p);
        }
    }
    let keep = dest.join("data");
    let backup = paths::plugins().join(format!(".data-{}", m.id));
    if keep.exists() {
        let _ = std::fs::remove_dir_all(&backup);
        std::thread::sleep(Duration::from_millis(100));
        std::fs::rename(&keep, &backup).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir(&src, &dest).map_err(|e| e.to_string())?;
    if backup.exists() {
        let _ = std::fs::remove_dir_all(dest.join("data"));
        let _ = std::fs::rename(&backup, dest.join("data"));
    }
    drop(staged);
    shared.edit_settings(|s| {
        let st = s.plugins.entry(m.id.clone()).or_default();
        st.enabled = true;
        for p in &m.permissions {
            st.granted.insert(p.key().to_string());
            st.confirmed.insert(p.key().to_string());
        }
    });
    log::info!(target: "plugin", "已安装 {} {}", m.name, m.version);
    let mut p = load_dir(&dest);
    let mut r = reg();
    if p.status == Status::Off
        && let Some(out) = r.out.clone()
    {
        start(shared, &mut p, out);
    }
    r.plugins.push(p);
    r.plugins.sort_by(|a, b| a.dir.cmp(&b.dir));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, data) in files {
                w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn prepare_reads_zip_and_rejects_bad_packages() {
        let tmp = std::env::temp_dir().join(format!("rh-plugins-{}", std::process::id()));
        // The data directory can be set only once; skip if another test set it first, so nothing is written to the real data directory
        if paths::init(Some(tmp.clone())) != tmp.as_path() {
            return;
        }
        std::fs::create_dir_all(&tmp).unwrap();
        let ex = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/split-timer");
        let manifest = std::fs::read(ex.join("plugin.toml")).unwrap();
        let template = std::fs::read(ex.join("split.xml")).unwrap();

        let good = tmp.join("good.zip");
        std::fs::write(
            &good,
            zip_of(&[
                ("split-timer/plugin.toml", &manifest),
                ("split-timer/split.xml", &template),
                ("split-timer/plugin.wasm", b"\0asm"),
            ]),
        )
        .unwrap();
        let s = prepare(&good).unwrap();
        assert_eq!(s.manifest.id, "split-timer");
        assert!(s.root.join("split.xml").is_file());
        let staging = s.dir.clone();
        drop(s);
        assert!(!staging.exists(), "临时文件夹要删掉");

        let no_wasm = tmp.join("no-wasm.zip");
        std::fs::write(&no_wasm, zip_of(&[("plugin.toml", &manifest)])).unwrap();
        assert!(prepare(&no_wasm).err().unwrap().contains("plugin.wasm"));

        let empty = tmp.join("empty.zip");
        std::fs::write(&empty, zip_of(&[("readme.txt", b"hi")])).unwrap();
        assert!(prepare(&empty).is_err());
        let _ = std::fs::remove_dir_all(tmp);
    }
}
