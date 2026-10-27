//! First run: six steps, each can be skipped; options on the left, and on the right a looping animation
//! showing what this step's setting changes. A chosen option is written to settings right away.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use rh_core::i18n::{self, I18n, Lang};
use rh_core::model::Game;
use rh_core::modules;
use rh_core::settings::{Backing, MonsterLayout};
use ui_kit::Font;

use super::MainWindow;
use super::style::Pal;
use super::widgets::{self, Btn, button, link, text_at};
use crate::state::Shared;

const STEPS: usize = 6;
const STEP_KEYS: [&str; STEPS] = ["step_lang", "step_game", "step_account", "step_monster", "step_self", "step_keys"];
const SCENE_SECS: f64 = 3.4;

struct Found {
    game: Game,
    path: Option<PathBuf>,
}

struct Account {
    id: u32,
    name: String,
    last: i64,
}

pub struct State {
    step: usize,
    /// When this step's animation started
    t0: f64,
    lang: Lang,
    games: Vec<Found>,
    game: usize,
    accounts: Vec<Account>,
    account: Option<usize>,
    bottom: bool,
    ours: bool,
    scene: usize,
    /// Copy in each language for the preview
    previews: Vec<(Lang, Arc<I18n>)>,
    /// A smoothly transitioning value
    hud: f32,
    side: f32,
}

fn find_games(shared: &Shared) -> Vec<Found> {
    let steam = platform::steam::steam_dir();
    let paths = shared.settings().game_paths.clone();
    Game::ALL
        .iter()
        .map(|&g| {
            let path = paths.get(g.key()).map(PathBuf::from).filter(|p| p.join(g.exe()).is_file()).or_else(|| {
                steam
                    .as_ref()
                    .and_then(|s| platform::steam::app_dir(s, g.steam_app_id()))
                    .filter(|p| p.join(g.exe()).is_file())
            });
            Found { game: g, path }
        })
        .collect()
}

fn find_accounts() -> Vec<Account> {
    let Some(steam) = platform::steam::steam_dir() else { return Vec::new() };
    platform::steam::accounts(&steam)
        .into_iter()
        .map(|a| Account {
            id: a.account_id,
            name: if a.persona.is_empty() { a.account_id.to_string() } else { a.persona },
            last: a.timestamp as i64,
        })
        .collect()
}

impl State {
    pub fn new(shared: &Shared) -> State {
        let s = shared.settings().clone();
        let accounts = find_accounts();
        let account = s
            .save_account
            .and_then(|id| accounts.iter().position(|a| a.id == id))
            .or((!accounts.is_empty()).then_some(0));
        let p = s.profile(Game::World);
        State {
            step: 0,
            t0: shared.now(),
            lang: s.language,
            games: find_games(shared),
            game: 0,
            accounts,
            account,
            bottom: p.monster.layout == MonsterLayout::Bottom,
            ours: p.module(modules::PLAYER).is_some_and(|m| m.enabled),
            scene: 0,
            previews: Vec::new(),
            hud: 1.0,
            side: 0.0,
        }
    }

    pub fn set_step(&mut self, shared: &Shared, step: usize) {
        self.go(shared, step.min(STEPS - 1));
    }

    fn go(&mut self, shared: &Shared, step: usize) {
        self.step = step;
        self.scene = 0;
        self.t0 = shared.now();
    }

    fn preview_i18n(&mut self) -> Arc<I18n> {
        if let Some((_, i)) = self.previews.iter().find(|(l, _)| *l == self.lang) {
            return i.clone();
        }
        let i = Arc::new(i18n::load_bundled(&rh_core::paths::bundled("locales"), self.lang, None));
        self.previews.push((self.lang, i.clone()));
        i
    }

