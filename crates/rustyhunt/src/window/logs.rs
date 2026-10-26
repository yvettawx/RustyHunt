//! 07 日志：按级别筛选、搜索、复制、打开日志文件。

use egui::{Color32, Ui, vec2};
use log::Level;
use rh_core::i18n;
use ui_kit::Font;

use super::MainWindow;
use super::style::Pal;
use super::widgets::{self, Btn, button};

#[derive(Default)]
pub struct State {
    level: usize,
    query: String,
}

const LEVELS: [Option<Level>; 5] = [None, Some(Level::Info), Some(Level::Warn), Some(Level::Error), Some(Level::Debug)];

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let st = &mut w.logs;
    let lines = crate::logging::lines();
    let filtered: Vec<&crate::logging::Line> = lines
        .iter()
        .filter(|l| LEVELS[st.level].is_none_or(|lv| l.level == lv))
        .filter(|l| st.query.is_empty() || l.message.contains(&st.query) || l.target.contains(&st.query))
        .collect();
    widgets::page_header(ui, pal, &i18n::t("nav.logs"), |ui| {
        if button(ui, pal, &i18n::t("action.open_file"), Btn::Outline, false).clicked() {
            crate::win::open(&crate::logging::file_path());
        }
        if button(ui, pal, &i18n::t("action.copy"), Btn::Outline, false).clicked() {
            let text: String = filtered.iter().map(|l| l.format() + "\n").collect();
            ui.ctx().copy_text(text);
        }
    });
    ui.horizontal(|ui| {
        let items: Vec<(String, bool)> = [i18n::t("label.all"), "INFO".into(), "WARN".into(), "ERROR".into(), "DEBUG".into()]
            .into_iter()
            .map(|s| (s, true))
            .collect();
        if let Some(i) = widgets::tabs(ui, pal, &items, st.level, false) {
            st.level = i;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            widgets::text_input(ui, pal, &mut st.query, &i18n::t("label.search"), 220.0);
        });
    });
    ui.add_space(12.0);
    let h = (ui.available_height()).max(560.0);
    egui::Frame::new()
        .fill(Color32::from_rgb(0x14, 0x14, 0x14))
        .inner_margin(egui::Margin::symmetric(18, 16))
        .show(ui, |ui| {
            ui.set_min_height(h);
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("logview")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .max_height(h)
                .show_rows(ui, 21.6, filtered.len(), |ui, range| {
                    for l in &filtered[range] {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let col = |ui: &mut Ui, text: &str, w: f32, c: Color32| {
                                let (r, _) = ui.allocate_exact_size(vec2(w, 18.0), egui::Sense::hover());
                                ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, text, Font::Mono.id(12.0), c);
                            };
                            col(ui, &l.time, 96.0, Color32::from_rgb(0x6a, 0x68, 0x62));
                            let lc = match l.level {
                                Level::Info => Color32::from_rgb(0xb9, 0xd8, 0xc6),
                                Level::Warn => pal.accent,
                                Level::Error => Color32::from_rgb(0xff, 0x70, 0x66),
                                _ => Color32::from_rgb(0x6a, 0x68, 0x62),
                            };
                            col(ui, &l.level.to_string(), 46.0, lc);
                            col(ui, &l.target, 96.0, Color32::from_rgb(0x8b, 0x89, 0x83));
                            ui.add(egui::Label::new(egui::RichText::new(&l.message).font(Font::Mono.id(12.0)).color(Color32::from_rgb(0xcf, 0xcd, 0xc6))).truncate());
                        });
                    }
                });
        });
}
