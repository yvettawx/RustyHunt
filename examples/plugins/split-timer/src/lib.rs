//! Example plugin: split timer.
//!
//! Shows every part of the plugin interface: subscribing to events, receiving data snapshots,
//! registering an in-game module, hotkey actions, settings, reading and writing its own folder,
//! writing fields to hunt records, and registering an exporter. The interface is documented in
//! `docs/plugins.md` in the repository.

use std::cell::RefCell;

use serde_json::{Value, json};

#[link(wasm_import_module = "rh")]
unsafe extern "C" {
    fn log(level: i32, ptr: *const u8, len: i32);
    fn set_module_data(mp: *const u8, ml: i32, jp: *const u8, jl: i32);
    fn set_record_field(kp: *const u8, kl: i32, vp: *const u8, vl: i32);
    fn fs_read(pp: *const u8, pl: i32) -> i64;
    fn fs_write(pp: *const u8, pl: i32, dp: *const u8, dl: i32) -> i32;
}

// ---- Exchanging data with the host ----

/// The host calls this for a block of memory before writing data into the plugin; the block belongs to the plugin, which frees it when done.
#[unsafe(no_mangle)]
pub extern "C" fn rh_alloc(len: i32) -> *mut u8 {
    let b = vec![0u8; len.max(0) as usize].into_boxed_slice();
    Box::into_raw(b) as *mut u8
}

/// Takes back the bytes the host wrote and frees them.
fn take(ptr: *mut u8, len: i32) -> Vec<u8> {
    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len.max(0) as usize)) }.into_vec()
}

fn take_str(ptr: *mut u8, len: i32) -> String {
    String::from_utf8_lossy(&take(ptr, len)).into_owned()
}

/// (ptr << 32) | len returned by host functions; negative means failure.
fn unpack(r: i64) -> Option<Vec<u8>> {
    (r >= 0).then(|| take((r >> 32) as usize as *mut u8, (r & 0xFFFF_FFFF) as i32))
}

fn info(text: &str) {
    unsafe { log(2, text.as_ptr(), text.len() as i32) }
}

fn read_file(name: &str) -> Option<Vec<u8>> {
    unpack(unsafe { fs_read(name.as_ptr(), name.len() as i32) })
}

fn write_file(name: &str, data: &[u8]) {
    let r = unsafe { fs_write(name.as_ptr(), name.len() as i32, data.as_ptr(), data.len() as i32) };
    if r != 0 {
        info("写文件失败");
    }
}

fn record_field(key: &str, value: &str) {
    unsafe { set_record_field(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32) }
}

// ---- Splits ----

struct Split {
    name: String,
    t: f64,
}

#[derive(Default)]
struct Timer {
    quest: i64,
    running: bool,
    elapsed: f64,
    splits: Vec<Split>,
    /// Split times of this quest's best run
    best: Vec<f64>,
    stage_split: bool,
    rows: usize,
    /// What the exporter returned, freed on the next call
    export: Vec<u8>,
}

thread_local! {
    static T: RefCell<Timer> = RefCell::new(Timer { rows: 5, ..Default::default() });
}

fn clock(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0) as i64;
    format!("{m}:{:04.1}", t - m as f64 * 60.0)
}

fn best_file(quest: i64) -> String {
    format!("best-{quest}.json")
}

impl Timer {
    fn split(&mut self, name: &str, t: f64) {
        if !self.running {
            return;
        }
        self.splits.push(Split { name: name.to_string(), t });
        self.publish();
    }

    /// Hands the lines to display to the host, which draws them in the current theme.
    fn publish(&self) {
        let start = self.splits.len().saturating_sub(self.rows);
        let rows: Vec<Value> = self.splits[start..]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (diff, color) = match self.best.get(start + i) {
                    Some(b) => {
                        let d = s.t - b;
                        let sign = if d < 0.0 { "-" } else { "+" };
                        (format!("{sign}{:.1}", d.abs()), if d < 0.0 { "ok" } else { "danger" })
                    }
                    None => (String::new(), "text"),
                };
                json!({ "name": s.name, "time": clock(s.t), "diff": diff, "color": color })
            })
            .collect();
        let data = json!({ "rows": rows }).to_string();
        let module = "split";
        unsafe { set_module_data(module.as_ptr(), module.len() as i32, data.as_ptr(), data.len() as i32) }
    }

    fn finish(&mut self, success: bool, t: f64) {
        if !self.running {
            return;
        }
        self.split("任务结束", t);
        self.running = false;
        record_field("splits", &self.splits.len().to_string());
        let better = self.best.last().is_none_or(|b| t < *b);
        if success && better {
            let times: Vec<f64> = self.splits.iter().map(|s| s.t).collect();
            write_file(&best_file(self.quest), json!(times).to_string().as_bytes());
            info(&format!("任务 {} 新的最好成绩 {}", self.quest, clock(t)));
            self.best = times;
        }
        if let Some(b) = self.best.last() {
            record_field("best", &clock(*b));
        }
    }
}