    /// Returns false when onboarding is finished.
    pub fn ui(&mut self, w: &mut MainWindow, ui: &mut Ui, pal: &Pal, r: Rect) -> bool {
        let shared = w.shared.clone();
        let mut keep = true;
        egui::Area::new(Id::unique("onboarding"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.min)
            .show(ui.ctx(), |ui| {
                let (bg, _) = ui.allocate_exact_size(r.size(), Sense::click());
                ui.painter().rect_filled(bg, CornerRadius::ZERO, pal.paper);
                let inner = Rect::from_min_max(bg.min + vec2(64.0, 40.0), bg.max - vec2(64.0, 40.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    self.steps_row(ui, pal);
                    ui.add_space(36.0);
                    let top = ui.cursor().min.y;
                    let left = Rect::from_min_size(pos2(inner.min.x, top), vec2(420.0, inner.max.y - top));
                    let right_x = left.max.x + 56.0;
                    let pw = (inner.max.x - right_x).max(200.0);
                    let preview = Rect::from_min_size(pos2(right_x, top + 8.0), vec2(pw, pw * 9.0 / 16.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                        keep = self.pane(ui, pal, &shared);
                    });
                    self.preview(ui, pal, &shared, preview);
                });
            });
        // Animations refresh at 60 fps; calling request_repaint all the time would starve the overlay's repaints
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        if !keep {
            shared.edit_settings(|s| s.first_run_done = true);
        }
        keep
    }

    fn steps_row(&self, ui: &mut Ui, pal: &Pal) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 26.0;
            for (i, k) in STEP_KEYS.iter().enumerate() {
                let on = i == self.step;
                let (num_c, c) = if on {
                    (pal.accent, pal.ink)
                } else if i < self.step {
                    (pal.mute, pal.mute)
                } else {
                    (pal.faint, pal.faint)
                };
                let text = i18n::t(&format!("onboarding.{k}"));
                let g = widgets::galley(ui, &text, if on { Font::TextBold } else { Font::Text }, 13.0, c);
                let (rr, _) = ui.allocate_exact_size(vec2(g.size().x + 22.0, 18.0), Sense::hover());
                text_at(
                    ui,
                    pos2(rr.min.x, rr.center().y),
                    Align2::LEFT_CENTER,
                    &format!("{:02}", i + 1),
                    Font::Num,
                    11.0,
                    num_c,
                );
                ui.painter().galley(pos2(rr.min.x + 22.0, rr.center().y - g.size().y / 2.0), g, c);
            }
        });
    }

    /// One option row; returns whether it was clicked.
    fn opt(ui: &mut Ui, pal: &Pal, text: &str, desc: &str, on: bool, miss: bool) -> bool {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
        if on {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(r.min.x, r.min.y + 14.0), vec2(2.0, r.height() - 28.0)),
                CornerRadius::ZERO,
                pal.ink,
            );
        }
        let c = if miss { pal.faint } else { pal.ink };
        text_at(
            ui,
            pos2(r.min.x + 14.0, r.center().y),
            Align2::LEFT_CENTER,
            text,
            if on { Font::TextBold } else { Font::Text },
            15.0,
            c,
        );
        if !desc.is_empty() {
            text_at(ui, pos2(r.max.x, r.center().y), Align2::RIGHT_CENTER, desc, Font::Text, 12.0, pal.mute);
        }
        ui.painter()
            .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    fn pane(&mut self, ui: &mut Ui, pal: &Pal, shared: &Arc<Shared>) -> bool {
        widgets::label(ui, i18n::t(&format!("onboarding.{}", STEP_KEYS[self.step])), Font::TextBold, 30.0, pal.ink);
        ui.add_space(22.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        match self.step {
            0 => {
                for l in Lang::ALL {
                    if Self::opt(ui, pal, l.native_name(), "", l == self.lang, false) && l != self.lang {
                        self.lang = l;
                        shared.edit_settings(|s| s.language = l);
                        super::settings::reload_i18n(shared);
                    }
                }
            }
            1 => {
                for i in 0..self.games.len() {
                    let g = self.games[i].game;
                    let found = self.games[i].path.is_some();
                    let name = i18n::t(&format!("game.{}", g.key()));
                    let desc = i18n::t(if found { "onboarding.found" } else { "onboarding.not_found" });
                    let w = ui.available_width();
                    let row_top = ui.cursor().min.y;
                    if Self::opt(ui, pal, &name, if found { &desc } else { "" }, i == self.game, !found) {
                        self.game = i;
                        self.t0 = shared.now();
                    }
                    if !found {
                        // Pick the path by hand when it can't be found
                        let rr = Rect::from_min_size(pos2(ui.min_rect().min.x, row_top), vec2(w, 48.0));
                        let mut pick = false;
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(rr).layout(egui::Layout::right_to_left(egui::Align::Center)),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                pick = link(ui, pal, &i18n::t("action.pick_path")).clicked();
                                widgets::label_mute(ui, pal, &desc, 12.0);
                            },
                        );
                        if pick && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            if dir.join(g.exe()).is_file() {
                                shared.edit_settings(|s| {
                                    s.game_paths.insert(g.key().to_string(), dir.display().to_string());
                                });
                                self.games[i].path = Some(dir);
                            } else {
                                log::warn!("{} 里没有 {}", dir.display(), g.exe());
                            }
                        }
                    }
                }
            }
            2 => {
                if self.accounts.is_empty() {
                    ui.add_space(12.0);
                    widgets::label_mute(ui, pal, &i18n::t("onboarding.no_account"), 13.0);
                }
                for i in 0..self.accounts.len() {
                    let a = &self.accounts[i];
                    let desc = if a.last > 0 {
                        i18n::tf("onboarding.last_login", &[("t", &super::fmt_when(a.last))])
                    } else {
                        String::new()
                    };
                    if Self::opt(ui, pal, &a.name.clone(), &desc, self.account == Some(i), false) {
                        self.account = Some(i);
                        let id = self.accounts[i].id;
                        shared.edit_settings(|s| s.save_account = Some(id));
                        self.t0 = shared.now();
                    }
                }
            }
            3 => {
                for (bottom, key) in [(true, "modules.bottom"), (false, "modules.side")] {
                    if Self::opt(ui, pal, &i18n::t(key), "", self.bottom == bottom, false) && self.bottom != bottom {
                        self.bottom = bottom;
                        shared.edit_settings(|s| {
                            let p = s.profile_mut(Game::World);
                            p.monster.layout = if bottom { MonsterLayout::Bottom } else { MonsterLayout::Side };
                            p.reset_module(Game::World, modules::MONSTER);
                        });
                    }
                }
            }
            4 => {
                for (ours, key, desc) in [(false, "onboarding.native", ""), (true, "onboarding.ours", "onboarding.ours_desc")] {
                    let d = if desc.is_empty() { String::new() } else { i18n::t(desc) };
                    if Self::opt(ui, pal, &i18n::t(key), &d, self.ours == ours, false) && self.ours != ours {
                        self.ours = ours;
                        self.t0 = shared.now();
                        shared.edit_settings(|s| {
                            let p = s.profile_mut(Game::World);
                            p.module_mut(modules::PLAYER).enabled = ours;
                            if ours {
                                p.player.backing = Backing::Blur;
                            }
                        });
                    }
                }
            }
            _ => {
                let hk = shared.settings().hotkeys.clone();
                for (i, (key, name)) in hotkey_rows(&hk).into_iter().enumerate() {
                    if Self::opt(ui, pal, &name, &key, i == self.scene, false) {
                        self.scene = i;
                        self.t0 = shared.now();
                    }
                }
            }
        }
        ui.add_space(30.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        let mut keep = true;
        let last = self.step == STEPS - 1;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if self.step > 0 && button(ui, pal, &i18n::t("action.back"), Btn::Outline, false).clicked() {
                self.go(shared, self.step - 1);
            }
            let next = i18n::t(if last { "action.finish" } else { "action.next" });
            if button(ui, pal, &next, Btn::Primary, false).clicked() {
                if last {
                    keep = false;
                } else {
                    self.go(shared, self.step + 1);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if link(ui, pal, &i18n::t("action.skip")).clicked() {
                    if last {
                        keep = false;
                    } else {
                        self.go(shared, self.step + 1);
                    }
                }
            });
        });
        keep
    }
}

