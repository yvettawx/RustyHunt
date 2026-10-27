//! 02 Modules: switch by game and profile; module list, live preview, module settings, element toggles.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use rh_core::i18n;
use rh_core::model::Game;
use rh_core::modules::{self, ModuleInfo};
use rh_core::settings::{Backing, ModuleConfig, MonsterLayout, TargetMode, Tray};
use ui_kit::Font;

use super::MainWindow;
use super::style::Pal;
use super::widgets::{self, label_mute, link, switch, text_at, toggle_row};
use crate::overlay::Preview;
use crate::state::Action;

#[derive(Default)]
pub struct State {
    selected: usize,
    preview: Preview,
    new_profile: Option<String>,
    renaming: Option<String>,
    /// The "More" group of the element list is open
    more: bool,
}

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let game = Game::World;
    let key = shared.settings().hotkeys.layout_mode.clone();
    widgets::page_header(ui, pal, &i18n::t("nav.modules"), |ui| {
        if widgets::button_key(ui, pal, &i18n::t("action.layout"), &key).clicked() {
            shared.act(Action::SetLayout(true));
        }
    });

    // Game tabs and profiles
    ui.horizontal(|ui| {
        let tabs: Vec<(String, bool)> = Game::ALL.iter().map(|g| (i18n::t(&format!("game_short.{}", g.key())), g.supported())).collect();
        widgets::tabs(ui, pal, &tabs, 0, false);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (names, active) = {
                let s = shared.settings();
                let l = s.layouts(game);
                (l.profiles.keys().cloned().collect::<Vec<_>>(), l.active.clone())
            };
            if names.len() > 1 && link(ui, pal, &i18n::t("action.delete")).clicked() {
                let a = active.clone();
                shared.edit_settings(|s| {
                    let l = s.layouts_mut(game);
                    l.profiles.remove(&a);
                    l.active = l.profiles.keys().next().cloned().unwrap_or_default();
                });
            }
            if link(ui, pal, &i18n::t("action.rename")).clicked() {
                w.modules.renaming = Some(active.clone());
            }
            if link(ui, pal, &i18n::t("action.new_profile")).clicked() {
                w.modules.new_profile = Some(String::new());
            }
            let mut sel = names.iter().position(|n| *n == active).unwrap_or(0);
            // The default profile's name is stored in Simplified Chinese and shown in the UI language
            let shown: Vec<String> = names
                .iter()
                .map(|n| {
                    if n == rh_core::settings::DEFAULT_PROFILE {
                        i18n::t("modules.default_profile")
                    } else {
                        n.clone()
                    }
                })
                .collect();
            if widgets::select(ui, pal, "profile", &shown, &mut sel, 110.0) {
                let n = names[sel].clone();
                shared.edit_settings(|s| s.layouts_mut(game).active = n);
            }
            label_mute(ui, pal, &i18n::t("label.profile"), 13.0);
        });
    });
    // New or renamed profile
    if let Some(name) = w.modules.new_profile.as_mut() {
        let mut done = None;
        ui.horizontal(|ui| {
            widgets::text_input(ui, pal, name, &i18n::t("modules.profile_name"), 180.0);
            if link(ui, pal, &i18n::t("action.done")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(true);
            }
            if link(ui, pal, &i18n::t("action.cancel")).clicked() {
                done = Some(false);
            }
        });
        match done {
            Some(true) if !name.trim().is_empty() => {
                let n = name.trim().to_string();
                shared.edit_settings(|s| {
                    let l = s.layouts_mut(game);
                    let copy = l.active().clone();
                    l.profiles.insert(n.clone(), copy);
                    l.active = n;
                });
                w.modules.new_profile = None;
            }
            Some(_) => w.modules.new_profile = None,
            None => {}
        }
    }
    if let Some(old) = w.modules.renaming.clone() {
        let mut name = w.modules.new_profile.take().unwrap_or_else(|| old.clone());
        let mut done = None;
        ui.horizontal(|ui| {
            widgets::text_input(ui, pal, &mut name, &i18n::t("modules.profile_name"), 180.0);
            if link(ui, pal, &i18n::t("action.done")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(true);
            }
            if link(ui, pal, &i18n::t("action.cancel")).clicked() {
                done = Some(false);
            }
        });
        match done {
            Some(true) if !name.trim().is_empty() => {
                let n = name.trim().to_string();
                shared.edit_settings(|s| {
                    let l = s.layouts_mut(game);
                    if let Some(p) = l.profiles.remove(&old) {
                        l.profiles.insert(n.clone(), p);
                        l.active = n;
                    }
                });
                w.modules.renaming = None;
            }
            Some(_) => w.modules.renaming = None,
            None => w.modules.new_profile = Some(name).filter(|n| *n != old),
        }
    }
    ui.add_space(26.0);

    let infos = crate::plugins::all_modules();
    if w.modules.selected >= infos.len() {
        w.modules.selected = 0;
    }
    let total_w = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 44.0;
        // Module list
        ui.vertical(|ui| {
            ui.set_width(220.0);
            for (i, m) in infos.iter().enumerate() {
                let enabled = shared.settings().profile(game).module(&m.id).is_some_and(|c| c.enabled);
                let (r, resp) = ui.allocate_exact_size(vec2(220.0, 48.0), Sense::click());
                let on = i == w.modules.selected;
                let hov = widgets::ease(ui, resp.id.with("hover"), resp.hovered() && !on);
                ui.painter().rect_filled(r, CornerRadius::ZERO, pal.tint(hov * 0.6));
                let sel = widgets::ease(ui, resp.id.with("on"), on);
                if sel > 0.0 {
                    ui.painter().rect_filled(
                        Rect::from_min_size(pos2(r.min.x, r.min.y + 12.0), vec2(3.0, 24.0 * sel)),
                        CornerRadius::ZERO,
                        pal.hue,
                    );
                }
                text_at(
                    ui,
                    pos2(r.min.x + 14.0, r.center().y + 1.0),
                    Align2::LEFT_CENTER,
                    &format!("{:02}", i + 1),
                    Font::Num,
                    widgets::T_MICRO,
                    if on { pal.hue } else { pal.faint },
                );
                let name = crate::plugins::module_name(&m.id);
                let color = if !enabled { pal.faint } else { pal.ink };
                text_at(
                    ui,
                    pos2(r.min.x + 42.0, r.center().y),
                    Align2::LEFT_CENTER,
                    &name,
                    if on { Font::TextBold } else { Font::Text },
                    widgets::T_BODY,
                    color,
                );
                ui.painter()
                    .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
                let sr = Rect::from_min_size(pos2(r.max.x - 28.0, r.center().y - 7.0), vec2(28.0, 14.0));
                let mut v = enabled;
                let mut changed = false;
                ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
                    changed = switch(ui, pal, &mut v).changed();
                });
                if changed {
                    let id = m.id.clone();
                    shared.edit_settings(|s| s.profile_mut(game).module_mut(&id).enabled = v);
                } else if resp.clicked() {
                    w.modules.selected = i;
                }
            }
        });
        // Preview and settings
        ui.vertical(|ui| {
            let width = total_w - 220.0 - 44.0;
            ui.set_width(width);
            let info = &infos[w.modules.selected];
            let cfg = shared.settings().profile(game).module(&info.id).cloned().unwrap_or_default();
            let (pr, _) = ui.allocate_exact_size(vec2(width, 190.0), Sense::hover());
            preview_backdrop(ui, pal, pr);
            if cfg.enabled {
                w.modules.preview.draw(ui, pr, &shared, &info.id);
            }
            ui.add_space(28.0);
            let col_w = (width - 48.0) / 2.0;
            ui.add_enabled_ui(cfg.enabled, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 48.0;
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        options(w, ui, pal, info, &cfg, col_w);
                    });
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        field_label(ui, pal, &i18n::t("label.content"));
                        let element = |ui: &mut Ui, e: &rh_core::modules::ElementInfo| {
                            let mut on = cfg.shows(&e.key);
                            let name = if info.plugin.is_some() { e.name.clone() } else { i18n::t(&e.name) };
                            if toggle_row(ui, pal, &name, &mut on) {
                                let (id, k) = (info.id.clone(), e.key.clone());
                                shared.edit_settings(|s| s.profile_mut(game).module_mut(&id).toggle(&k));
                            }
                        };
                        for e in info.elements.iter().filter(|e| !e.more) {
                            element(ui, e);
                        }
                        // Fine settings stay folded away under "More"
                        let more: Vec<_> = info.elements.iter().filter(|e| e.more).collect();
                        if !more.is_empty() {
                            ui.add_space(8.0);
                            let open = w.modules.more;
                            let text = format!("{} {}  {}", i18n::t("label.more"), more.len(), if open { "−" } else { "+" });
                            if link(ui, pal, &text).clicked() {
                                w.modules.more = !open;
                            }
                            ui.add_space(8.0);
                            if open {
                                for e in more {
                                    element(ui, e);
                                }
                            }
                        }
                    });
                });
            });
        });
    });
}

