//! 04 Themes: built-in light and dark plus local themes; apply, import, delete, open the XML, follow the system.

use std::time::{Duration, Instant};

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use rh_core::i18n;
use ui_kit::Font;

use super::style::Pal;
use super::widgets::{self, Btn, button, label, link, switch, text_at};
use super::{MainWindow, Modal};
use crate::themes::{self, Entry};

#[derive(Default)]
pub struct State {
    entries: Vec<Entry>,
    loaded: Option<Instant>,
    error: Option<String>,
}

impl State {
    fn refresh(&mut self) {
        self.entries = themes::list();
        self.loaded = Some(Instant::now());
    }
}

/// ID of the theme in use (when following the system, the built-in light or dark).
fn active(w: &MainWindow) -> String {
    let s = w.shared.settings();
    if s.theme_follow_system {
        if crate::win::system_dark() { "dark" } else { "light" }.into()
    } else {
        s.theme.clone()
    }
}

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    // Theme folders may be changed by hand; rescan every few seconds
    if w.themes.loaded.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
        w.themes.refresh();
    }
    widgets::page_header(ui, pal, &i18n::t("nav.themes"), |ui| {
        if button(ui, pal, &i18n::t("action.import"), Btn::Primary, false).clicked()
            && let Some(p) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file()
        {
            match themes::import(&p) {
                Ok(_) => w.themes.error = None,
                Err(e) => w.themes.error = Some(e),
            }
            w.themes.refresh();
        }
        if button(ui, pal, &i18n::t("action.open_folder"), Btn::Outline, false).clicked() {
            let dir = rh_core::paths::themes();
            let _ = std::fs::create_dir_all(&dir);
            crate::win::open(&dir);
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        widgets::label_mute(ui, pal, &i18n::t("theme.follow_system"), 13.0);
        let mut follow = w.shared.settings().theme_follow_system;
        if switch(ui, pal, &mut follow).changed() {
            w.shared.edit_settings(|s| s.theme_follow_system = follow);
            themes::reload(&w.shared);
        }
    });
    ui.add_space(26.0);
    if let Some(e) = &w.themes.error {
        label(ui, e, Font::Text, widgets::T_BODY, pal.accent);
        ui.add_space(14.0);
    }

    let active = active(w);
    let cols = 3;
    let gap = 30.0;
    let card_w = ((ui.available_width() - gap * (cols - 1) as f32) / cols as f32).floor();
    let entries = std::mem::take(&mut w.themes.entries);
    for chunk in entries.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for e in chunk {
                ui.vertical(|ui| {
                    ui.set_width(card_w);
                    card(w, ui, pal, e, card_w, e.id == active);
                });
            }
        });
        ui.add_space(34.0);
    }
    if w.themes.entries.is_empty() {
        w.themes.entries = entries;
    }
}

fn name_of(e: &Entry) -> String {
    match (e.builtin(), &e.theme) {
        (true, _) => i18n::t(&format!("theme.{}", e.id)),
        (false, Some(t)) => t.manifest.name.clone(),
        (false, None) => e.id.clone(),
    }
}

fn card(w: &mut MainWindow, ui: &mut Ui, pal: &Pal, e: &Entry, width: f32, on: bool) {
    let broken = e.theme.is_none();
    let (r, resp) = ui.allocate_exact_size(vec2(width, 156.0), if broken { Sense::hover() } else { Sense::click() });
    preview(ui, r, e, broken);
    let border = if on { Stroke::new(2.0, pal.ink) } else { Stroke::new(1.0, pal.line) };
    ui.painter().rect_stroke(r, CornerRadius::ZERO, border, StrokeKind::Outside);
    if resp.hovered() && !broken {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() && !on {
        apply(w, &e.id);
    }
    ui.add_space(14.0);
    let (author, version) = match &e.theme {
        _ if e.builtin() => (i18n::t("theme.builtin"), rh_core::VERSION.to_string()),
        Some(t) => (t.manifest.author.clone(), t.manifest.version.clone()),
        None => (String::new(), String::new()),
    };
    let (mr, _) = ui.allocate_exact_size(vec2(width, 36.0), Sense::hover());
    text_at(ui, mr.left_top(), Align2::LEFT_TOP, &name_of(e), Font::TextBold, widgets::T_BODY, pal.ink);
    text_at(
        ui,
        pos2(mr.min.x, mr.min.y + 22.0),
        Align2::LEFT_TOP,
        &author,
        Font::Text,
        widgets::T_SMALL,
        pal.mute,
    );
    text_at(
        ui,
        pos2(mr.max.x, mr.min.y + 4.0),
        Align2::RIGHT_TOP,
        &version,
        Font::NumBold,
        widgets::T_SMALL,
        pal.mute,
    );
    if let Some(err) = &e.error {
        ui.add_space(6.0);
        ui.add(egui::Label::new(widgets::rich(err, Font::Text, widgets::T_SMALL, pal.accent)).wrap());
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        if on {
            let text = i18n::t("state.in_use");
            let g = widgets::galley(ui, &text, Font::TextBold, widgets::T_SMALL, pal.ink);
            let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 18.0), Sense::hover());
            widgets::diamond(ui, pos2(r.min.x + 3.0, r.center().y), 7.0, pal.hue);
            text_at(
                ui,
                pos2(r.min.x + 14.0, r.center().y),
                Align2::LEFT_CENTER,
                &text,
                Font::TextBold,
                widgets::T_SMALL,
                pal.ink,
            );
        } else if !broken && link(ui, pal, &i18n::t("action.apply")).clicked() {
            apply(w, &e.id);
        }
        if let Some(dir) = &e.dir {
            if link(ui, pal, &i18n::t("action.open_xml")).clicked() {
                crate::win::open(&dir.join("theme.xml"));
            }
            if link(ui, pal, &i18n::t("action.delete")).clicked() {
                let (dir, id, current) = (dir.clone(), e.id.clone(), on);
                w.modal = Some(Modal {
                    title: i18n::tf("plugin.ask_delete", &[("name", &name_of(e))]),
                    lines: vec![dir.display().to_string()],
                    yes: i18n::t("action.delete"),
                    on_yes: Box::new(move |shared| {
                        if let Err(err) = themes::delete(&dir) {
                            log::warn!("removing theme {id}: {err}");
                        }
                        if current {
                            themes::apply(shared, "light");
                        }
                    }),
                });
            }
        }
    });
}

