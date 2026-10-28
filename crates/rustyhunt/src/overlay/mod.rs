//! In-game overlay: a transparent, topmost, click-through window over the game's client area.
//! Designed on a 1080-high reference frame, scaled by the game window's height, and aligned by anchor on wide screens.
//! Nothing is redrawn while the picture is still; only data changes or running animations draw the next frame.

mod backing;
mod hunter;
pub mod layout;
mod misc;
mod monster;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use egui::emath::TSTransform;
use egui::{Id, LayerId, Order, Pos2, Rect, Vec2, pos2, vec2};
use rh_core::Event;
use rh_core::i18n::{self, I18n};
use rh_core::model::{Game, Monster, Snapshot};
use rh_core::modules;
use rh_core::settings::{Density, ModuleConfig, Profile, Settings};
use ui_kit::fields::Activity;
use ui_kit::render::Images;
use ui_kit::{Canvas, Theme};

use crate::state::{Connection, Live, Shared};

pub use layout::LayoutState;
pub use misc::SLOT_COLORS;

/// This frame's data used when drawing modules.
pub struct Frame<'a> {
    pub snap: &'a Snapshot,
    pub live: &'a Live,
    pub settings: &'a Settings,
    pub profile: &'a Profile,
    pub i18n: &'a I18n,
    pub density: Density,
    pub now: f64,
    pub game: Game,
    pub target: Option<&'a Monster>,
    pub frame_us: u32,
    /// Layout mode or preview: ignore the display conditions
    pub preview: bool,
    /// Hunter's Notes rows by monster key; empty when the save hasn't been read
    pub notes: &'a HashMap<String, crate::bestiary::Row>,
}

