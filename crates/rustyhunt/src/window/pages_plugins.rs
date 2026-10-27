//! 05 Plugins: installed list, enable, disable, permissions, settings, delete; install from file.

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use plugin_host::Permission;
use plugin_host::manifest::SettingKind;
use rh_core::i18n;
use ui_kit::Font;

use super::style::Pal;
use super::widgets::{self, Btn, button, label, label_mute, link, switch, text_at};
use super::{MainWindow, Modal, Page};
use crate::plugins::{self, Status, View};

#[derive(Default)]
pub struct State {
    selected: usize,
    error: Option<String>,
}

fn perm_name(p: Permission) -> String {
    i18n::t(&format!("plugin.perm_{}", p.key()))
}

fn event_name(e: &str) -> String {
    i18n::t(&format!("event.{e}"))
}

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    widgets::page_header(ui, pal, &i18n::t("nav.plugins"), |ui| {
        if button(ui, pal, &i18n::t("action.install"), Btn::Primary, false).clicked()
            && let Some(p) = rfd::FileDialog::new().add_filter("zip", &["zip"]).pick_file()
        {
            ask_install(w, &p);
        }
        if button(ui, pal, &i18n::t("action.open_folder"), Btn::Outline, false).clicked() {
            let dir = rh_core::paths::plugins();
            let _ = std::fs::create_dir_all(&dir);
            crate::win::open(&dir);
        }
    });
    if let Some(e) = &w.plugins.error {
        label(ui, e, Font::Text, 13.0, pal.accent);
        ui.add_space(14.0);
    }
    let list = plugins::views();
    if list.is_empty() {
        label_mute(ui, pal, &i18n::t("plugin.empty"), 13.0);
        return;
    }
    if w.plugins.selected >= list.len() {
        w.plugins.selected = 0;
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 48.0;
        ui.vertical(|ui| {
            ui.set_width(300.0);
            for (i, v) in list.iter().enumerate() {
                list_item(w, ui, pal, i, v);
            }
        });
        ui.vertical(|ui| {
            ui.set_width((ui.available_width()).min(640.0));
            let v = list[w.plugins.selected].clone();
            detail(w, ui, pal, &v);
        });
    });
}

fn list_item(w: &mut MainWindow, ui: &mut Ui, pal: &Pal, i: usize, v: &View) {
    let (r, resp) = ui.allocate_exact_size(vec2(300.0, 64.0), Sense::click());
    let on = i == w.plugins.selected;
    if on {
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(r.min.x, r.min.y + 14.0), vec2(2.0, r.height() - 28.0)),
            CornerRadius::ZERO,
            pal.ink,
        );
    }
    let m = &v.manifest;
    text_at(
        ui,
        pos2(r.min.x + 14.0, r.min.y + 22.0),
        Align2::LEFT_CENTER,
        &m.name,
        if on { Font::TextBold } else { Font::Text },
        15.0,
        pal.ink,
    );
    let (sub, err) = match &v.status {
        Status::Failed(_) => (i18n::t("state.disabled"), true),
        Status::Incompatible(e) | Status::Broken(e) => (e.clone(), true),
        _ => (m.author.clone(), false),
    };
    text_at(
        ui,
        pos2(r.min.x + 14.0, r.min.y + 44.0),
        Align2::LEFT_CENTER,
        &sub,
        Font::Text,
        12.0,
        if err { pal.accent } else { pal.mute },
    );
    ui.painter()
        .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
    let sr = Rect::from_min_size(pos2(r.max.x - 28.0, r.center().y - 7.0), vec2(28.0, 14.0));
    let mut enabled = v.enabled;
    let mut changed = false;
    let usable = !matches!(v.status, Status::Incompatible(_) | Status::Broken(_));
    ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
        ui.add_enabled_ui(usable, |ui| changed = switch(ui, pal, &mut enabled).changed());
    });
    if changed {
        plugins::set_enabled(&m.id, enabled);
    } else if resp.clicked() {
        w.plugins.selected = i;
    }
}

fn row(ui: &mut Ui, pal: &Pal, text: &str, right: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    text_at(ui, pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, text, Font::Text, 13.0, pal.ink);
    ui.painter()
        .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
    let cr = Rect::from_min_max(pos2(r.min.x + r.width() * 0.4, r.min.y), r.max);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(cr).layout(egui::Layout::right_to_left(egui::Align::Center)),
        right,
    );
}

fn group(ui: &mut Ui, pal: &Pal, title: &str) {
    ui.add_space(30.0);
    label_mute(ui, pal, title, 12.0);
    ui.add_space(2.0);
}