fn hotkey_rows(hk: &rh_core::settings::Hotkeys) -> Vec<(String, String)> {
    vec![
        (hk.density.clone(), i18n::t("hotkey.density")),
        (hk.show_all.clone(), i18n::t("hotkey.show_all")),
        (hk.layout_mode.clone(), i18n::t("hotkey.layout_mode")),
        (hk.cycle_target.clone(), i18n::t("hotkey.cycle_target")),
        (hk.toggle_overlay.clone(), i18n::t("hotkey.toggle_overlay")),
    ]
}

// ---- Demo on the right ----

const HUD: Color32 = Color32::from_rgb(0xf2, 0xf0, 0xe9);

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Easing: fast start, slow end
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[derive(Clone, Copy, PartialEq)]
enum Density {
    Full,
    Standard,
    Minimal,
}

/// State of the preview picture this frame.
struct Frame {
    density: Density,
    hud: f32,
    layout_grid: bool,
    cursor: Option<Pos2>,
    /// Position of the damage module (proportional coordinates)
    dps_at: Pos2,
    target_other: bool,
    banner: f32,
    player: f32,
}

impl State {
    fn preview(&mut self, ui: &mut Ui, pal: &Pal, shared: &Shared, r: Rect) {
        let now = shared.now();
        let t = (now - self.t0) as f32;
        let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);
        let i18n_p = self.preview_i18n();
        let p = ui.painter_at(r.expand(2.0));
        backdrop(&p, r);
        let u = r.width() / 760.0;