impl Frame<'_> {
    pub fn t(&self, key: &str) -> String {
        self.i18n.t(key)
    }

    pub fn idle_secs(&self) -> f64 {
        self.profile.monster.idle_fade_secs.max(1.0) as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStyle {
    Plain,
    Accent,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub t: f64,
    pub tag: String,
    pub text: String,
    pub style: ToastStyle,
}

/// Overlay state kept between frames.
#[derive(Default)]
pub struct State {
    pub activity: Activity,
    prev: Option<Arc<Snapshot>>,
    /// Monster HP trail
    pub ghost: HashMap<u64, f32>,
    pub player_ghost: f32,
    pub player_hit_at: f64,
    pub toasts: VecDeque<Toast>,
    pub flash: HashMap<String, f64>,
    last_event: f64,
    last_frame: f64,
    pub dt: f32,
    /// Something animated this frame, keep redrawing
    pub animating: bool,
    /// Each module's rect on screen (reference coordinates)
    pub rects: BTreeMap<String, Rect>,
    pub images: Images,
    /// Data for template modules, built once per frame
    pub context: Option<serde_json::Value>,
}

impl State {
    fn track(&mut self, snap: &Arc<Snapshot>, now: f64) {
        let Some(prev) = self.prev.replace(snap.clone()) else { return };
        if Arc::ptr_eq(&prev, snap) {
            return;
        }
        for m in &snap.monsters {
            let Some(p) = prev.monster(m.uid) else {
                self.ghost.insert(m.uid, m.hp);
                continue;
            };
            if m.enraged && !p.enraged {
                self.flash.insert(format!("{}:enrage", m.uid), now);
            }
            for (i, part) in m.parts.iter().enumerate() {
                let Some(old) = p.parts.get(i) else { continue };
                if (part.flinch - old.flinch).abs() > 0.01 || (part.durability - old.durability).abs() > 0.01 || part.breaks != old.breaks {
                    self.activity.touch(format!("{}:p{i}", m.uid), now);
                }
                if part.breaks > old.breaks {
                    self.flash.insert(format!("{}:p{i}", m.uid), now);
                }
            }
            for a in &m.ailments {
                let Some(old) = p.ailments.iter().find(|x| x.key == a.key) else { continue };
                if (a.buildup - old.buildup).abs() > 0.01 || a.count != old.count || a.active() != old.active() {
                    self.activity.touch(format!("{}:a{}", m.uid, a.key), now);
                }
                if a.count > old.count {
                    self.flash.insert(format!("{}:a{}", m.uid, a.key), now);
                }
            }
        }
        if snap.player.hp < prev.player.hp - 0.5 {
            self.player_ghost = self.player_ghost.max(prev.player.hp);
            self.player_hit_at = now;
        }
        self.ghost.retain(|uid, _| snap.monster(*uid).is_some());
    }

    fn take_events(&mut self, shared: &Shared, f: &Frame) {
        let events: Vec<(f64, Event)> = shared
            .events
            .lock()
            .unwrap()
            .items
            .iter()
            .filter(|(t, _)| *t > self.last_event)
            .cloned()
            .collect();
        for (t, e) in events {
            self.last_event = self.last_event.max(t);
            if f.now - t > 3.0 {
                continue;
            }
            let target = f.target.map(|m| m.uid);
            let toast = match &e {
                Event::PartBreak { uid, part, .. } if Some(*uid) == target => Some((f.t("toast.break"), f.i18n.game(part), ToastStyle::Accent)),
                Event::PartSever { uid, part } if Some(*uid) == target => Some((f.t("toast.sever"), f.i18n.game(part), ToastStyle::Accent)),
                Event::AilmentTrigger { uid, ailment, .. } if Some(*uid) == target => Some((f.i18n.game(ailment), String::new(), ToastStyle::Plain)),
                Event::Enrage { uid } if Some(*uid) == target => Some((f.t("toast.enrage"), String::new(), ToastStyle::Accent)),
                Event::Tired { uid } if Some(*uid) == target => Some((f.t("toast.tired"), String::new(), ToastStyle::Plain)),
                Event::Capturable { uid } if Some(*uid) == target => Some((f.t("toast.capturable"), String::new(), ToastStyle::Accent)),
                Event::MonsterDeath { uid, .. } if Some(*uid) == target => Some((f.t("toast.slain"), String::new(), ToastStyle::Accent)),
                Event::MonsterCapture { uid, .. } if Some(*uid) == target => Some((f.t("toast.captured"), String::new(), ToastStyle::Accent)),
                _ => None,
            };
            if let Some((tag, text, style)) = toast {
                self.toasts.push_back(Toast { t, tag, text, style });
                if self.toasts.len() > 4 {
                    self.toasts.pop_front();
                }
            }
        }
    }
}

pub const LAYER_ROOT: &str = "rustyhunt-overlay";

fn module_layer(id: &str) -> LayerId {
    LayerId::new(Order::Foreground, Id::unique((LAYER_ROOT, id)))
}

/// Whether a module should be shown.
fn visible(f: &Frame, cfg: &ModuleConfig) -> bool {
    if !cfg.enabled {
        return false;
    }
    if f.preview {
        return true;
    }
    if cfg.quest_only && !(f.snap.in_quest() || f.snap.in_hunt_zone) {
        return false;
    }
    if cfg.hide_in_menu && f.snap.hud_open {
        return false;
    }
    true
}

const LAYOUT_TITLE: &str = "RustyHunt Layout";

/// For screenshots: draw layout mode into the overlay window itself, which can be captured.
pub static LAYOUT_INLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn layout_viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("rustyhunt-layout")
}

pub struct Overlay {
    pub shared: Arc<Shared>,
    pub state: State,
    pub layout: LayoutState,
    geometry: Option<(i32, i32, i32, i32)>,
    /// Keep repainting until then after a move: a resize drops a frame while the swapchain is reconfigured
    settle_until: Option<std::time::Instant>,
    /// The layout window while layout mode is on, and where it was last placed
    layout_win: Option<(isize, (i32, i32, i32, i32))>,
    hwnd: Option<isize>,
    preview_snap: Option<Arc<Snapshot>>,
}

