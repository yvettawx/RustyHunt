//! Reader thread: finds the game process, picks the address map for its version, reads memory at
//! the configured rate, and hands snapshots and events to the overlay, records and plugins. Reading and drawing run on separate threads.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game_world::WorldReader;
use platform::process::Process;
use platform::window::{self, Window};
use rh_core::Event;
use rh_core::i18n;
use rh_core::model::{Game, Snapshot, Target};
use rh_core::settings::TargetMode;

use crate::notify::{Kind, Notification};
use crate::state::{Connection, Live, Shared};

/// Consumers of each frame's snapshot and events: hunt records, plugins, injection.
pub trait Sink: Send {
    fn frame(&mut self, shared: &Shared, snap: &Arc<Snapshot>, events: &[Event]);
    fn attached(&mut self, _shared: &Shared, _pid: u32) {}
    fn detached(&mut self, _shared: &Shared) {}
}

enum Phase {
    Idle,
    Attached {
        reader: Box<WorldReader<Process>>,
        window: Window,
        pid: u32,
    },
    Mismatch {
        pid: u32,
        version: String,
        since: Instant,
    },
}

pub fn map_dirs() -> Vec<PathBuf> {
    // Separately updated maps take priority
    vec![rh_core::paths::maps(), rh_core::paths::bundled("maps")]
}

pub struct Scanner {
    shared: Arc<Shared>,
    sinks: Vec<Box<dyn Sink>>,
    phase: Phase,
    demo: Option<game_world::demo::Demo>,
    last_search: Option<Instant>,
    last_dps_sample: f64,
    manual_target: usize,
    warned: std::collections::HashMap<String, Instant>,
    mismatch_notified: Option<String>,
    hit_times: std::collections::HashMap<u64, f64>,
}

impl Scanner {
    pub fn new(shared: Arc<Shared>, sinks: Vec<Box<dyn Sink>>) -> Scanner {
        Scanner {
            shared,
            sinks,
            phase: Phase::Idle,
            demo: None,
            last_search: None,
            last_dps_sample: 0.0,
            manual_target: 0,
            warned: Default::default(),
            mismatch_notified: None,
            hit_times: Default::default(),
        }
    }

    pub fn spawn(mut self) -> std::thread::JoinHandle<()> {
        std::thread::Builder::new()
            .name("scanner".into())
            .spawn(move || self.run())
            .expect("spawn the reader thread")
    }

    fn run(&mut self) {
        log::info!("reader thread started");
        let mut timing_at = Instant::now();
        while !self.shared.quit.load(std::sync::atomic::Ordering::Relaxed) {
            // Once a minute while attached: how fast reading and drawing really run, for lag reports
            if timing_at.elapsed() >= Duration::from_secs(60) {
                let secs = timing_at.elapsed().as_secs_f32();
                timing_at = Instant::now();
                let frames = self.shared.overlay_frames.swap(0, std::sync::atomic::Ordering::Relaxed);
                if matches!(self.phase, Phase::Attached { .. }) {
                    let live = self.shared.live();
                    log::info!(
                        "timing: reading {:.0} Hz, {:.1} ms a read; overlay {:.0} fps, {:.1} ms a frame; game in front {}",
                        live.actual_hz,
                        live.read_ms,
                        frames as f32 / secs,
                        self.shared.frame_us.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0,
                        live.game_foreground
                    );
                }
            }
            let start = Instant::now();
            let hz = self.shared.settings().perf.read_hz.clamp(5, 60);
            self.tick();
            let period = Duration::from_secs_f64(1.0 / hz as f64);
            if let Some(rest) = period.checked_sub(start.elapsed()) {
                std::thread::sleep(rest);
            }
            let real = start.elapsed().as_secs_f32();
            if real > 0.0 {
                let mut live = self.shared.live.write().unwrap();
                live.actual_hz = live.actual_hz * 0.9 + (1.0 / real) * 0.1;
            }
        }
        if let Phase::Attached { .. } = self.phase {
            for s in &mut self.sinks {
                s.detached(&self.shared);
            }
        }
    }