/// Background of the preview box: dark with a slight vertical gradient, no game screenshot.
fn preview_backdrop(ui: &Ui, pal: &Pal, r: Rect) {
    let p = ui.painter();
    let mut mesh = egui::epaint::Mesh::default();
    // The theme's window.desk as the base, lightened a little
    let bottom = pal.desk;
    let top = egui::lerp(egui::Rgba::from(bottom)..=egui::Rgba::from(Color32::WHITE), 0.1).into();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

fn field_label(ui: &mut Ui, pal: &Pal, text: &str) {
    label_mute(ui, pal, text, 12.0);
    ui.add_space(4.0);
}

fn options(w: &mut MainWindow, ui: &mut Ui, pal: &Pal, info: &ModuleInfo, cfg: &ModuleConfig, col_w: f32) {
    let shared = w.shared.clone();
    let game = Game::World;
    let id = info.id.clone();
    let edit = |f: &dyn Fn(&mut ModuleConfig)| {
        let id = id.clone();
        shared.edit_settings(|s| f(s.profile_mut(game).module_mut(&id)));
    };
    let rw = (col_w - 66.0).max(80.0);

    field_label(ui, pal, &i18n::t("label.opacity"));
    let mut op = (cfg.opacity * 100.0).round();
    if widgets::range(ui, pal, "op", &mut op, 20.0, 100.0, 1.0, "%", rw) {
        edit(&|c| c.opacity = op / 100.0);
    }
    ui.add_space(18.0);
    field_label(ui, pal, &i18n::t("label.scale"));
    let mut sc = (cfg.scale * 100.0).round();
    if widgets::range(ui, pal, "sc", &mut sc, 50.0, 200.0, 5.0, "%", rw) {
        edit(&|c| c.scale = sc / 100.0);
    }
    ui.add_space(18.0);
    if info.id != modules::PLAYER {
        field_label(ui, pal, &i18n::t("label.backing"));
        let mut on = cfg.panel;
        if widgets::switch(ui, pal, &mut on).changed() {
            edit(&|c| c.panel = on);
        }
        ui.add_space(18.0);
    }

    match info.id.as_str() {
        modules::MONSTER => {
            let mo = shared.settings().profile(game).monster.clone();
            field_label(ui, pal, &i18n::t("label.target"));
            let modes = [TargetMode::Lock, TargetMode::LastHit, TargetMode::Manual];
            let mut sel = modes.iter().position(|m| *m == mo.target_mode).unwrap_or(0);
            if widgets::seg(
                ui,
                pal,
                &[i18n::t("modules.lock"), i18n::t("modules.last_hit"), i18n::t("modules.manual")],
                &mut sel,
            ) {
                shared.edit_settings(|s| s.profile_mut(game).monster.target_mode = modes[sel]);
            }
            ui.add_space(18.0);
            field_label(ui, pal, &i18n::t("label.position"));
            let mut sel = if mo.layout == MonsterLayout::Bottom { 0 } else { 1 };
            if widgets::seg(ui, pal, &[i18n::t("modules.bottom"), i18n::t("modules.side")], &mut sel) {
                shared.edit_settings(|s| {
                    let p = s.profile_mut(game);
                    p.monster.layout = if sel == 0 { MonsterLayout::Bottom } else { MonsterLayout::Side };
                    p.reset_module(game, modules::MONSTER);
                });
            }
            ui.add_space(18.0);
            field_label(ui, pal, &i18n::t("label.parts_shown"));
            let mut sel = if mo.all_parts { 0 } else { 1 };
            if widgets::seg(ui, pal, &[i18n::t("modules.parts_all"), i18n::t("modules.parts_active")], &mut sel) {
                shared.edit_settings(|s| s.profile_mut(game).monster.all_parts = sel == 0);
            }
            ui.add_space(18.0);
            field_label(ui, pal, &i18n::t("label.idle_fade"));
            let mut v = mo.idle_fade_secs;
            if widgets::range(ui, pal, "idle", &mut v, 2.0, 30.0, 1.0, &i18n::t("unit.seconds"), rw) {
                shared.edit_settings(|s| s.profile_mut(game).monster.idle_fade_secs = v);
            }
            ui.add_space(18.0);
        }
        modules::BUFFS => trays(w, ui, pal),
        modules::PLAYER => {
            let po = shared.settings().profile(game).player.clone();
            let set = |f: &dyn Fn(&mut rh_core::settings::PlayerOptions)| shared.edit_settings(|s| f(&mut s.profile_mut(game).player));
            field_label(ui, pal, &i18n::t("label.low_hp"));
            let mut v = po.low_hp;
            if widgets::range(ui, pal, "low", &mut v, 10.0, 50.0, 1.0, "%", rw) {
                set(&|p| p.low_hp = v);
            }
            ui.add_space(18.0);
            field_label(ui, pal, &i18n::t("label.backing"));
            let kinds = [Backing::None, Backing::Solid, Backing::Blur];
            let mut sel = kinds.iter().position(|k| *k == po.backing).unwrap_or(2);
            if widgets::seg(
                ui,
                pal,
                &[
                    i18n::t("modules.backing_none"),
                    i18n::t("modules.backing_solid"),
                    i18n::t("modules.backing_blur"),
                ],
                &mut sel,
            ) {
                set(&|p| p.backing = kinds[sel]);
            }
            ui.add_space(18.0);
            if po.backing == Backing::Blur {
                field_label(ui, pal, &i18n::t("label.blur"));
                let mut v = (po.blur_strength * 100.0).round();
                if widgets::range(ui, pal, "blur", &mut v, 10.0, 100.0, 5.0, "%", rw) {
                    set(&|p| p.blur_strength = v / 100.0);
                }
                ui.add_space(18.0);
            }
            if po.backing == Backing::Solid {
                field_label(ui, pal, &i18n::t("label.backing_op"));
                let mut v = (po.backing_opacity * 100.0).round();
                if widgets::range(ui, pal, "bop", &mut v, 0.0, 100.0, 1.0, "%", rw) {
                    set(&|p| p.backing_opacity = v / 100.0);
                }
                ui.add_space(18.0);
            }
            if po.backing != Backing::None {
                field_label(ui, pal, &i18n::t("label.backing_w"));
                let mut v = (po.backing_w * 100.0).round();
                if widgets::range(ui, pal, "bw", &mut v, 60.0, 140.0, 1.0, "%", rw) {
                    set(&|p| p.backing_w = v / 100.0);
                }
                ui.add_space(18.0);
                field_label(ui, pal, &i18n::t("label.backing_h"));
                let mut v = (po.backing_h * 100.0).round();
                if widgets::range(ui, pal, "bh", &mut v, 60.0, 180.0, 1.0, "%", rw) {
                    set(&|p| p.backing_h = v / 100.0);
                }
                ui.add_space(18.0);
            }
        }
        _ => {}
    }

    field_label(ui, pal, &i18n::t("label.conditions"));
    let mut v = cfg.hide_in_menu;
    if toggle_row(ui, pal, &i18n::t("modules.hide_in_menu"), &mut v) {
        edit(&|c| c.hide_in_menu = v);
    }
    let mut v = cfg.quest_only;
    if toggle_row(ui, pal, &i18n::t("modules.quest_only"), &mut v) {
        edit(&|c| c.quest_only = v);
    }
    if let Some(p) = &info.plugin {
        ui.add_space(12.0);
        label_mute(ui, pal, &format!("{} {p}", i18n::t("modules.plugin")), 12.0);
    }
}

/// Buff trays: each tray picks which buffs to show; none picked means all.
fn trays(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let game = Game::World;
    field_label(ui, pal, &i18n::t("label.trays"));
    let trays = shared.settings().profile(game).buffs.trays.clone();
    let i18n_cur = i18n::current();
    // Buffs to choose from: common ones and those active right now
    let mut options: Vec<(String, String)> = game_world::data::data()
        .abnormalities
        .iter()
        .filter(|a| {
            matches!(
                a.kind,
                rh_core::model::AbnormalityKind::Consumable
                    | rh_core::model::AbnormalityKind::Song
                    | rh_core::model::AbnormalityKind::Food
                    | rh_core::model::AbnormalityKind::Skill
            )
        })
        .map(|a| (a.id.clone(), i18n_cur.game(&a.key)))
        .collect();
    options.dedup_by(|a, b| a.1 == b.1);
    let mut changed: Option<Vec<Tray>> = None;
    for (ti, tray) in trays.iter().enumerate() {
        ui.horizontal(|ui| {
            widgets::label(
                ui,
                i18n::tf("label.tray_n", &[("n", &(ti + 1).to_string())]),
                Font::TextBold,
                widgets::T_BODY,
                pal.ink,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if trays.len() > 1 && link(ui, pal, &i18n::t("action.delete")).clicked() {
                    let mut t = trays.clone();
                    t.remove(ti);
                    changed = Some(t);
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (id, name) in &options {
                let on = tray.ids.contains(id);
                let g = widgets::galley(ui, name, Font::Text, widgets::T_SMALL, pal.ink);
                let (r, resp) = ui.allocate_exact_size(g.size() + vec2(18.0, 6.0), Sense::click());
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, if on { pal.ink } else { pal.line }),
                    egui::StrokeKind::Inside,
                );
                ui.painter().galley(r.min + vec2(9.0, 3.0), g, if on { pal.ink } else { pal.mute });
                if resp.clicked() {
                    let mut t = trays.clone();
                    if !t[ti].ids.remove(id) {
                        t[ti].ids.insert(id.clone());
                    }
                    changed = Some(t);
                }
            }
        });
        ui.add_space(16.0);
    }
    if link(ui, pal, &i18n::t("action.new_tray")).clicked() {
        let mut t = trays.clone();
        t.push(Tray {
            name: (t.len() + 1).to_string(),
            ids: Default::default(),
        });
        changed = Some(t);
    }
    if let Some(t) = changed {
        shared.edit_settings(|s| s.profile_mut(game).buffs.trays = t);
    }
    ui.add_space(18.0);
}