impl Overlay {
    pub fn new(shared: Arc<Shared>) -> Overlay {
        Overlay {
            shared,
            state: State::default(),
            layout: LayoutState::default(),
            geometry: None,
            settle_until: None,
            layout_win: None,
            hwnd: None,
            preview_snap: None,
        }
    }

    pub fn set_hwnd(&mut self, hwnd: isize) {
        if self.hwnd != Some(hwnd) {
            self.hwnd = Some(hwnd);
            crate::win::style_overlay(hwnd);
        }
    }

    /// Which part of the screen the overlay should cover (physical pixels). None when there is nothing to show.
    fn target_geometry(&self, live: &Live, layout: bool) -> Option<(i32, i32, i32, i32)> {
        if let Some(r) = live.game_rect
            && live.connected()
            && !live.game_minimized
            && r.w > 0
            && r.h > 0
        {
            return Some((r.x, r.y, r.w, r.h));
        }
        if matches!(live.connection, Connection::Demo) || layout {
            let (w, h) = crate::win::primary_screen();
            return Some((0, 0, w, h));
        }
        None
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let t0 = std::time::Instant::now();
        let ctx = ui.ctx().clone();
        let shared = self.shared.clone();
        let layout_on = shared.layout_mode();
        let live = shared.live().clone();
        let settings = shared.settings().clone();

        // Window position
        let geometry = self.target_geometry(&live, layout_on);
        let want = geometry.unwrap_or((-32000, -32000, 1, 1));
        // Moving onto a monitor with a different scale makes winit resize the window
        // to keep its logical size, so check the real rect, not just what was last asked for.
        if let Some(h) = self.hwnd {
            let real = crate::win::window_rect(h);
            if self.geometry != Some(want) || (geometry.is_some() && real != Some(want)) {
                crate::win::place_overlay(h, want);
                self.geometry = Some(want);
                self.settle_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(1));
            }
        }
        if self.settle_until.is_some_and(|t| std::time::Instant::now() < t) {
            ctx.request_repaint();
        }
        // The overlay is topmost: draw nothing over other programs while the game is in the background
        let in_front = live.game_foreground || layout_on || matches!(live.connection, Connection::Demo);
        if geometry.is_none() || (!settings.overlay_visible && !layout_on) || !in_front {
            self.layout_win = None;
            return;
        }

        if !layout_on || LAYOUT_INLINE.load(std::sync::atomic::Ordering::Relaxed) {
            self.layout_win = None;
            self.paint(ui, t0);
            return;
        }
        // Layout mode needs the mouse and keyboard, which the overlay can't take: it has to stay a
        // layered click-through window, since changing that style breaks its DirectComposition
        // output. A plain window on top takes over while layout mode is on, and the overlay stays empty.
        let builder = egui::ViewportBuilder::default()
            .with_title(LAYOUT_TITLE)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_inner_size([1.0, 1.0])
            .with_position([-32000.0, -32000.0]);
        ctx.show_viewport_immediate(layout_viewport(), builder, |vui, _| {
            let first = self.layout_win.is_none();
            if let Some(h) = self.layout_win.map(|w| w.0).or_else(|| crate::win::own_window(LAYOUT_TITLE)) {
                if self.layout_win.is_none_or(|(_, at)| at != want) || crate::win::window_rect(h) != Some(want) {
                    crate::win::place_overlay(h, want);
                    vui.ctx().request_repaint();
                }
                self.layout_win = Some((h, want));
            }
            if first {
                vui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            self.paint(vui, t0);
        });
    }