    fn tick(&mut self) {
        match &mut self.phase {
            Phase::Idle => {
                if DEMO_SCENE.get().is_none() && self.last_search.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
                    self.last_search = Some(Instant::now());
                    self.search();
                }
                if matches!(self.phase, Phase::Idle) {
                    self.demo_tick();
                }
            }
            Phase::Mismatch { pid, version, since } => {
                let (pid, version) = (*pid, version.clone());
                if !platform::process::find_process(Game::World.exe()).is_some_and(|p| p == pid) {
                    self.detach("game exited");
                } else if since.elapsed() > Duration::from_secs(10) {
                    // The map may have just been updated separately; try again
                    if game_world::attach::load_map(&map_dirs(), &version).is_ok() {
                        log::info!("found the address map for {version}, reconnecting");
                        self.phase = Phase::Idle;
                        self.last_search = None;
                    } else if let Phase::Mismatch { since, .. } = &mut self.phase {
                        *since = Instant::now();
                    }
                }
            }
            Phase::Attached { .. } => self.read(),
        }
    }

    fn demo_tick(&mut self) {
        let demo_on = self.shared.settings().demo;
        if !demo_on {
            if self.demo.take().is_some() {
                let mut live = self.shared.live.write().unwrap();
                *live = Live::default();
                drop(live);
                self.shared.repaint();
            }
            return;
        }
        let hz = self.shared.settings().perf.read_hz.clamp(5, 60) as f32;
        let demo = self
            .demo
            .get_or_insert_with(|| DEMO_SCENE.get().and_then(|s| game_world::demo::Demo::scene(s)).unwrap_or_default());
        let snap = demo.step(1.0 / hz);
        self.publish(snap, Connection::Demo);
    }

    fn search(&mut self) {
        let Some(pid) = platform::process::find_process(Game::World.exe()) else {
            let mut live = self.shared.live.write().unwrap();
            if !matches!(live.connection, Connection::Demo) {
                live.connection = Connection::Searching;
            }
            return;
        };
        let Some(win) = window::main_window(pid) else { return };
        let title = win.title();
        let Some(version) = game_world::attach::version_from_title(&title) else {
            // Right after launch the title has no version yet
            log::debug!("no version in window title {title}, retrying later");
            return;
        };
        self.demo = None;

        let (map, map_path) = match game_world::attach::load_map(&map_dirs(), &version) {
            Ok(m) => m,
            Err(address_map::LoadError::NotFound { .. }) => {
                let dirs = map_dirs();
                let known = game_world::attach::known_versions(&dirs.iter().map(|d| d.as_path()).collect::<Vec<_>>());
                log::warn!("no address map for game version {version}, reading stopped");
                {
                    let mut live = self.shared.live.write().unwrap();
                    *live = Live::default();
                    live.connection = Connection::MapMismatch {
                        version: version.clone(),
                        latest: known.last().cloned(),
                    };
                    live.pid = Some(pid);
                    live.exe = Game::World.exe().into();
                    live.version = Some(version.clone());
                }
                if self.mismatch_notified.as_deref() != Some(&version) && self.shared.settings().notify.map_mismatch {
                    self.mismatch_notified = Some(version.clone());
                    self.shared.notify(Notification::new(
                        Kind::MapMismatch { version: version.clone() },
                        i18n::tf("notify.map_mismatch", &[("version", &version)]),
                        i18n::t("notify.map_mismatch_body"),
                    ));
                }
                self.phase = Phase::Mismatch {
                    pid,
                    version,
                    since: Instant::now(),
                };
                self.shared.repaint();
                return;
            }
            Err(e) => {
                log::error!("failed to read the address map: {e}");
                self.shared.live.write().unwrap().connection = Connection::Failed(e.to_string());
                return;
            }
        };

        let process = match Process::open(pid) {
            Ok(p) => p,
            Err(e) => {
                log::error!("failed to open process {pid}: {e}");
                self.shared.live.write().unwrap().connection = Connection::Failed(e.to_string());
                return;
            }
        };
        let Some(module) = process.module(Game::World.exe()) else {
            log::warn!("main module not found yet, retrying later");
            return;
        };
        let reader = WorldReader::new(process, map, module.base);
        if let Err(missing) = reader.check_map() {
            log::error!("address map {} is missing {}", map_path.display(), missing.0);
            self.shared.live.write().unwrap().connection = Connection::Failed(format!("address map is missing {}", missing.0));
            return;
        }
        log::info!(
            "attached to {}, PID {pid}, version {version}, address map {} ({} entries), base 0x{:X}",
            Game::World.exe(),
            map_path.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
            reader.map().len(),
            module.base
        );
        // Crash logs should include the game version
        crate::crash::set_game_version(&version);
        {
            let mut live = self.shared.live.write().unwrap();
            *live = Live {
                connection: Connection::Connected,
                pid: Some(pid),
                exe: Game::World.exe().into(),
                version: Some(version),
                attached_at: Some(Instant::now()),
                ..Live::default()
            };
        }
        for s in &mut self.sinks {
            s.attached(&self.shared, pid);
        }
        self.phase = Phase::Attached {
            reader: Box::new(reader),
            window: win,
            pid,
        };
        self.shared.repaint();
    }

    fn detach(&mut self, why: &str) {
        log::info!("detached: {why}");
        let was_attached = matches!(self.phase, Phase::Attached { .. });
        self.phase = Phase::Idle;
        self.mismatch_notified = None;
        *self.shared.live.write().unwrap() = Live::default();
        if was_attached {
            for s in &mut self.sinks {
                s.detached(&self.shared);
            }
            if self.shared.settings().notify.disconnect {
                self.shared.notify(Notification::new(
                    Kind::Disconnected,
                    i18n::t("notify.disconnected"),
                    i18n::t("notify.disconnected_body"),
                ));
            }
        }
        self.shared.repaint();
    }

    fn read(&mut self) {
        let Phase::Attached { reader, window, pid } = &mut self.phase else { return };
        let pid = *pid;
        if !window.is_valid() || !reader.memory().is_alive() {
            self.detach("game exited");
            return;
        }
        let rect = window.client_rect();
        let foreground = window.is_foreground();
        let minimized = window.is_minimized();
        let prev = self.shared.live().snapshot.clone();
        let t0 = Instant::now();
        let now = self.shared.now();
        let snap = reader.read(&prev, now);
        let read_ms = t0.elapsed().as_secs_f32() * 1000.0;

        // Read failures only go to the log, at most once a minute per kind
        let warnings = std::mem::take(&mut reader.warnings);
        for w in warnings {
            let key = w.split('：').next().unwrap_or(&w).to_string();
            if self.warned.get(&key).is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
                log::warn!("{w}, frame skipped");
                self.warned.insert(key, Instant::now());
            }
            self.shared.live.write().unwrap().errors += 1;
        }

        let switched = {
            let mut live = self.shared.live.write().unwrap();
            let switched = live.game_foreground != foreground;
            live.game_rect = rect;
            live.game_foreground = foreground;
            live.game_minimized = minimized;
            live.read_ms = live.read_ms * 0.8 + read_ms * 0.2;
            live.pid = Some(pid);
            switched
        };
        if switched {
            // The overlay hides while the game is in the background, even if no data changed; the main
            // window, held while the game was in front, catches up
            self.shared.repaint();
        }
        self.publish(snap, Connection::Connected);
    }

    /// Picks the target, computes combo damage and the DPS curve, compares events, then publishes.
    fn publish(&mut self, mut snap: Snapshot, connection: Connection) {
        let now = self.shared.now();
        let prev = self.shared.live().snapshot.clone();
        let mode = self.shared.settings().profile(Game::World).monster.target_mode;

        if let Some(split) = crate::injection::damage_split() {
            // Solo, our damage is estimated from monster HP loss; with the hook use the exact value
            if !snap.online
                && split.total > 0.0
                && let Some(me) = snap.party.iter_mut().find(|p| p.is_self)
            {
                me.damage = split.total;
            }
            snap.damage_split = Some(split);
        }

        for m in &snap.monsters {
            if let Some(p) = prev.monster(m.uid)
                && m.hp < p.hp
            {
                self.hit_times.insert(m.uid, now);
            }
        }
        self.hit_times.retain(|uid, _| snap.monster(*uid).is_some());

        // The "switch target" action is counted on the main thread; read the result here
        let manual = self.shared.live().target;
        let target = select_target(&snap, mode, manual, &self.hit_times, &mut self.manual_target);

        let events = rh_core::events::diff(&prev, &snap);
        {
            let mut log = self.shared.events.lock().unwrap();
            for e in &events {
                log.push(now, e.clone());
                log::debug!("event {e:?}");
            }
        }

        let changed = *prev != snap;
        let arc = Arc::new(snap);
        {
            let mut live = self.shared.live.write().unwrap();
            if connection == Connection::Demo && live.connection != Connection::Demo {
                *live = Live::default();
            }
            live.connection = connection;
            if let Some(t) = target
                && let (Some(old), Some(new)) = (prev.monster(t), arc.monster(t))
                && new.hp < old.hp
            {
                if now - live.combo_at > 1.6 {
                    live.combo_damage = 0.0;
                }
                live.combo_damage += old.hp - new.hp;
                live.combo_at = now;
            }
            if mode != TargetMode::Manual || live.target.is_none_or(|t| arc.monster(t).is_none()) {
                live.target = target;
            }
            if !arc.in_quest() && !arc.in_hunt_zone {
                live.dps_history.clear();
                live.combat = Default::default();
                self.last_dps_sample = 0.0;
            } else {
                live.combat.update(arc.elapsed, arc.party.iter().map(|p| p.damage).sum());
            }
            if (arc.in_quest() || arc.in_hunt_zone) && arc.elapsed as f64 - self.last_dps_sample >= 2.0 {
                self.last_dps_sample = arc.elapsed as f64;
                let mine = live.combat.dps(arc.self_member().map_or(0.0, |m| m.damage));
                live.dps_history.push(mine);
                if live.dps_history.len() > 600 {
                    live.dps_history.remove(0);
                }
            }
            live.snapshot = arc.clone();
        }

        for s in &mut self.sinks {
            s.frame(&self.shared, &arc, &events);
        }
        if changed || !events.is_empty() {
            self.shared.repaint_overlay();
            if !events.is_empty() || (now * 2.0).fract() < 0.1 {
                self.shared.repaint();
            }
        }
    }
}