fn with<R>(f: impl FnOnce(&mut Timer) -> R) -> R {
    T.with(|t| f(&mut t.borrow_mut()))
}

fn apply_settings(v: &Value) {
    with(|t| {
        t.stage_split = v["stage_split"].as_bool().unwrap_or(false);
        t.rows = v["rows"].as_f64().unwrap_or(5.0).clamp(1.0, 10.0) as usize;
        t.publish();
    });
}

// ---- Entry points called by the host ----

#[unsafe(no_mangle)]
pub extern "C" fn rh_init(ptr: *mut u8, len: i32) {
    let v: Value = serde_json::from_slice(&take(ptr, len)).unwrap_or_default();
    apply_settings(&v);
}

#[unsafe(no_mangle)]
pub extern "C" fn rh_settings(ptr: *mut u8, len: i32) {
    rh_init(ptr, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn rh_event(ptr: *mut u8, len: i32) {
    let e: Value = serde_json::from_slice(&take(ptr, len)).unwrap_or_default();
    let t = e["elapsed"].as_f64().unwrap_or(0.0);
    with(|s| match e["type"].as_str().unwrap_or("") {
        "quest_start" => {
            s.quest = e["quest_id"].as_i64().unwrap_or(0);
            s.splits.clear();
            s.running = true;
            s.best = read_file(&best_file(s.quest))
                .and_then(|b| serde_json::from_slice::<Vec<f64>>(&b).ok())
                .unwrap_or_default();
            s.publish();
        }
        "quest_end" => s.finish(e["status"] == "success", e["elapsed"].as_f64().unwrap_or(s.elapsed)),
        "part_break" => s.split("部位破坏", t),
        "monster_death" => s.split("讨伐", t),
        "monster_capture" => s.split("捕获", t),
        "stage_change" if s.stage_split => s.split("区域切换", t),
        _ => {}
    });
}

/// Data snapshot: same fields as theme templates; only the quest time is used here (for manual splits).
#[unsafe(no_mangle)]
pub extern "C" fn rh_frame(ptr: *mut u8, len: i32) {
    let f: Value = serde_json::from_slice(&take(ptr, len)).unwrap_or_default();
    with(|s| s.elapsed = f["quest"]["elapsed"].as_f64().unwrap_or(s.elapsed));
}

#[unsafe(no_mangle)]
pub extern "C" fn rh_action(ptr: *mut u8, len: i32) {
    match take_str(ptr, len).as_str() {
        "split" => with(|s| {
            let t = s.elapsed;
            s.split("手动", t)
        }),
        "reset" => with(|s| {
            write_file(&best_file(s.quest), b"[]");
            s.best.clear();
            s.publish();
        }),
        _ => {}
    }
}

/// Exporter: takes `{"exporter": id, "hunts": [...]}` and returns the file contents.
#[unsafe(no_mangle)]
pub extern "C" fn rh_export(ptr: *mut u8, len: i32) -> i64 {
    let v: Value = serde_json::from_slice(&take(ptr, len)).unwrap_or_default();
    let mut md = String::from("| 开始时间 | 怪物 | 结果 | 用时 |\n|---|---|---|---|\n");
    for h in v["hunts"].as_array().into_iter().flatten() {
        md += &format!(
            "| {} | {} | {} | {} |\n",
            h["started_at"].as_i64().unwrap_or(0),
            h["monster"].as_str().unwrap_or(""),
            h["result"].as_str().unwrap_or(""),
            clock(h["duration"].as_f64().unwrap_or(0.0)),
        );
    }
    with(|s| {
        s.export = md.into_bytes();
        ((s.export.as_ptr() as usize as i64) << 32) | s.export.len() as i64
    })
}