    /// Draws the modules (and in layout mode the layout tools) into `ui`'s window.
    fn paint(&mut self, ui: &mut egui::Ui, t0: std::time::Instant) {
        let ctx = ui.ctx().clone();
        let shared = self.shared.clone();
        let layout_on = shared.layout_mode();
        let live = shared.live().clone();
        let settings = shared.settings().clone();
        let theme = shared.theme();
        let i18n = i18n::current();
        let now = shared.now();

        let snap = if live.has_data() {
            self.preview_snap = None;
            live.snapshot.clone()
        } else {
            // Entering layout mode without the game: place modules using one frame of demo data
            self.preview_snap
                .get_or_insert_with(|| Arc::new(game_world::demo::Demo::new().step(0.1)))
                .clone()
        };
        self.state.dt = ((now - self.state.last_frame) as f32).clamp(0.0, 0.1);
        self.state.last_frame = now;
        self.state.track(&snap, now);
        self.state.animating = false;

        let game = Game::World;
        let profile = settings.profile(game).clone();
        let mut density = settings.density;
        if shared.show_all.load(std::sync::atomic::Ordering::Relaxed) {
            density = Density::Full;
        }
        // Without a selected monster the monster module stays hidden; layout mode shows one to place it
        let target = live
            .target
            .and_then(|t| snap.monster(t))
            .or_else(|| snap.large_monsters().next().filter(|_| layout_on));
        crate::bestiary::keep_fresh(&shared);
        let notes = if snap.large_monsters().next().is_some() {
            crate::bestiary::by_key()
        } else {
            HashMap::new()
        };
        let f = Frame {
            snap: &snap,
            live: &live,
            settings: &settings,
            profile: &profile,
            i18n: &i18n,
            density,
            now,
            game,
            target,
            frame_us: shared.frame_us.load(std::sync::atomic::Ordering::Relaxed),
            preview: layout_on,
            notes: &notes,
        };
        self.state.take_events(&shared, &f);
        self.state.images.set_dir(theme.dir.clone());

        let screen = ctx.content_rect();
        let s = screen.height() / 1080.0;
        let frame_w = screen.width() / s;

        if layout_on {
            layout::backdrop(&ctx, screen, s, &self.layout);
        }

        // Backing of the player module, around where it was drawn last frame
        let player_cfg = profile.module(modules::PLAYER).cloned().unwrap_or_default();
        if visible(&f, &player_cfg) {
            let module = self
                .state
                .rects
                .get(modules::PLAYER)
                .map(|r| Rect::from_min_max(screen.min + r.min.to_vec2() * s, screen.min + r.max.to_vec2() * s));
            backing::draw(&ctx, &f, module, screen, s);
        }

        let mut ids: Vec<String> = profile.modules.keys().cloned().collect();
        // Monster modules go on top
        ids.sort_by_key(|id| id == modules::MONSTER);
        let mut rects = BTreeMap::new();
        for id in ids {
            let cfg = &profile.modules[&id];
            if !visible(&f, cfg) {
                continue;
            }
            let layer = module_layer(&id);
            // Modules draw at their own origin and the layer is moved into place afterwards, so the clip
            // must not be the screen: that would cut off anything left of or above the origin
            let mut painter = ctx.layer_painter(layer);
            painter.set_clip_rect(Rect::from_min_max(pos2(-8000.0, -8000.0), pos2(16000.0, 16000.0)));
            let scale = s * cfg.scale.clamp(0.5, 2.0);
            let cv = Canvas {
                painter,
                origin: Pos2::ZERO,
                scale,
                opacity: cfg.opacity.clamp(0.1, 1.0),
                theme: &theme,
            };
            let size = self.draw_module(&id, &cv, &f, cfg, &theme, &ctx);
            if size == Vec2::ZERO {
                continue;
            }
            let size_design = size * cfg.scale.clamp(0.5, 2.0);
            let (x, y) = cfg.placement.to_top_left(frame_w, (size_design.x, size_design.y));
            let offset = screen.min + vec2(x, y) * s;
            ctx.set_transform_layer(layer, TSTransform::from_translation(offset.to_vec2()));
            rects.insert(id.clone(), Rect::from_min_size(pos2(x, y), size_design));
        }
        self.state.rects = rects;

        if layout_on {
            layout::interact(ui, &shared, &mut self.layout, &self.state.rects, screen, s, frame_w, game);
        }

        shared.overlay_frames.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let us = t0.elapsed().as_micros() as u32;
        let prev = shared.frame_us.load(std::sync::atomic::Ordering::Relaxed);
        shared
            .frame_us
            .store(if prev == 0 { us } else { (prev * 7 + us) / 8 }, std::sync::atomic::Ordering::Relaxed);

        // While animating keep drawing up to the frame cap; when still, stop requesting repaints
        if self.state.animating || layout_on || !self.state.toasts.is_empty() {
            let fps = settings.perf.fps_cap.clamp(30, 144) as f32;
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.0 / fps));
        }
        while self.state.toasts.front().is_some_and(|t| now - t.t > theme.tokens.duration("toast") as f64) {
            self.state.toasts.pop_front();
        }
    }

    fn draw_module(&mut self, id: &str, cv: &Canvas, f: &Frame, cfg: &ModuleConfig, theme: &Theme, ctx: &egui::Context) -> Vec2 {
        draw_any(id, cv, f, cfg, theme, ctx, &mut self.state)
    }

    pub fn end_frame(&mut self) {
        self.state.context = None;
    }
}