/// Set for screenshots: demo data only, never the game; a named scene (`--screenshot overlay:<scene>`)
/// instead of the random hunt.
pub static DEMO_SCENE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Picks the target monster by target mode. No monster is picked until the player selects one
/// (locks on in game, switches manually, or hits one in last-hit mode).
pub fn select_target(snap: &Snapshot, mode: TargetMode, manual: Option<u64>, hits: &std::collections::HashMap<u64, f64>, _cursor: &mut usize) -> Option<u64> {
    let alive: Vec<&rh_core::model::Monster> = snap.large_monsters().filter(|m| m.alive()).collect();
    let locked = || {
        alive
            .iter()
            .find(|m| m.target == Target::Own)
            .or_else(|| alive.iter().find(|m| m.manual_target == Target::Own))
            .map(|m| m.uid)
    };
    match mode {
        TargetMode::Lock => locked(),
        TargetMode::LastHit => hits
            .iter()
            .filter(|(uid, _)| alive.iter().any(|m| m.uid == **uid))
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(uid, _)| *uid)
            .or_else(locked),
        TargetMode::Manual => manual.filter(|t| snap.monster(*t).is_some()),
    }
}

/// Manual switch: from the current target to the next living large monster.
pub fn next_target(snap: &Snapshot, current: Option<u64>) -> Option<u64> {
    let list: Vec<u64> = snap.large_monsters().filter(|m| m.alive()).map(|m| m.uid).collect();
    if list.is_empty() {
        return None;
    }
    let i = current.and_then(|c| list.iter().position(|&u| u == c)).map_or(0, |i| (i + 1) % list.len());
    Some(list[i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rh_core::model::Monster;

    fn snap() -> Snapshot {
        let m = |uid, t| Monster {
            uid,
            hp: 100.0,
            hp_max: 100.0,
            large: true,
            target: t,
            ..Default::default()
        };
        Snapshot {
            monsters: vec![m(1, Target::Other), m(2, Target::Own), m(3, Target::Other)],
            ..Default::default()
        }
    }

    #[test]
    fn lock_mode_prefers_locked_monster() {
        let s = snap();
        let mut c = 0;
        assert_eq!(select_target(&s, TargetMode::Lock, None, &Default::default(), &mut c), Some(2));
    }

    #[test]
    fn nothing_selected_means_no_target() {
        let mut s = snap();
        s.monsters[1].target = Target::Other;
        let mut c = 0;
        assert_eq!(select_target(&s, TargetMode::Lock, None, &Default::default(), &mut c), None);
        assert_eq!(select_target(&s, TargetMode::LastHit, None, &Default::default(), &mut c), None);
        assert_eq!(select_target(&s, TargetMode::Manual, None, &Default::default(), &mut c), None);
    }

    #[test]
    fn last_hit_mode() {
        let s = snap();
        let mut hits = std::collections::HashMap::new();
        hits.insert(3, 10.0);
        hits.insert(1, 5.0);
        let mut c = 0;
        assert_eq!(select_target(&s, TargetMode::LastHit, None, &hits, &mut c), Some(3));
    }

    #[test]
    fn manual_cycles() {
        let s = snap();
        assert_eq!(next_target(&s, None), Some(1));
        assert_eq!(next_target(&s, Some(1)), Some(2));
        assert_eq!(next_target(&s, Some(3)), Some(1));
    }
}