        let mut f = Frame {
            density: Density::Standard,
            hud: 1.0,
            layout_grid: false,
            cursor: None,
            dps_at: pos2(0.03, 0.38),
            target_other: false,
            banner: if self.ours { 1.0 } else { 0.0 },
            player: if self.ours { 1.0 } else { 0.0 },
        };
        let mut caption: Option<(String, String, String, bool)> = None;
        let mut book = false;
        let mut proc: Option<(bool, f32)> = None;

        match self.step {
            1 => {
                let found = self.games.get(self.game).is_some_and(|g| g.path.is_some());
                let lt = if found { t % 3.6 } else { t };
                let done = lt > 1.1;
                f.hud = if found && done { 1.0 } else { 0.0 };
                proc = Some((found && done, (lt / 1.1).min(1.0)));
            }
            2 => book = true,
            4 => {
                let lt = t % 4.2;
                if self.ours {
                    f.banner = ease((lt - 0.4) / 0.5);
                    f.player = ease((lt - 0.9) / 0.35);
                }
            }
            5 => {
                if t as f64 > SCENE_SECS {
                    self.scene = (self.scene + 1) % 5;
                    self.t0 = now;
                }
                let st = (now - self.t0) as f32;
                let hk = shared.settings().hotkeys.clone();
                let (key, name) = hotkey_rows(&hk)[self.scene].clone();
                let tap = |at: f32| st >= at && st < at + 0.18;
                let mut chip = String::new();
                let down;
                match self.scene {
                    0 => {
                        let idx = ((st / 1.1) as usize).min(2);
                        f.density = [Density::Full, Density::Standard, Density::Minimal][idx];
                        chip = i18n::t(["density.full", "density.standard", "density.minimal"][idx]);
                        down = tap(0.0) || tap(1.1) || tap(2.2);
                    }
                    1 => {
                        let held = (0.7..2.4).contains(&st);
                        f.density = if held { Density::Full } else { Density::Minimal };
                        down = held;
                        chip = if st < 0.7 {
                            i18n::t("density.minimal")
                        } else {
                            i18n::t(if held { "onboarding.held" } else { "onboarding.released" })
                        };
                    }
                    2 => {
                        f.layout_grid = (0.2..2.9).contains(&st);
                        down = tap(0.2);
                        let a = ease((st - 0.6) / 0.9);
                        let b = ease((st - 1.6) / 0.9);
                        if (0.6..2.9).contains(&st) {
                            let c1 = pos2(lerp(0.60, 0.10, a), lerp(0.60, 0.42, a));
                            f.cursor = Some(pos2(lerp(c1.x, 0.85, b), lerp(c1.y, 0.42, b)));
                        }
                        f.dps_at = pos2(lerp(0.03, 0.78, b), 0.38);
                    }
                    3 => {
                        f.target_other = (0.6..2.1).contains(&st);
                        down = tap(0.6) || tap(2.1);
                    }
                    _ => {
                        f.hud = if (0.5..2.0).contains(&st) { 0.0 } else { 1.0 };
                        down = tap(0.5) || tap(2.0);
                    }
                }
                caption = Some((key, name, chip, down));
            }
            _ => {}
        }

