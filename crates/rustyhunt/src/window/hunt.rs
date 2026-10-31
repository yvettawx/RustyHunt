//! Hunt view: the overlay's modules on the main window, for a second monitor.

use std::collections::HashMap;

use egui::{Ui, vec2};
use rh_core::i18n;
use rh_core::modules;

use super::MainWindow;
use super::modules::preview_backdrop;
use super::style::Pal;
use super::widgets;
use crate::overlay::Preview;

#[derive(Default)]
pub struct State {
    previews: HashMap<&'static str, Preview>,
}

const ROWS: [(&[&str], f32); 4] = [
    (&[modules::MONSTER], 250.0),
    (&[modules::PLAYER, modules::DAMAGE], 130.0),
    (&[modules::BUFFS, modules::WEAPON], 110.0),
    (&[modules::CLOCK, modules::AREA], 110.0),
];

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    let mut follow = shared.settings().main_window_follow;
    widgets::page_header(ui, pal, &i18n::t("nav.hunt"), |ui| {
        if widgets::switch(ui, pal, &mut follow).changed() {
            shared.edit_settings(|s| s.main_window_follow = follow);
        }
        widgets::label_mute(ui, pal, &i18n::t("hunt.follow"), 12.0);
    });

    let live = shared.live().clone();
    let hunting = live.has_data() && !live.snapshot.loading && (live.snapshot.in_quest() || live.snapshot.in_hunt_zone);
    if !hunting {
        widgets::label_mute(ui, pal, &i18n::t("hunt.empty"), 13.0);
        return;
    }

    let width = ui.available_width();
    let gap = 8.0;
    ui.spacing_mut().item_spacing = vec2(gap, gap);
    for (ids, height) in ROWS {
        ui.horizontal(|ui| {
            let cell = (width - gap * (ids.len() - 1) as f32) / ids.len() as f32;
            for id in ids {
                let (rect, _) = ui.allocate_exact_size(vec2(cell, height), egui::Sense::hover());
                preview_backdrop(ui, pal, rect);
                w.hunt.previews.entry(id).or_default().draw(ui, rect, &shared, id, false);
            }
        });
    }
}
