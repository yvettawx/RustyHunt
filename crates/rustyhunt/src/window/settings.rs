//! 06 Settings: general, hotkeys, performance, files, injection, notifications, language, about.

use egui::{Align2, CornerRadius, Rect, Sense, Ui, pos2, vec2};
use rh_core::i18n::{self, Lang};
use rh_core::settings::LogLevel;
use ui_kit::Font;

use super::MainWindow;
use super::style::Pal;
use super::widgets::{self, Btn, button, key_box, label, label_mute, link, setting_row, switch, text_at};
use crate::state::Shared;

#[derive(Default)]
pub struct State {
    pub tab: usize,
    /// The hotkey being recorded
    capturing: Option<String>,
    capture_armed: bool,
}

const TABS: [&str; 8] = [
    "settings.general",
    "settings.hotkeys",
    "settings.perf",
    "settings.files",
    "settings.injection",
    "settings.notify",
    "settings.language",
    "settings.about",
];

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    widgets::page_header(ui, pal, &i18n::t("nav.settings"), |_| {});
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 48.0;
        ui.vertical(|ui| {
            ui.set_width(120.0);
            for (i, t) in TABS.iter().enumerate() {
                let on = w.settings.tab == i;
                let (r, resp) = ui.allocate_exact_size(vec2(120.0, 32.0), Sense::click());
                let color = if on || resp.hovered() { pal.ink } else { pal.mute };
                text_at(
                    ui,
                    pos2(r.min.x + 14.0, r.center().y),
                    Align2::LEFT_CENTER,
                    &i18n::t(t),
                    if on { Font::TextBold } else { Font::Text },
                    14.0,
                    color,
                );
                if on {
                    ui.painter()
                        .rect_filled(Rect::from_min_size(pos2(r.min.x, r.min.y + 9.0), vec2(2.0, 14.0)), CornerRadius::ZERO, pal.ink);
                }
                if resp.clicked() {
                    w.settings.tab = i;
                    w.settings.capturing = None;
                }
            }
        });
        ui.vertical(|ui| {
            ui.set_max_width(640.0);
            match w.settings.tab {
                0 => general(w, ui, pal),
                1 => hotkeys(w, ui, pal),
                2 => perf(w, ui, pal),
                3 => files(w, ui, pal),
                4 => injection(w, ui, pal),
                5 => notify(w, ui, pal),
                6 => language(w, ui, pal),
                _ => about(w, ui, pal),
            }
        });
    });
}

fn sw(ui: &mut Ui, pal: &Pal, shared: &Shared, get: impl Fn(&rh_core::settings::Settings) -> bool, set: impl FnOnce(&mut rh_core::settings::Settings, bool)) {
    let mut v = get(&shared.settings());
    if switch(ui, pal, &mut v).changed() {
        shared.edit_settings(|s| set(s, v));
    }
}

fn general(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let mut rerun = false;
    setting_row(ui, pal, &i18n::t("settings.onboarding"), None, |ui| {
        rerun = link(ui, pal, &i18n::t("action.rerun")).clicked();
    });
    if rerun {
        w.open_onboarding();
    }
    setting_row(ui, pal, &i18n::t("settings.autostart"), None, |ui| {
        let mut v = shared.settings().autostart;
        if switch(ui, pal, &mut v).changed() {
            shared.edit_settings(|s| s.autostart = v);
            crate::win::set_autostart(v);
        }
    });
    setting_row(ui, pal, &i18n::t("settings.on_close"), None, |ui| {
        let mut sel = if shared.settings().close_to_tray { 0 } else { 1 };
        if widgets::seg(ui, pal, &[i18n::t("settings.to_tray"), i18n::t("settings.quit")], &mut sel) {
            shared.edit_settings(|s| s.close_to_tray = sel == 0);
        }
    });
    setting_row(ui, pal, &i18n::t("settings.auto_connect"), None, |ui| {
        sw(ui, pal, &shared, |s| s.auto_connect, |s, v| s.auto_connect = v);
    });
    setting_row(ui, pal, &i18n::t("settings.hide_in_menu"), None, |ui| {
        let game = rh_core::model::Game::World;
        let mut v = shared.settings().profile(game).modules.values().all(|m| m.hide_in_menu);
        if switch(ui, pal, &mut v).changed() {
            shared.edit_settings(|s| {
                for m in s.profile_mut(game).modules.values_mut() {
                    m.hide_in_menu = v;
                }
            });
        }
    });
    setting_row(ui, pal, &i18n::t("settings.demo"), None, |ui| {
        sw(ui, pal, &shared, |s| s.demo, |s, v| s.demo = v);
    });
}