        // Transitions: showing, hiding and moving never jump
        let k = (dt / 0.3).min(1.0);
        self.hud += (f.hud - self.hud) * k;
        let side_target = if self.bottom { 0.0 } else { 1.0 };
        self.side += (side_target - self.side) * (dt / 0.45).min(1.0);
        f.hud = self.hud;

        if book {
            draw_book(&p, pal, r, t, u);
        } else {
            draw_hud(&p, r, u, &f, self.side, &i18n_p);
        }
        if let Some((ok, scan)) = proc {
            let g = self.games.get(self.game).map(|g| g.game).unwrap_or(Game::World);
            if scan < 1.0 {
                let x = lerp(r.min.x, r.max.x, scan);
                p.rect_filled(
                    Rect::from_min_max(pos2(x - 1.0, r.min.y), pos2(x + 1.0, r.max.y)),
                    CornerRadius::ZERO,
                    Color32::from_white_alpha(120),
                );
            }
            let mut text = g.exe().to_string();
            if scan >= 1.0 && !ok {
                text = format!("{text}　{}", i18n::t("onboarding.not_found"));
            }
            let gal = p.layout_no_wrap(text, Font::Mono.id(10.0 * u.max(0.8)), HUD);
            let br = Rect::from_min_size(
                pos2(r.min.x + r.width() * 0.03, r.max.y - r.height() * 0.05 - gal.size().y - 10.0),
                gal.size() + vec2(32.0, 10.0),
            );
            p.rect_filled(br, CornerRadius::ZERO, Color32::from_black_alpha(150));
            let dot = if ok {
                Color32::from_rgb(0x5f, 0xb5, 0x48)
            } else {
                Color32::from_rgb(0x86, 0x84, 0x7d)
            };
            p.circle_filled(pos2(br.min.x + 12.0, br.center().y), 3.0, dot);
            p.galley(pos2(br.min.x + 22.0, br.min.y + 5.0), gal, HUD);
        }

        // Caption under the hotkey demo: key, name, effect, five dots
        if let Some((key, name, chip, down)) = caption {
            let p = ui.painter();
            let y = r.max.y + 24.0;
            let kg = p.layout_no_wrap(key, Font::Mono.id(12.0), if down { pal.paper } else { pal.ink });
            let kr = Rect::from_min_size(pos2(r.min.x, y - 13.0), kg.size() + vec2(16.0, 8.0));
            p.rect(
                kr,
                CornerRadius::ZERO,
                if down { pal.ink } else { Color32::TRANSPARENT },
                Stroke::new(1.0, pal.line),
                StrokeKind::Inside,
            );
            p.galley(kr.min + vec2(8.0, 4.0), kg, pal.ink);
            let nr = p.text(pos2(kr.max.x + 12.0, kr.center().y), Align2::LEFT_CENTER, name, Font::Text.id(13.0), pal.ink);
            if !chip.is_empty() {
                p.text(pos2(nr.max.x + 12.0, kr.center().y), Align2::LEFT_CENTER, chip, Font::Text.id(12.0), pal.mute);
            }
            for i in 0..5 {
                let c = pos2(r.max.x - 4.0 - (4 - i) as f32 * 14.0, kr.center().y);
                let dr = Rect::from_center_size(c, vec2(12.0, 12.0));
                let resp = ui.interact(dr, Id::unique(("ob-dot", i)), Sense::click());
                if resp.clicked() {
                    self.scene = i;
                    self.t0 = now;
                }
                p.circle_filled(c, 3.0, if i == self.scene { pal.ink } else { pal.line });
            }
        }
    }
}

