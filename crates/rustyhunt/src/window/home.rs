//! 01 Home: connection status, current hunt, games, recent hunts.

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use rh_core::i18n;
use rh_core::model::{Game, Target};
use ui_kit::Font;

use super::style::Pal;
use super::widgets::{self, Btn, button, label, text_at};
use super::{MainWindow, Page};
use crate::overlay::{mmss, thousands};
use crate::state::{Action, Connection};

#[derive(Default)]
pub struct State {
    recent: Vec<crate::records::Hunt>,
    loaded_at: Option<std::time::Instant>,
}

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    widgets::page_header(ui, pal, &i18n::t("nav.home"), |_| {});
    let live = shared.live().clone();
    let snap = live.snapshot.clone();

    // Connection status
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let (dot, status, warn) = match &live.connection {
                Connection::Connected => (pal.ok, i18n::t("conn.connected"), false),
                Connection::Demo => (pal.hue, i18n::t("conn.demo"), false),
                Connection::MapMismatch { .. } => (pal.accent, i18n::t("conn.mismatch"), true),
                Connection::Failed(_) => (pal.faint, i18n::t("conn.failed"), true),
                Connection::Searching => (pal.faint, i18n::t("conn.searching"), false),
            };
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
                if live.has_data() {
                    ui.painter().circle_filled(r.center(), 7.5, dot.gamma_multiply(0.18));
                }
                ui.painter().circle_filled(r.center(), 3.5, dot);
                label(ui, &status, Font::Text, widgets::T_BODY, if warn { pal.accent } else { pal.mute });
            });
            let game = match &live.connection {
                Connection::Searching | Connection::Failed(_) => i18n::t("home.waiting_game"),
                _ => i18n::t("game.world"),
            };
            ui.add_space(4.0);
            label(ui, game, Font::TextBold, widgets::T_TITLE + 4.0, pal.ink);
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 32.0;
                let exe = if live.exe.is_empty() { "-".to_string() } else { live.exe.clone() };
                widgets::fact(ui, pal, &i18n::t("home.process"), &exe, 14.0, false);
                widgets::fact(ui, pal, &i18n::t("home.version"), live.version.as_deref().unwrap_or("-"), 14.0, false);
                widgets::fact(ui, pal, &i18n::t("home.read"), &format!("{} Hz", shared.settings().perf.read_hz), 14.0, false);
                let fm = shared.frame_us.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
                widgets::fact(ui, pal, &i18n::t("home.frame"), &format!("{fm:.2} ms"), 14.0, false);
                let up = live.attached_at.map(|t| mmss(t.elapsed().as_secs_f32())).unwrap_or_else(|| "-".into());
                widgets::fact(ui, pal, &i18n::t("home.uptime"), &up, 14.0, false);
            });
            if let Connection::MapMismatch { version, latest } = &live.connection {
                ui.add_space(12.0);
                label(
                    ui,
                    i18n::tf("home.mismatch", &[("game", version), ("map", latest.as_deref().unwrap_or("-"))]),
                    Font::Text,
                    13.0,
                    pal.accent,
                );
            }
            if let Connection::Failed(e) = &live.connection {
                ui.add_space(12.0);
                label(ui, e, Font::Text, 13.0, pal.accent);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if matches!(live.connection, Connection::MapMismatch { .. }) {
                if button(ui, pal, &i18n::t("action.update_maps"), Btn::Primary, false).clicked() {
                    let s = shared.clone();
                    std::thread::spawn(move || crate::update::check(&s, false));
                }
            } else {
                if button(ui, pal, &i18n::t("action.layout"), Btn::Primary, false).clicked() {
                    shared.act(Action::SetLayout(true));
                }
                let visible = shared.settings().overlay_visible;
                let t = if visible {
                    i18n::t("action.pause_overlay")
                } else {
                    i18n::t("action.resume_overlay")
                };
                if button(ui, pal, &t, Btn::Outline, false).clicked() {
                    shared.act(Action::ToggleOverlay);
                }
            }
        });
    });
    ui.add_space(40.0);

    // Current hunt
    let target = live.target.and_then(|t| snap.monster(t)).or_else(|| snap.large_monsters().next());
    if live.has_data()
        && (snap.in_quest() || snap.in_hunt_zone)
        && let Some(m) = target
    {
        widgets::section(ui, pal, &i18n::t("home.current_hunt"), Some(&mmss(snap.elapsed)));
        ui.horizontal(|ui| {
            widgets::tag(ui, pal, &live_name(m), 20.0);
            let state = if m.enraged {
                format!("{} {}", i18n::t("state.enraged"), mmss(m.enrage_timer))
            } else if m.tired {
                i18n::t("state.tired")
            } else if m.capturable() {
                i18n::t("state.capturable")
            } else {
                String::new()
            };
            let state_color = if m.enraged {
                pal.enrage
            } else if m.tired {
                pal.cool
            } else {
                pal.ok
            };
            label(ui, state, Font::TextBold, widgets::T_BODY, state_color);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                label(ui, format!("/{}", thousands(m.hp_max)), Font::NumBold, widgets::T_HEAD, pal.mute);
                label(ui, thousands(m.hp), Font::NumHeavy, widgets::T_DISPLAY, pal.ink);
            });
        });
        ui.add_space(12.0);
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
        hatch(ui, r, pal);
        ui.painter().rect_filled(
            Rect::from_min_size(r.min, vec2(r.width() * m.hp_ratio(), r.height())),
            CornerRadius::ZERO,
            if m.enraged { pal.enrage } else { pal.ink },
        );
        if m.capture_threshold > 0.0 {
            let x = r.min.x + r.width() * m.capture_threshold;
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(x - 1.0, r.min.y - 5.0), pos2(x + 1.0, r.max.y + 5.0)),
                CornerRadius::ZERO,
                pal.accent,
            );
        }
        ui.add_space(4.0);
        let stamina = if m.tired { pal.cool } else { pal.mute };
        widgets::thin_bar(ui, stamina, pal.line, m.stamina_ratio(), vec2(ui.available_width(), 2.0));
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 56.0;
            let elapsed = snap.elapsed.max(1.0);
            let me = snap.self_member().map_or(0.0, |p| p.damage) / elapsed;
            let mut sorted: Vec<f32> = snap.party.iter().map(|p| p.damage).collect();
            sorted.sort_by(|a, b| b.total_cmp(a));
            let my = snap.self_member().map_or(0.0, |p| p.damage);
            let rank = sorted.iter().position(|d| *d <= my).map_or(1, |i| i + 1);
            widgets::fact(ui, pal, &i18n::t("home.your_dps"), &thousands(me), widgets::T_TITLE, true);
            widgets::fact(
                ui,
                pal,
                &i18n::t("home.rank"),
                &format!("{rank}/{}", snap.party.len().max(1)),
                widgets::T_TITLE,
                false,
            );
            let breaks: i32 = m.parts.iter().map(|p| p.breaks).sum();
            let breaks_max: i32 = m.parts.iter().map(|p| p.breaks_max).sum();
            widgets::fact(ui, pal, &i18n::t("home.breaks"), &format!("{breaks}/{breaks_max}"), widgets::T_TITLE, false);
            let ails: i32 = m.ailments.iter().map(|a| a.count).sum();
            widgets::fact(ui, pal, &i18n::t("home.ailments"), &ails.to_string(), widgets::T_TITLE, false);
            let _ = Target::None;
        });
        ui.add_space(40.0);
    }

    // Two columns: games and recent hunts
    if w.home.loaded_at.is_none_or(|t| t.elapsed().as_secs() > 5) {
        w.home.loaded_at = Some(std::time::Instant::now());
        w.home.recent = crate::records::open().and_then(|c| crate::records::list(&c, None, 6)).unwrap_or_default();
    }
    let avail = ui.available_width();
    let left_w = (avail - 56.0) / 2.25;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 56.0;
        ui.vertical(|ui| {
            ui.set_width(left_w);
            widgets::section(ui, pal, &i18n::t("home.games"), None);
            for g in Game::ALL {
                let running = g == Game::World && live.connected();
                let (r, _) = ui.allocate_exact_size(vec2(left_w, 64.0), Sense::hover());
                // The running game is marked left of the column so the names stay aligned
                if running {
                    widgets::diamond(ui, pos2(r.min.x - 14.0, r.min.y + 22.0), 7.0, pal.ok);
                }
                text_at(
                    ui,
                    pos2(r.min.x, r.min.y + 12.0),
                    Align2::LEFT_TOP,
                    &i18n::t(&format!("game.{}", g.key())),
                    Font::TextBold,
                    widgets::T_HEAD,
                    if g.supported() { pal.ink } else { pal.mute },
                );
                let ds = if !g.supported() {
                    i18n::t("home.later")
                } else if running {
                    i18n::t("home.running")
                } else {
                    i18n::t("home.not_running")
                };
                text_at(
                    ui,
                    pos2(r.min.x, r.min.y + 36.0),
                    Align2::LEFT_TOP,
                    &ds,
                    Font::Text,
                    widgets::T_SMALL + 1.0,
                    if running { pal.ok } else { pal.mute },
                );
                ui.painter()
                    .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
                if g.supported() {
                    let lw = widgets::galley(ui, &i18n::t("nav.modules"), Font::Text, widgets::T_BODY, pal.ink).size().x;
                    let lr = Rect::from_min_size(pos2(r.max.x - lw, r.center().y - 9.0), vec2(lw, 18.0));
                    // A child that doesn't take space in the column, or the next row would start lower
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(lr));
                    if widgets::link(&mut child, pal, &i18n::t("nav.modules")).clicked() {
                        w.page = Page::Modules;
                    }
                }
            }
        });
        ui.vertical(|ui| {
            let week = w
                .home
                .recent
                .iter()
                .filter(|h| chrono::Local::now().timestamp() - h.started_at < 7 * 86400)
                .count();
            widgets::section(ui, pal, &i18n::t("home.recent"), Some(&i18n::tf("home.this_week", &[("n", &week.to_string())])));
            if w.home.recent.is_empty() {
                // An empty list keeps the height of a few rows, with the message in a dashed box
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 128.0), Sense::hover());
                let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                for s in pts.windows(2) {
                    ui.painter()
                        .extend(egui::Shape::dashed_line(&[s[0], s[1]], Stroke::new(1.0, pal.line), 4.0, 4.0));
                }
                text_at(
                    ui,
                    r.center(),
                    Align2::CENTER_CENTER,
                    &i18n::t("home.no_records"),
                    Font::Text,
                    widgets::T_BODY,
                    pal.mute,
                );
            }
            let width = ui.available_width();
            for h in w.home.recent.clone() {
                let (r, resp) = ui.allocate_exact_size(vec2(width, 46.0), Sense::click());
                let name = rh_core::i18n::current().monster(&h.monster);
                let hov = widgets::ease(ui, resp.id.with("hover"), resp.hovered());
                ui.painter().rect_filled(r.expand2(vec2(8.0, 0.0)), CornerRadius::ZERO, pal.tint(hov * 0.6));
                let nr = text_at(
                    ui,
                    pos2(r.min.x, r.center().y),
                    Align2::LEFT_CENTER,
                    &name,
                    Font::TextBold,
                    widgets::T_BODY + 1.0,
                    pal.ink,
                );
                if resp.hovered() {
                    ui.painter()
                        .line_segment([pos2(nr.min.x, nr.max.y + 1.0), pos2(nr.max.x, nr.max.y + 1.0)], Stroke::new(1.0, pal.ink));
                }
                text_at(
                    ui,
                    pos2(nr.max.x + 8.0, r.center().y + 1.0),
                    Align2::LEFT_CENTER,
                    &super::fmt_when(h.started_at),
                    Font::Text,
                    12.0,
                    pal.mute,
                );
                let res_x = r.max.x;
                let capture = h.result == "capture";
                text_at(
                    ui,
                    pos2(res_x, r.center().y),
                    Align2::RIGHT_CENTER,
                    &i18n::t(&format!("result.{}", h.result)),
                    Font::Text,
                    12.0,
                    if capture { pal.ok } else { pal.mute },
                );
                text_at(
                    ui,
                    pos2(res_x - 56.0, r.center().y),
                    Align2::RIGHT_CENTER,
                    &format!("{} DPS", h.self_dps().round()),
                    Font::NumBold,
                    13.0,
                    pal.ink,
                );
                text_at(
                    ui,
                    pos2(res_x - 140.0, r.center().y),
                    Align2::RIGHT_CENTER,
                    &mmss(h.duration),
                    Font::NumBold,
                    13.0,
                    pal.ink,
                );
                ui.painter()
                    .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
                if resp.clicked() {
                    w.page = Page::Records;
                    w.records.tab = 0;
                    w.records.select(h.id);
                }
            }
        });
    });
}

fn live_name(m: &rh_core::model::Monster) -> String {
    rh_core::i18n::current().monster(&m.key)
}

/// Hatched track (the empty part of a bar).
pub fn hatch(ui: &Ui, r: Rect, pal: &Pal) {
    let p = ui.painter().with_clip_rect(r);
    let c = pal.ink.gamma_multiply(if pal.dark { 0.42 } else { 0.5 });
    let mut x = r.min.x - r.height();
    while x < r.max.x {
        p.line_segment([pos2(x, r.max.y), pos2(x + r.height(), r.min.y)], Stroke::new(1.0, c));
        x += 5.0;
    }
}