fn hotkeys(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let hk = shared.settings().hotkeys.clone();
    let rows: [(&str, &str, String); 5] = [
        ("toggle_overlay", "hotkey.toggle_overlay", hk.toggle_overlay.clone()),
        ("layout_mode", "hotkey.layout_mode", hk.layout_mode.clone()),
        ("density", "hotkey.density", hk.density.clone()),
        ("show_all", "hotkey.show_all", hk.show_all.clone()),
        ("cycle_target", "hotkey.cycle_target", hk.cycle_target.clone()),
    ];
    // Recording: after all keys are released, the first combination pressed becomes the new hotkey
    if let Some(id) = w.settings.capturing.clone() {
        let any = platform::keys::capture();
        if !w.settings.capture_armed {
            if any.is_none() {
                w.settings.capture_armed = true;
            }
        } else if let Some(k) = any {
            if k.key == platform::keys::VK_ESCAPE {
                w.settings.capturing = None;
            } else if !(k.key == 0x01) {
                let s = k.to_string();
                shared.edit_settings(|set| match id.as_str() {
                    "toggle_overlay" => set.hotkeys.toggle_overlay = s,
                    "layout_mode" => set.hotkeys.layout_mode = s,
                    "density" => set.hotkeys.density = s,
                    "show_all" => set.hotkeys.show_all = s,
                    "cycle_target" => set.hotkeys.cycle_target = s,
                    other => {
                        set.hotkeys.plugin.insert(other.to_string(), s);
                    }
                });
                w.settings.capturing = None;
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }
    for (id, key, value) in rows {
        let capturing = w.settings.capturing.as_deref() == Some(id);
        setting_row(ui, pal, &i18n::t(key), None, |ui| {
            let text = if capturing { i18n::t("hotkey.press") } else { value };
            if key_box(ui, pal, &text, capturing).clicked() {
                w.settings.capturing = Some(id.to_string());
                w.settings.capture_armed = false;
            }
        });
    }
    let actions = crate::plugins::actions();
    if !actions.is_empty() {
        ui.add_space(26.0);
        label_mute(ui, pal, &i18n::t("label.plugins"), 12.0);
        for a in actions {
            let id = format!("{}.{}", a.plugin, a.id);
            let capturing = w.settings.capturing.as_deref() == Some(id.as_str());
            let value = hk.plugin.get(&id).cloned().unwrap_or_else(|| a.default_key.clone());
            setting_row(ui, pal, &a.name, Some(&a.plugin_name), |ui| {
                let text = if capturing {
                    i18n::t("hotkey.press")
                } else if value.is_empty() {
                    "—".into()
                } else {
                    value
                };
                if key_box(ui, pal, &text, capturing).clicked() {
                    w.settings.capturing = Some(id.clone());
                    w.settings.capture_armed = false;
                }
            });
        }
    }
}

fn perf(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    setting_row(ui, pal, &i18n::t("settings.read_hz"), None, |ui| {
        let mut v = shared.settings().perf.read_hz as f32;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            if widgets::range(ui, pal, "hz", &mut v, 5.0, 60.0, 1.0, "Hz", 240.0) {
                shared.edit_settings(|s| s.perf.read_hz = v as u32);
            }
        });
    });
    setting_row(ui, pal, &i18n::t("settings.fps_cap"), None, |ui| {
        let mut v = shared.settings().perf.fps_cap as f32;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            if widgets::range(ui, pal, "fps", &mut v, 30.0, 144.0, 1.0, "FPS", 240.0) {
                shared.edit_settings(|s| s.perf.fps_cap = v as u32);
            }
        });
    });
    setting_row(ui, pal, &i18n::t("settings.animations"), None, |ui| {
        sw(ui, pal, &shared, |s| s.perf.animations, |s, v| s.perf.animations = v);
    });
}