/// Preview background: dark vertical gradient, no game screenshot.
fn backdrop(p: &egui::Painter, r: Rect) {
    let mut mesh = egui::epaint::Mesh::default();
    let top = Color32::from_rgb(0x34, 0x36, 0x35);
    let bottom = Color32::from_rgb(0x11, 0x12, 0x12);
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

fn fade(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// A thin bar: track plus fill
fn line(p: &egui::Painter, x: f32, y: f32, w: f32, h: f32, ratio: f32, fill: Color32, a: f32) {
    p.rect_filled(
        Rect::from_min_size(pos2(x, y), vec2(w, h)),
        CornerRadius::ZERO,
        fade(Color32::from_black_alpha(115), a),
    );
    p.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w * ratio, h)), CornerRadius::ZERO, fade(fill, a));
}

fn tag(p: &egui::Painter, x: f32, y: f32, text: &str, size: f32, a: f32) -> Rect {
    let g = p.layout_no_wrap(text.to_string(), Font::TextBold.id(size), fade(HUD, a));
    let r = Rect::from_min_size(pos2(x, y), g.size() + vec2(size, size * 0.3));
    p.rect_filled(r, CornerRadius::ZERO, fade(Color32::from_rgb(0x11, 0x11, 0x11), a));
    p.galley(r.min + vec2(size * 0.5, size * 0.15), g, fade(HUD, a));
    r
}

