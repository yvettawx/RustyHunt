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
        label(ui, e, Font::Text, 13.0, pal.accent);
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
    text_at(ui, mr.left_top(), Align2::LEFT_TOP, &name_of(e), Font::TextBold, 15.0, pal.ink);
    text_at(ui, pos2(mr.min.x, mr.min.y + 22.0), Align2::LEFT_TOP, &author, Font::Text, 12.0, pal.mute);
    text_at(ui, pos2(mr.max.x, mr.min.y + 4.0), Align2::RIGHT_TOP, &version, Font::NumBold, 11.0, pal.mute);
    if let Some(err) = &e.error {
        ui.add_space(6.0);
        ui.add(egui::Label::new(widgets::rich(err, Font::Text, 12.0, pal.accent)).wrap());
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        if on {
            let text = i18n::t("state.in_use");
            let g = widgets::galley(ui, &text, Font::TextBold, 12.0, pal.ink);
            let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 18.0), Sense::hover());
            widgets::diamond(ui, pos2(r.min.x + 3.0, r.center().y), 7.0, pal.accent);
            text_at(
                ui,
                pos2(r.min.x + 14.0, r.center().y),
                Align2::LEFT_CENTER,
                &text,
                Font::TextBold,
                12.0,
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
    let c = Pal::of(e.theme.as_ref().unwrap_or(&fallback));
    let fade = |col: egui::Color32| if broken { col.gamma_multiply(0.45) } else { col };
    let p = ui.painter_at(r);
    p.rect_filled(r, CornerRadius::ZERO, fade(c.paper));
    let side = Rect::from_min_size(r.min, vec2(44.0, r.height()));
    p.line_segment([pos2(side.max.x, r.min.y), pos2(side.max.x, r.max.y)], Stroke::new(1.0, fade(c.line)));
    let x = r.min.x + 60.0;
    let right = r.max.x - 16.0;
    p.rect_filled(
        Rect::from_min_max(pos2(x, r.min.y + 22.0), pos2(right, r.min.y + 28.0)),
        CornerRadius::ZERO,
        fade(c.line),
    );
    let tag = p.layout_no_wrap(i18n::current().monster("raging_brachydios"), Font::TextBold.id(13.0), fade(c.onblock));
    let tr = Rect::from_min_size(pos2(x, r.min.y + 46.0), tag.size() + vec2(16.0, 5.0));
    p.rect_filled(tr, CornerRadius::ZERO, fade(c.block));
    p.galley(tr.min + vec2(8.0, 3.0), tag, fade(c.onblock));
    // HP bar: hatch the remaining part
    let hp = Rect::from_min_max(pos2(x, r.min.y + 84.0), pos2(right, r.min.y + 91.0));
    let mut sx = hp.min.x;
    while sx < hp.max.x + hp.height() {
        let a = pos2(sx, hp.max.y);
        let b = pos2(sx + hp.height(), hp.min.y);
        p.with_clip_rect(hp).line_segment([a, b], Stroke::new(1.0, fade(c.mute)));
        sx += 5.0;
    }
    p.rect_filled(
        Rect::from_min_size(hp.min, vec2(hp.width() * 0.62, hp.height())),
        CornerRadius::ZERO,
        fade(c.ink),
    );
    let ay = r.min.y + 114.0;
    p.add(egui::Shape::convex_polygon(
        vec![pos2(x + 4.0, ay - 4.0), pos2(x + 8.0, ay), pos2(x + 4.0, ay + 4.0), pos2(x, ay)],
        fade(c.accent),
        Stroke::NONE,
    ));
    p.text(
        pos2(x + 16.0, ay),
        Align2::LEFT_CENTER,
        format!("{} 0:21", i18n::t("state.enraged")),
        Font::Text.id(11.0),
        fade(c.mute),
    );
}