fn apply(w: &mut MainWindow, id: &str) {
    // Picking a theme by hand stops following the system
    w.shared.edit_settings(|s| s.theme_follow_system = false);
    themes::apply(&w.shared, id);
    w.themes.loaded = None;
}

/// Swatch on a theme card: a bit of sidebar, a name tag, an HP bar and a status line in this theme's own window colors.
fn preview(ui: &Ui, r: Rect, e: &Entry, broken: bool) {
    let fallback = ui_kit::Theme::builtin(false);
    let theme = e.theme.as_ref().unwrap_or(&fallback);
    let c = Pal::of(theme);
    let fade = |col: egui::Color32| if broken { col.gamma_multiply(0.45) } else { col };
    let p = ui.painter_at(r);
    // Left: the main window's paper and current-page block
    let side = Rect::from_min_size(r.min, vec2(44.0, r.height()));
    p.rect_filled(side, CornerRadius::ZERO, fade(c.nav));
    p.rect_filled(
        Rect::from_min_size(side.min + vec2(8.0, 18.0), vec2(28.0, 10.0)),
        CornerRadius::ZERO,
        fade(c.hue),
    );
    for i in 0..3 {
        p.rect_filled(
            Rect::from_min_size(side.min + vec2(8.0, 38.0 + i as f32 * 14.0), vec2(20.0, 3.0)),
            CornerRadius::ZERO,
            fade(c.faint),
        );
    }
    // Right: a bit of the overlay drawn with this theme, over a dark game-like backdrop
    let game = Rect::from_min_max(pos2(side.max.x, r.min.y), r.max);
    p.rect_filled(game, CornerRadius::ZERO, fade(egui::Color32::from_rgb(0x3a, 0x3c, 0x40)));
    let cv = ui_kit::Canvas {
        painter: p.clone(),
        origin: game.min + vec2(22.0, 26.0),
        scale: 1.0,
        opacity: if broken { 0.45 } else { 1.0 },
        theme,
    };
    let w = game.width() - 44.0;
    cv.panel(0.0, 0.0, w, 104.0);
    let name = i18n::current().monster("raging_brachydios");
    cv.tag(0.0, 0.0, &name, Font::TextBold, 12.0, (6.0, 2.0, 6.0, 2.0), cv.tok("ink"), cv.tok("text"));
    let mhp = [ui_kit::Layer::fill(0.62, cv.tok("monster-hp"))];
    cv.bar_ex("monster-hp", 0.0, 26.0, w, 6.0, &mhp, true, false);
    let hp = [ui_kit::Layer::fill(0.74, cv.tok("hp"))];
    cv.bar_ex("player-hp", 0.0, 58.0, w * 0.7, 10.0, &hp, false, false);
    cv.bar("stamina", 0.0, 74.0, w * 0.7, 4.0, &[ui_kit::Layer::fill(0.9, cv.tok("stamina"))], false);
    let ay = 96.0;
    cv.dot(4.0, ay, 3.0, cv.tok("enrage"), false);
    cv.text(
        14.0,
        ay - 8.0,
        &format!("{} 0:21", i18n::t("state.enraged")),
        Font::TextBold,
        11.0,
        cv.tok("enrage"),
        Align2::LEFT_TOP,
    );
}