fn draw_hud(p: &egui::Painter, r: Rect, u: f32, f: &Frame, side: f32, i: &I18n) {
    let a = f.hud;
    if a <= 0.01 {
        return;
    }
    let at = |x: f32, y: f32| pos2(r.min.x + r.width() * x, r.min.y + r.height() * y);
    let fs = |s: f32| (s * u).max(6.0);

    // Player: the backing first blurs out the native health bar, then the module appears
    if f.banner > 0.0 {
        let br = Rect::from_min_size(at(0.045, 0.025), vec2(r.width() * 0.36, r.height() * 0.11));
        p.rect_filled(br, CornerRadius::same(6), fade(Color32::from_black_alpha(110), f.banner * a));
    }
    if f.player > 0.0 {
        let o = at(0.09, 0.045);
        let w = r.width() * 0.215;
        let pa = f.player * a;
        tag(p, o.x, o.y, "mcwz", fs(8.0), pa);
        p.text(
            pos2(o.x + w, o.y + fs(10.0)),
            Align2::RIGHT_BOTTOM,
            "128",
            Font::NumBold.id(fs(11.0)),
            fade(HUD, pa),
        );
        line(p, o.x, o.y + fs(14.0), w, fs(4.0), 0.85, HUD, pa);
        line(p, o.x, o.y + fs(21.0), w, fs(2.0), 0.7, fade(HUD, 0.6), pa);
    }

    // Buffs
    let buff_a = if f.density == Density::Minimal { 0.0 } else { a };
    if buff_a > 0.0 {
        let mut x = at(0.03, 0.30).x;
        let y = at(0.03, 0.30).y;
        for (name, time) in [
            (i.game("ABNORMALITY_DEMONDRUG"), ""),
            (i.game("ABNORMALITY_ARMORSKIN"), "46s"),
            (i.game("ABNORMALITY_MIGHT_SEED"), "1:34"),
        ] {
            let tr = tag(p, x, y, &name, fs(8.0), buff_a);
            x = tr.max.x + 3.0;
            if !time.is_empty() {
                let g = p.text(pos2(x, tr.center().y), Align2::LEFT_CENTER, time, Font::NumBold.id(fs(8.0)), fade(HUD, buff_a));
                x = g.max.x + 5.0;
            } else {
                x += 2.0;
            }
        }
    }

    // Damage
    let dps_a = if f.density == Density::Minimal { 0.0 } else { a };
    if dps_a > 0.0 {
        let o = at(f.dps_at.x, f.dps_at.y);
        let w = r.width() * 0.17;
        let you = i.t("label.you");
        let rows = [
            ("Aster", 68, 1.0, HUD),
            (you.as_str(), 61, 0.9, Color32::from_rgb(0xff, 0x6b, 0x1a)),
            ("Kiri", 56, 0.82, HUD),
        ];
        let mut y = o.y;
        for (name, v, ratio, c) in rows {
            p.text(pos2(o.x, y), Align2::LEFT_TOP, name, Font::Text.id(fs(8.0)), fade(c, dps_a));
            p.text(pos2(o.x + w, y), Align2::RIGHT_TOP, v.to_string(), Font::NumBold.id(fs(8.0)), fade(c, dps_a));
            line(p, o.x, y + fs(11.0), w, fs(2.0), ratio, c, dps_a);
            y += fs(17.0);
        }
        if f.layout_grid {
            dashed(p, Rect::from_min_max(o - vec2(4.0, 4.0), pos2(o.x + w + 4.0, y)), a);
        }
    }

    // Monster: moves smoothly between the bottom and right positions
    let ox = lerp(0.22, 0.70, side);
    let oy = lerp(0.66, 0.14, side);
    let mw = r.width() * lerp(0.56, 0.27, side);
    let o = at(ox, oy);
    let (key, hp) = if f.target_other { ("bazelgeuse", 0.81) } else { ("raging_brachydios", 0.63) };
    let tr = tag(p, o.x, o.y, &i.monster(key), fs(11.0), a);
    let weak = format!("{} {} {}", i.t("label.weak"), i.t("element.water"), i.t("element.ice"));
    if side < 0.5 {
        p.text(
            pos2(tr.max.x + 8.0, tr.max.y),
            Align2::LEFT_BOTTOM,
            weak,
            Font::TextBold.id(fs(9.0)),
            fade(fade(HUD, 0.66), a),
        );
    }
    p.text(
        pos2(o.x + mw, tr.max.y),
        Align2::RIGHT_BOTTOM,
        format!("{}%", (hp * 100.0) as i32),
        Font::Num.id(fs(15.0)),
        fade(HUD, a),
    );
    let y = tr.max.y + 5.0;
    line(p, o.x, y, mw, fs(5.0), hp, HUD, a);
    line(p, o.x, y + fs(8.0), mw, fs(2.0), 0.48, fade(HUD, 0.6), a);
    if f.density != Density::Minimal {
        let parts = ["PART_HEAD", "PART_ARMS", "PART_BODY", "PART_BACK", "PART_TAIL", "PART_LEGS"];
        let flinch = [0.72, 0.40, 0.88, 0.55, 0.30, 0.64];
        let dura = [0.30, 0.80, 0.15, 0.60, 0.45, 0.20];
        let cols = if side < 0.5 { 3 } else { 1 };
        let gap = fs(14.0);
        let cw = (mw - gap * (cols - 1) as f32) / cols as f32;
        let shown = if f.density == Density::Full { 6 } else { 3 };
        let py = y + fs(16.0);
        for k in 0..shown {
            let (cx, cy) = (o.x + (k % cols) as f32 * (cw + gap), py + (k / cols) as f32 * fs(24.0));
            p.text(pos2(cx, cy), Align2::LEFT_TOP, i.game(parts[k]), Font::Text.id(fs(8.0)), fade(HUD, a));
            line(p, cx, cy + fs(11.0), cw, fs(2.0), flinch[k], HUD, a);
            line(p, cx, cy + fs(15.0), cw, fs(2.0), dura[k], HUD, a * 0.6);
        }
    }

    // Layout mode: grid and cursor
    if f.layout_grid {
        p.rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(80));
        let step = 12.0 * u.max(0.8);
        let c = Color32::from_white_alpha(14);
        let mut x = r.min.x;
        while x < r.max.x {
            p.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, c));
            x += step;
        }
        let mut y = r.min.y;
        while y < r.max.y {
            p.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, c));
            y += step;
        }
    }
    if let Some(c) = f.cursor {
        let c = at(c.x, c.y);
        let s = 1.1 * u.max(0.8);
        let pts = [(1.0, 1.0), (1.0, 15.0), (4.5, 11.5), (7.0, 17.0), (9.5, 16.0), (7.0, 10.5), (12.0, 10.5)];
        let poly: Vec<Pos2> = pts.iter().map(|(x, y)| c + vec2(*x, *y) * s).collect();
        p.add(egui::Shape::convex_polygon(poly.clone(), HUD, Stroke::NONE));
        p.add(egui::Shape::closed_line(poly, Stroke::new(1.0, Color32::from_rgb(0x11, 0x11, 0x11))));
    }
}