fn files(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let path_row = |ui: &mut Ui, label: &str, path: std::path::PathBuf| {
        setting_row(ui, pal, label, None, |ui| {
            if link(ui, pal, &i18n::t("action.open")).clicked() {
                crate::win::open(&path);
            }
            ui.add(egui::Label::new(widgets::rich(short_path(&path), Font::Mono, widgets::T_SMALL, pal.mute)).truncate());
        });
    };
    path_row(ui, &i18n::t("settings.maps"), rh_core::paths::maps());
    path_row(ui, &i18n::t("settings.records"), rh_core::paths::records());
    path_row(ui, &i18n::t("settings.data_dir"), rh_core::paths::root().to_path_buf());
    setting_row(ui, pal, &i18n::t("settings.log_level"), None, |ui| {
        let levels = [LogLevel::Debug, LogLevel::Info, LogLevel::Warn];
        let mut sel = levels.iter().position(|l| *l == shared.settings().log_level).unwrap_or(1);
        if widgets::seg(ui, pal, &["DEBUG".into(), "INFO".into(), "WARN".into()], &mut sel) {
            shared.edit_settings(|s| s.log_level = levels[sel]);
            crate::logging::set_level(levels[sel]);
        }
    });
    setting_row(ui, pal, &i18n::t("settings.backup"), None, |ui| {
        if link(ui, pal, &i18n::t("action.import")).clicked()
            && let Some(p) = rfd::FileDialog::new().add_filter("TOML", &["toml"]).pick_file()
        {
            match std::fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|t| toml::from_str::<rh_core::settings::Settings>(&t).map_err(|e| e.to_string()))
            {
                Ok(mut s) => {
                    s.normalize();
                    *shared.settings.write().unwrap() = s;
                    shared.edit_settings(|_| {});
                    log::info!("imported settings {}", p.display());
                }
                Err(e) => log::warn!("failed to import settings: {e}"),
            }
        }
        if link(ui, pal, &i18n::t("action.export")).clicked()
            && let Some(p) = rfd::FileDialog::new().set_file_name("rustyhunt-settings.toml").save_file()
            && let Err(e) = shared.settings().save(&p)
        {
            log::warn!("failed to export settings: {e}");
        }
    });
}

fn short_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if let Ok(appdata) = std::env::var("APPDATA") {
        return s.replace(&appdata, "%APPDATA%");
    }
    s
}