/// Draws one module; shared by the overlay and the preview on the main window's Modules page.
pub fn draw_any(id: &str, cv: &Canvas, f: &Frame, cfg: &ModuleConfig, theme: &Theme, ctx: &egui::Context, st: &mut State) -> Vec2 {
    {
        // If the theme has a template for this module, draw from it
        if let Some(t) = theme.modules.get(id) {
            let mut data = st.context.get_or_insert_with(|| template_context(f, &st.activity)).clone();
            // Themes can also template plugin modules; the plugin's data is under data as usual
            if id.contains('.') {
                data["data"] = crate::plugins::module_data(id).unwrap_or_default();
            }
            let info = crate::plugins::all_modules().into_iter().find(|m| m.id == id);
            data["module"] = module_context(info.as_ref().map(|m| m.elements.as_slice()).unwrap_or_default(), cfg);
            let (size, anim) = ui_kit::render::render(t, cv, &mut st.images, ctx, &data, f.now);
            st.animating |= anim;
            return size;
        }
        // Reserve the backing's place first so it lands under the module once its size is known
        let slot = cv.painter.add(egui::Shape::Noop);
        let size = match id {
            modules::MONSTER => monster::draw(cv, f, cfg, st),
            modules::PLAYER => hunter::player(cv, f, cfg, st),
            modules::BUFFS => hunter::buffs(cv, f, cfg, st),
            modules::WEAPON => hunter::weapon(cv, f, cfg, st),
            modules::DAMAGE => misc::damage(cv, f, cfg, st),
            modules::AREA => misc::area(cv, f, cfg, st),
            modules::CLOCK => misc::clock(cv, f, cfg, st),
            modules::BASE => misc::base(cv, f, cfg, st),
            modules::PERF => misc::perf(cv, f, cfg, st),
            other => crate::plugins::draw_module(other, cv, f, cfg, st, ctx),
        };
        // The player module's backing is its own setting (none or solid), drawn under it with a wide soft edge
        if size != Vec2::ZERO && cfg.panel && id != modules::PLAYER {
            cv.panel_into(slot, 0.0, 0.0, size.x, size.y);
        }
        size
    }
}

/// `module.show.<element>` in templates: whether this element is on in the Modules page.
pub fn module_context(elements: &[rh_core::modules::ElementInfo], cfg: &ModuleConfig) -> serde_json::Value {
    let show: serde_json::Map<String, serde_json::Value> = elements.iter().map(|e| (e.key.clone(), serde_json::Value::Bool(cfg.shows(&e.key)))).collect();
    serde_json::json!({ "show": show })
}

/// Preview in the main window: draws a module scaled into `rect`, with live data or demo data when there is none.
pub struct Preview {
    pub state: State,
    demo: Option<game_world::demo::Demo>,
    snap: Option<Arc<Snapshot>>,
    last: f64,
}