fn dashed(p: &egui::Painter, r: Rect, a: f32) {
    let c = fade(Color32::from_white_alpha(140), a);
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        p.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, c), 4.0, 3.0));
    }
}

/// The save account step: crowns light up one by one in the notes grid according to the save.
fn draw_book(p: &egui::Painter, pal: &Pal, r: Rect, t: f32, u: f32) {
    const BOOK: [(&str, bool, bool); 12] = [
        ("raging_brachydios", true, false),
        ("nergigante", true, true),
        ("velkhana", false, true),
        ("teostra", false, false),
        ("safijiiva", true, true),
        ("bazelgeuse", true, false),
        ("savage_deviljho", false, true),
        ("namielle", true, true),
        ("zinogre", false, false),
        ("alatreon", true, false),
        ("fatalis", false, true),
        ("rajang", false, false),
    ];
    let i = i18n::current();
    p.rect_filled(r, CornerRadius::ZERO, pal.paper);
    p.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, pal.line), StrokeKind::Inside);
    let lt = t % 3.73;
    let pad = 18.0;
    let cols = 6;
    let cw = (r.width() - pad * 2.0) / cols as f32;
    let ch = 58.0 * u.max(0.8);
    let top = r.center().y - ch;
    for (k, (key, gold, mini)) in BOOK.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(r.min.x + pad + (k % cols) as f32 * cw, top + (k / cols) as f32 * ch), vec2(cw, ch));
        p.rect_stroke(cell, CornerRadius::ZERO, Stroke::new(1.0, pal.line), StrokeKind::Inside);
        let on = lt > 0.25 + k as f32 * 0.14;
        p.text(
            cell.min + vec2(8.0, 10.0),
            Align2::LEFT_TOP,
            i.monster(key),
            Font::Text.id(10.0 * u.max(0.8)),
            if on { pal.ink } else { pal.faint },
        );
        for (j, (has, fill, edge)) in [
            (*gold, Color32::from_rgb(0xe9, 0xc2, 0x5b), Color32::from_rgb(0xb8, 0x95, 0x2e)),
            (*mini, Color32::from_rgb(0xc9, 0xcd, 0xd2), Color32::from_rgb(0x9a, 0xa0, 0xa6)),
        ]
        .into_iter()
        .enumerate()
        {
            let c = pos2(cell.min.x + 12.0 + j as f32 * 13.0, cell.max.y - 12.0);
            let d = 4.5;
            let pts = vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)];
            let lit = on && has;
            p.add(egui::Shape::convex_polygon(
                pts,
                if lit { fill } else { Color32::TRANSPARENT },
                Stroke::new(1.0, if lit { edge } else { pal.line }),
            ));
        }
    }
}