fn detail(w: &mut MainWindow, ui: &mut Ui, pal: &Pal, v: &View) {
    let m = &v.manifest;
    label(ui, &m.name, Font::TextBold, 24.0, pal.ink);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if !m.author.is_empty() {
            label_mute(ui, pal, &m.author, 12.0);
        }
        label(ui, &m.version, Font::NumBold, 12.0, pal.mute);
    });
    if !m.description.is_empty() {
        ui.add_space(16.0);
        ui.add(egui::Label::new(widgets::rich(&m.description, Font::Text, 13.0, pal.ink)).wrap());
    }
    let err = match &v.status {
        Status::Failed(_) => Some(i18n::t("plugin.errored")),
        Status::Incompatible(e) | Status::Broken(e) => Some(e.clone()),
        _ => None,
    };
    if let Some(e) = err {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            label(ui, &e, Font::Text, 13.0, pal.accent);
            if matches!(v.status, Status::Failed(_)) && link(ui, pal, &i18n::t("action.view_logs")).clicked() {
                w.page = Page::Logs;
            }
        });
    }

    group(ui, pal, &i18n::t("label.permissions"));
    if m.permissions.is_empty() {
        row(ui, pal, &i18n::t("label.none"), |_| {});
    }
    for &p in &m.permissions {
        let mut granted = v.granted.contains(p.key());
        let mut changed = false;
        row(ui, pal, &perm_name(p), |ui| changed = switch(ui, pal, &mut granted).changed());
        if changed {
            if granted {
                // Granting a permission needs confirmation
                let id = m.id.clone();
                w.modal = Some(Modal {
                    title: i18n::tf("plugin.ask_perm", &[("perm", &perm_name(p))]),
                    lines: vec![format!("{} {}", m.name, m.version), i18n::tf("plugin.ask_install_by", &[("author", &m.author)])],
                    yes: i18n::t("action.allow"),
                    on_yes: Box::new(move |_| plugins::set_permission(&id, p, true)),
                });
            } else {
                plugins::set_permission(&m.id, p, false);
            }
        }
    }

    group(ui, pal, &i18n::t("label.overlay_modules"));
    if m.modules.is_empty() {
        row(ui, pal, &i18n::t("label.none"), |_| {});
    }
    for d in &m.modules {
        let mut go = false;
        row(ui, pal, &d.name, |ui| go = link(ui, pal, &i18n::t("plugin.open_settings")).clicked());
        if go {
            w.page = Page::Modules;
        }
    }

    group(ui, pal, &i18n::t("label.events"));
    let events = if m.events.is_empty() {
        i18n::t("label.none")
    } else {
        m.events.iter().map(|e| event_name(e)).collect::<Vec<_>>().join("、")
    };
    row(ui, pal, &events, |_| {});

    if !m.settings.is_empty() {
        group(ui, pal, &i18n::t("nav.settings"));
        let st = w.shared.settings().plugins.get(&m.id).cloned();
        for s in &m.settings {
            let cur = plugins::setting_value(st.as_ref(), s);
            row(ui, pal, &s.name, |ui| match &s.kind {
                SettingKind::Bool { .. } => {
                    let mut b = cur.as_bool().unwrap_or(false);
                    if switch(ui, pal, &mut b).changed() {
                        plugins::set_setting(&m.id, &s.key, toml::Value::Boolean(b));
                    }
                }
                SettingKind::Number { min, max, .. } => {
                    let mut x = cur.as_f64().unwrap_or(*min) as f32;
                    let whole = min.fract() == 0.0 && max.fract() == 0.0;
                    let step = if whole { 1.0 } else { ((max - min) / 100.0) as f32 };
                    ui.allocate_ui_with_layout(vec2(246.0, 40.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if widgets::range(ui, pal, &s.key, &mut x, *min as f32, *max as f32, step, "", 180.0) {
                            let v = if whole {
                                toml::Value::Integer(x.round() as i64)
                            } else {
                                toml::Value::Float(x as f64)
                            };
                            plugins::set_setting(&m.id, &s.key, v);
                        }
                    });
                }
                SettingKind::Text { .. } => {
                    let mut t = cur.as_str().unwrap_or_default().to_string();
                    if widgets::text_input(ui, pal, &mut t, "", 220.0).changed() {
                        plugins::set_setting(&m.id, &s.key, toml::Value::String(t));
                    }
                }
            });
        }
    }

    ui.add_space(30.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        if link(ui, pal, &i18n::t("action.open_folder")).clicked() {
            crate::win::open(&v.dir);
        }
        if link(ui, pal, &i18n::t("action.delete")).clicked() {
            let id = m.id.clone();
            w.modal = Some(Modal {
                title: i18n::tf("plugin.ask_delete", &[("name", &m.name)]),
                lines: vec![v.dir.display().to_string()],
                yes: i18n::t("action.delete"),
                on_yes: Box::new(move |_| {
                    if let Err(e) = plugins::uninstall(&id) {
                        log::warn!(target: "plugin", "删除 {id}：{e}");
                    }
                }),
            });
        }
    });
}

/// Install from file: read the manifest first, show the author and permissions in a dialog, install after confirmation.
fn ask_install(w: &mut MainWindow, path: &std::path::Path) {
    match plugins::prepare(path) {
        Ok(staged) => {
            w.plugins.error = None;
            let m = staged.manifest.clone();
            let mut lines = Vec::new();
            if !m.author.is_empty() {
                lines.push(i18n::tf("plugin.ask_install_by", &[("author", &m.author)]));
            }
            if !m.permissions.is_empty() {
                let names: Vec<String> = m.permissions.iter().map(|p| perm_name(*p)).collect();
                lines.push(format!("{}：{}", i18n::t("label.permissions"), names.join("、")));
            }
            w.modal = Some(Modal {
                title: i18n::tf("plugin.ask_install", &[("name", &m.name), ("version", &m.version)]),
                lines,
                yes: i18n::t("action.install_yes"),
                on_yes: Box::new(move |_| {
                    if let Err(e) = plugins::install(staged) {
                        log::warn!(target: "plugin", "安装失败：{e}");
                    }
                }),
            });
        }
        Err(e) => w.plugins.error = Some(e),
    }
}