fn injection(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let st = crate::injection::status();
    let inj = shared.settings().injection.clone();
    let status = |ui: &mut Ui, s: &crate::injection::ItemStatus| {
        let (text, ok) = match s {
            crate::injection::ItemStatus::Off => (i18n::t("inj.off"), false),
            crate::injection::ItemStatus::Waiting => (i18n::t("inj.waiting"), false),
            crate::injection::ItemStatus::Active(t) => (t.clone(), true),
            crate::injection::ItemStatus::Failed(e) => (i18n::tf("inj.failed", &[("e", e)]), false),
        };
        let g = widgets::galley(ui, &text, if ok { Font::TextBold } else { Font::Text }, 12.0, pal.ink);
        let (r, _) = ui.allocate_exact_size(vec2(g.size().x + if ok { 13.0 } else { 0.0 }, 16.0), Sense::hover());
        if ok {
            ui.painter().circle_filled(pos2(r.min.x + 3.0, r.center().y), 3.0, pal.ok);
        }
        let color = if ok {
            pal.ink
        } else if matches!(s, crate::injection::ItemStatus::Failed(_)) {
            pal.accent
        } else {
            pal.mute
        };
        ui.painter().galley(pos2(r.max.x - g.size().x, r.center().y - g.size().y / 2.0), g, color);
    };
    let mut ask_patch = false;
    let mut ask_hook = false;
    setting_row(ui, pal, &i18n::t("inj.crc"), None, |ui| {
        let mut v = inj.crc_patch;
        if switch(ui, pal, &mut v).clicked() {
            if v {
                ask_patch = true;
            } else {
                shared.edit_settings(|s| {
                    s.injection.crc_patch = false;
                    s.injection.damage_hook = false;
                });
            }
        }
        status(ui, &st.crc);
    });
    ui.add_enabled_ui(inj.crc_patch, |ui| {
        setting_row(ui, pal, &i18n::t("inj.hook"), None, |ui| {
            let mut v = inj.damage_hook;
            if switch(ui, pal, &mut v).clicked() {
                if v {
                    ask_hook = true;
                } else {
                    shared.edit_settings(|s| s.injection.damage_hook = false);
                }
            }
            status(ui, &st.hook);
        });
    });
    setting_row(ui, pal, &i18n::t("inj.solo_only"), None, |ui| {
        sw(ui, pal, &shared, |s| s.injection.solo_only, |s, v| s.injection.solo_only = v);
    });
    if ask_patch {
        w.ask(
            i18n::t("inj.ask_crc_title"),
            vec![i18n::t("inj.ask_crc_1"), i18n::t("inj.ask_risk")],
            i18n::t("action.enable"),
            |s| s.edit_settings(|set| set.injection.crc_patch = true),
        );
    }
    if ask_hook {
        w.ask(
            i18n::t("inj.ask_hook_title"),
            vec![i18n::t("inj.ask_hook_1"), i18n::t("inj.ask_risk")],
            i18n::t("action.enable"),
            |s| s.edit_settings(|set| set.injection.damage_hook = true),
        );
    }
    ui.add_space(34.0);
    let kv = |ui: &mut Ui, k: &str, v: &str| {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(640.0), 38.0), Sense::hover());
        text_at(ui, pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, k, Font::Text, widgets::T_BODY, pal.mute);
        text_at(ui, pos2(r.max.x, r.center().y), Align2::RIGHT_CENTER, v, Font::Mono, widgets::T_SMALL, pal.ink);
        ui.painter()
            .line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], egui::Stroke::new(1.0, pal.line));
    };
    let live = shared.live();
    kv(ui, &i18n::t("home.process"), if live.exe.is_empty() { "-" } else { &live.exe });
    drop(live);
    kv(ui, "DLL", &st.dll);
    kv(ui, "IPC", &st.ipc);
    kv(ui, &i18n::t("inj.last_failure"), st.last_failure.as_deref().unwrap_or("-"));
}

fn notify(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    setting_row(ui, pal, &i18n::t("notify.toast"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.toast, |s, v| s.notify.toast = v)
    });
    setting_row(ui, pal, &i18n::t("notify.sound"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.sound, |s, v| s.notify.sound = v)
    });
    setting_row(ui, pal, &i18n::t("notify.in_game"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.in_game, |s, v| s.notify.in_game = v)
    });
    let channels = crate::plugins::channels();
    if !channels.is_empty() {
        ui.add_space(26.0);
        label_mute(ui, pal, &i18n::t("label.plugins"), 12.0);
        for c in channels {
            let key = format!("{}.{}", c.plugin, c.id);
            setting_row(ui, pal, &c.name, Some(&c.plugin_name), |ui| {
                let mut v = shared.settings().notify.channels.get(&key).copied().unwrap_or(false);
                if switch(ui, pal, &mut v).changed() {
                    let k = key.clone();
                    shared.edit_settings(|s| {
                        s.notify.channels.insert(k, v);
                    });
                }
            });
        }
    }
    ui.add_space(26.0);
    label_mute(ui, pal, &i18n::t("notify.which"), 12.0);
    setting_row(ui, pal, &i18n::t("notify.kind_update"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.update, |s, v| s.notify.update = v)
    });
    setting_row(ui, pal, &i18n::t("notify.kind_map"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.map_mismatch, |s, v| s.notify.map_mismatch = v)
    });
    setting_row(ui, pal, &i18n::t("notify.kind_crown"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.crown, |s, v| s.notify.crown = v)
    });
    setting_row(ui, pal, &i18n::t("notify.kind_disconnect"), None, |ui| {
        sw(ui, pal, &shared, |s| s.notify.disconnect, |s, v| s.notify.disconnect = v)
    });
}