impl Default for Preview {
    fn default() -> Self {
        Preview {
            state: State::default(),
            demo: None,
            snap: None,
            last: -1.0,
        }
    }
}

impl Preview {
    pub fn draw(&mut self, ui: &egui::Ui, rect: Rect, shared: &Shared, id: &str) {
        let live = shared.live().clone();
        let now = shared.now();
        let snap = if live.has_data() {
            live.snapshot.clone()
        } else {
            if now - self.last > 0.1 || self.snap.is_none() {
                let d = self.demo.get_or_insert_with(game_world::demo::Demo::new);
                self.snap = Some(Arc::new(d.step(((now - self.last) as f32).clamp(0.05, 0.5))));
                self.last = now;
            }
            self.snap.clone().unwrap()
        };
        let settings = shared.settings().clone();
        let theme = shared.theme();
        let i18n = i18n::current();
        let game = Game::World;
        let profile = settings.profile(game).clone();
        let Some(cfg) = profile.module(id).cloned() else { return };
        let target = live.target.and_then(|t| snap.monster(t)).or_else(|| snap.large_monsters().next());
        self.state.dt = 0.05;
        self.state.track(&snap, now);
        let f = Frame {
            snap: &snap,
            live: &live,
            settings: &settings,
            profile: &profile,
            i18n: &i18n,
            density: settings.density,
            now,
            game,
            target,
            frame_us: 0,
            preview: true,
            notes: &HashMap::new(),
        };
        let cfg = ModuleConfig {
            opacity: 1.0,
            scale: 1.0,
            ..cfg
        };
        // Measure first (by drawing off-screen), then draw scaled into the preview box
        let hidden = ui.painter().with_clip_rect(Rect::NOTHING);
        let probe = Canvas::new(hidden, Pos2::ZERO, 1.0, &theme);
        let size = draw_any(id, &probe, &f, &cfg, &theme, ui.ctx(), &mut self.state);
        self.state.context = None;
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let s = (rect.width() * 0.86 / size.x).min(rect.height() * 0.8 / size.y).min(1.4);
        let origin = rect.center() - size * s / 2.0;
        let painter = ui.painter().with_clip_rect(rect);
        let cv = Canvas::new(painter, origin, s, &theme);
        draw_any(id, &cv, &f, &cfg, &theme, ui.ctx(), &mut self.state);
        self.state.context = None;
    }
}

pub fn template_context(f: &Frame, activity: &Activity) -> serde_json::Value {
    let o = ui_kit::fields::ContextOptions {
        i18n: f.i18n,
        density: match f.density {
            Density::Full => "full",
            Density::Standard => "standard",
            Density::Minimal => "minimal",
        },
        time: f.now,
        hot_secs: f.idle_secs(),
        activity: Some(activity),
        target: f.target.map(|m| m.uid),
        low_hp: f.profile.player.low_hp / 100.0,
        damage_recent: if f.now - f.live.combo_at < 1.6 { f.live.combo_damage } else { 0.0 },
    };
    ui_kit::fields::context(f.snap, &o)
}

/// Time display: m:ss under an hour, h:mm:ss above.
pub fn mmss(secs: f32) -> String {
    let s = secs.max(0.0) as i64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Thousands separators.
pub fn thousands(v: f32) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Buff time left: m:ss for a minute or more, N s below.
pub fn buff_time(t: f32, infinite: bool) -> String {
    if infinite {
        "∞".into()
    } else if t >= 60.0 {
        mmss(t)
    } else {
        format!("{}s", t.ceil() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(mmss(754.0), "12:34");
        assert_eq!(mmss(3725.0), "1:02:05");
        assert_eq!(thousands(1234567.0), "1,234,567");
        assert_eq!(thousands(-1200.0), "-1,200");
        assert_eq!(buff_time(46.2, false), "47s");
        assert_eq!(buff_time(94.0, false), "1:34");
        assert_eq!(buff_time(1.0, true), "∞");
    }
}