fn language(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let names: Vec<String> = Lang::ALL.iter().map(|l| l.native_name().to_string()).collect();
    setting_row(ui, pal, &i18n::t("lang.ui"), None, |ui| {
        let mut sel = Lang::ALL.iter().position(|l| *l == shared.settings().language).unwrap_or(0);
        if widgets::select(ui, pal, "ui-lang", &names, &mut sel, 140.0) {
            shared.edit_settings(|s| s.language = Lang::ALL[sel]);
            reload_i18n(&shared);
        }
    });
    setting_row(ui, pal, &i18n::t("lang.terms"), None, |ui| {
        let mut opts = vec![i18n::t("lang.follow_ui")];
        opts.extend(names.iter().cloned());
        let mut sel = shared
            .settings()
            .term_language
            .and_then(|t| Lang::ALL.iter().position(|l| *l == t))
            .map_or(0, |i| i + 1);
        if widgets::select(ui, pal, "term-lang", &opts, &mut sel, 140.0) {
            shared.edit_settings(|s| s.term_language = if sel == 0 { None } else { Some(Lang::ALL[sel - 1]) });
            reload_i18n(&shared);
        }
    });
    ui.add_space(26.0);
    label_mute(ui, pal, &i18n::t("lang.installed"), 12.0);
    let cur = i18n::current();
    for l in Lang::ALL {
        setting_row(ui, pal, l.native_name(), None, |ui| {
            label(
                ui,
                format!("{}%", (cur.ui_coverage(l) * 100.0).round()),
                Font::NumBold,
                widgets::T_SMALL,
                pal.ink,
            );
            label_mute(ui, pal, &i18n::t("lang.builtin"), 12.0);
        });
    }
    for (code, path) in installed_packs() {
        setting_row(ui, pal, &code, None, |ui| {
            if link(ui, pal, &i18n::t("action.delete")).clicked() {
                let _ = std::fs::remove_file(&path);
                reload_i18n(&shared);
            }
            label_mute(ui, pal, &i18n::t("lang.community"), 12.0);
        });
    }
    setting_row(ui, pal, &i18n::t("lang.terms_table"), None, |ui| {
        label_mute(ui, pal, &i18n::t("lang.terms_world"), 12.0);
    });
    ui.add_space(22.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        if link(ui, pal, &i18n::t("lang.import")).clicked()
            && let Some(p) = rfd::FileDialog::new().add_filter("TOML", &["toml"]).pick_file()
            && let Some(name) = p.file_name()
        {
            let _ = std::fs::copy(&p, rh_core::paths::locales().join(name));
            reload_i18n(&shared);
        }
        if link(ui, pal, &i18n::t("action.open_folder")).clicked() {
            crate::win::open(&rh_core::paths::locales());
        }
    });
}

/// Language packs imported by the user: `<lang>.toml` under locales in the data directory, overriding or adding to the bundled copy.
fn installed_packs() -> Vec<(String, std::path::PathBuf)> {
    std::fs::read_dir(rh_core::paths::locales())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .map(|p| (p.file_stem().unwrap_or_default().to_string_lossy().into_owned(), p))
        .collect()
}

pub fn reload_i18n(shared: &Shared) {
    let s = shared.settings();
    let mut i = i18n::load_bundled(&rh_core::paths::bundled("locales"), s.language, s.term_language);
    for (code, path) in installed_packs() {
        if let (Some(lang), Ok(text)) = (Lang::from_code(&code), std::fs::read_to_string(&path))
            && let Ok(t) = i18n::Table::parse(&text)
        {
            i.load_ui(lang, t);
        }
    }
    drop(s);
    i18n::install(i);
    shared.repaint();
}

fn about(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    label(ui, "RustyHunt", Font::NumHeavy, widgets::T_TITLE, pal.ink);
    label(ui, rh_core::VERSION, Font::NumBold, widgets::T_BODY, pal.ink);
    ui.add_space(22.0);
    for k in ["about.based_on", "about.unaffiliated", "about.capcom"] {
        label_mute(ui, pal, &i18n::t(k), 13.0);
        ui.add_space(6.0);
    }
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        if link(ui, pal, &i18n::t("action.check_update")).clicked() {
            let s = w.shared.clone();
            std::thread::spawn(move || crate::update::check(&s, false));
            w.open_update();
        }
        if link(ui, pal, &i18n::t("about.changelog")).clicked() {
            w.open_update();
        }
        if link(ui, pal, "LICENSE").clicked() {
            crate::win::open(&rh_core::paths::bundled("LICENSE"));
        }
        if link(ui, pal, "NOTICE").clicked() {
            crate::win::open(&rh_core::paths::bundled("NOTICE"));
        }
    });
    let _ = button;
    let _ = Btn::Outline;
}
