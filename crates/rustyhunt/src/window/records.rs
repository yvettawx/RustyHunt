//! 03 记录：狩猎和图鉴两个页签。

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use rh_core::i18n;
use ui_kit::Font;

use super::MainWindow;
use super::style::Pal;
use super::widgets::{self, Btn, button, label, label_mute, link, switch, text_at};
use crate::overlay::mmss;
use crate::records::Hunt;

#[derive(Default)]
pub struct State {
    pub tab: usize,
    filter: usize,
    hunts: Vec<Hunt>,
    loaded: Option<std::time::Instant>,
    selected: Option<i64>,
    detail: Option<Hunt>,
    want: Option<i64>,
    export_open: bool,
    query: String,
    kind: usize,
    map: usize,
    status: usize,
    show_small: bool,
    read_once: bool,
}

impl State {
    pub fn select(&mut self, id: i64) {
        self.want = Some(id);
    }
}

pub const PLAYER_COLORS: [Color32; 4] = crate::overlay::SLOT_COLORS;

pub fn ui(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let tab = w.records.tab;
    widgets::page_header(ui, pal, &i18n::t("nav.records"), |ui| {
        if tab == 1 {
            if button(ui, pal, &i18n::t("action.import"), Btn::Outline, false).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file()
            {
                match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| crate::bestiary::import_json(&t)) {
                    Ok(n) => log::info!("导入图鉴 {n} 条"),
                    Err(e) => log::warn!("导入图鉴失败：{e}"),
                }
            }
        }
        let r = button(ui, pal, &i18n::t("action.export"), Btn::Outline, false);
        if r.clicked() {
            w.records.export_open = !w.records.export_open;
        }
        if w.records.export_open {
            export_menu(w, ui, pal, r.rect);
        }
    });
    let tabs = [(i18n::t("records.hunts"), true), (i18n::t("records.bestiary"), true)];
    if let Some(i) = widgets::tabs(ui, pal, &tabs, tab, true) {
        w.records.tab = i;
    }
    ui.add_space(22.0);
    if w.records.tab == 0 {
        hunts(w, ui, pal);
    } else {
        bestiary(w, ui, pal);
    }
}

fn export_menu(w: &mut MainWindow, ui: &mut Ui, pal: &Pal, anchor: Rect) {
    let mut close = false;
    let tab = w.records.tab;
    egui::Area::new(egui::Id::unique("export-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(anchor.max.x - 220.0, anchor.max.y + 6.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(pal.paper)
                .stroke(Stroke::new(1.0, pal.line))
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 34,
                    spread: 0,
                    color: Color32::from_black_alpha(50),
                })
                .inner_margin(egui::Margin::symmetric(0, 6))
                .show(ui, |ui| {
                    ui.set_width(220.0);
                    let item = |ui: &mut Ui, text: &str, sub: &str| -> bool {
                        let (r, resp) = ui.allocate_exact_size(vec2(220.0, 32.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::ZERO, pal.line);
                        }
                        text_at(ui, pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, text, Font::Text, 13.0, pal.ink);
                        if !sub.is_empty() {
                            text_at(ui, pos2(r.max.x - 16.0, r.center().y), Align2::RIGHT_CENTER, sub, Font::Text, 11.0, pal.mute);
                        }
                        resp.clicked()
                    };
                    if tab == 0 {
                        let all = crate::records::open().and_then(|c| crate::records::list(&c, None, 100_000)).unwrap_or_default();
                        if item(ui, "CSV", "") {
                            save_text("rustyhunt-hunts.csv", &crate::records::to_csv(&all));
                            close = true;
                        }
                        if item(ui, "JSON", "") {
                            let full: Vec<Hunt> = crate::records::open()
                                .map(|c| all.iter().filter_map(|h| crate::records::detail(&c, h.id).ok().flatten()).collect())
                                .unwrap_or_default();
                            save_text("rustyhunt-hunts.json", &crate::records::to_json(&full));
                            close = true;
                        }
                        let exporters = crate::plugins::exporters();
                        if !exporters.is_empty() {
                            widgets::hline(ui, pal);
                        }
                        for e in exporters {
                            if item(ui, &e.name, &e.plugin_name) {
                                crate::plugins::run_exporter(&e, &all);
                                close = true;
                            }
                        }
                    } else if item(ui, "JSON", "") {
                        save_text("rustyhunt-bestiary.json", &crate::bestiary::export_json());
                        close = true;
                    }
                });
        });
    if close || (ui.ctx().input(|i| i.pointer.any_click()) && !ui.ctx().pointer_interact_pos().is_some_and(|p| anchor.expand(240.0).contains(p))) {
        w.records.export_open = false;
    }
}

fn save_text(name: &str, text: &str) {
    if let Some(p) = rfd::FileDialog::new().set_file_name(name).save_file() {
        match std::fs::write(&p, text) {
            Ok(()) => log::info!("已导出 {}", p.display()),
            Err(e) => log::warn!("导出失败：{e}"),
        }
    }
}

fn hunts(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let st = &mut w.records;
    if st.loaded.is_none_or(|t| t.elapsed().as_secs() > 4) {
        st.loaded = Some(std::time::Instant::now());
        let game = [None, Some("world"), Some("rise"), Some("wilds")][st.filter];
        st.hunts = crate::records::open().and_then(|c| crate::records::list(&c, game, 500)).unwrap_or_default();
    }
    if let Some(id) = st.want.take() {
        st.selected = Some(id);
        st.detail = None;
    }
    if st.selected.is_none() {
        st.selected = st.hunts.first().map(|h| h.id);
    }
    if st.detail.as_ref().map(|d| d.id) != st.selected
        && let Some(id) = st.selected
    {
        st.detail = crate::records::open().ok().and_then(|c| crate::records::detail(&c, id).ok().flatten());
    }
    let i = i18n::current();
    let width = ui.available_width();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 48.0;
        ui.vertical(|ui| {
            ui.set_width(280.0);
            let tabs: Vec<(String, bool)> = [i18n::t("label.all"), i18n::t("game_short.world"), i18n::t("game_short.rise"), i18n::t("game_short.wilds")]
                .into_iter()
                .map(|t| (t, true))
                .collect();
            if let Some(f) = widgets::tabs(ui, pal, &tabs, st.filter, false) {
                st.filter = f;
                st.loaded = None;
            }
            ui.add_space(12.0);
            if st.hunts.is_empty() {
                label_mute(ui, pal, &i18n::t("records.no_hunts"), 13.0);
            }
            for h in &st.hunts {
                let (r, resp) = ui.allocate_exact_size(vec2(280.0, 62.0), Sense::click());
                let on = Some(h.id) == st.selected;
                if on {
                    ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x, r.min.y + 13.0), vec2(2.0, 36.0)), CornerRadius::ZERO, pal.ink);
                }
                text_at(ui, pos2(r.min.x + 14.0, r.min.y + 13.0), Align2::LEFT_TOP, &i.monster(&h.monster), if on { Font::TextBold } else { Font::Text }, 15.0, pal.ink);
                text_at(ui, pos2(r.max.x, r.min.y + 14.0), Align2::RIGHT_TOP, &mmss(h.duration), Font::NumBold, 14.0, pal.ink);
                let sub = format!("{}　{}", i18n::t(&format!("game_short.{}", h.game)), super::fmt_when(h.started_at));
                text_at(ui, pos2(r.min.x + 14.0, r.min.y + 38.0), Align2::LEFT_TOP, &sub, Font::Text, 12.0, pal.mute);
                ui.painter().line_segment([pos2(r.min.x, r.max.y - 0.5), pos2(r.max.x, r.max.y - 0.5)], Stroke::new(1.0, pal.line));
                if resp.clicked() {
                    st.selected = Some(h.id);
                }
            }
        });
        ui.vertical(|ui| {
            ui.set_width(width - 280.0 - 48.0);
            if let Some(h) = st.detail.clone() {
                detail(ui, pal, &h);
            }
        });
    });
}

fn detail(ui: &mut Ui, pal: &Pal, h: &Hunt) {
    let i = i18n::current();
    // 右边是结果和用时，左边放名牌和副标题；名字很长时副标题换到下一行
    let dur = widgets::galley(ui, &mmss(h.duration), Font::NumHeavy, 44.0, pal.ink);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), (dur.size().y + 18.0).max(84.0)), Sense::hover());
    let capture = h.result == "capture";
    text_at(ui, r.right_top(), Align2::RIGHT_TOP, &i18n::t(&format!("result.{}", h.result)), Font::Text, 12.0, if capture { pal.accent } else { pal.mute });
    let dw = dur.size().x;
    ui.painter().galley(pos2(r.max.x - dw, r.min.y + 16.0), dur, pal.ink);
    let left = Rect::from_min_max(r.min, pos2(r.max.x - dw - 32.0, r.max.y));
    let name = i.monster(&h.monster);
    let sub = format!("{} {}　{}", i18n::t(&format!("game_short.{}", h.game)), i18n::t(&format!("rank.{}", h.rank)), super::fmt_when(h.started_at));
    let need = widgets::galley(ui, &name, Font::TextBold, 24.0, pal.ink).size().x + 24.0 + 18.0 + widgets::galley(ui, &sub, Font::Text, 13.0, pal.mute).size().x;
    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        let row = |ui: &mut Ui| {
            widgets::tag(ui, pal, &name, 24.0);
            label_mute(ui, pal, &sub, 13.0);
        };
        if need <= left.width() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                row(ui);
            });
        } else {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                row(ui);
            });
        }
    });
    ui.add_space(26.0);

    // DPS 曲线：每人一条
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 160.0), Sense::hover());
    let max = h.curve.iter().map(|c| c.2).fold(1.0, f32::max) * 1.12;
    let p = ui.painter();
    for k in 0..4 {
        let y = r.min.y + 10.0 + k as f32 * 44.0;
        p.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, pal.line));
        let v = max * (1.0 - (10.0 + k as f32 * 44.0 - 10.0) / (r.height() - 20.0));
        p.text(pos2(r.min.x, y - 4.0), Align2::LEFT_BOTTOM, format!("{}", v.round()), Font::NumBold.id(9.0), pal.mute);
    }
    let tmax = h.duration.max(1.0);
    for (k, player) in h.players.iter().enumerate() {
        let pts: Vec<egui::Pos2> = h
            .curve
            .iter()
            .filter(|c| c.1 == player.slot)
            .map(|c| pos2(r.min.x + c.0 / tmax * r.width(), r.max.y - 8.0 - c.2 / max * (r.height() - 20.0)))
            .collect();
        if pts.len() > 1 {
            let c = PLAYER_COLORS[(player.slot.clamp(0, 3)) as usize];
            p.add(egui::Shape::line(pts, Stroke::new(if player.is_self { 2.4 } else { 1.4 }, c)));
        }
        let _ = k;
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        for pl in &h.players {
            let (sr, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
            ui.painter().rect_filled(Rect::from_center_size(sr.center(), vec2(12.0, 2.0)), CornerRadius::ZERO, PLAYER_COLORS[pl.slot.clamp(0, 3) as usize]);
            label_mute(ui, pal, &pl.name, 12.0);
        }
    });
    ui.add_space(30.0);

    // 表格
    let total: f32 = h.players.iter().map(|p| p.damage).sum::<f32>().max(1.0);
    let w = ui.available_width();
    let cols = [w - 4.0 * 90.0, 90.0, 90.0, 90.0, 90.0];
    let header = [i18n::t("records.hunter"), i18n::t("label.dps"), i18n::t("label.total_damage"), i18n::t("label.share"), i18n::t("label.critical")];
    let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
    let mut x = hr.min.x;
    for (i, (c, t)) in cols.iter().zip(&header).enumerate() {
        let (pos, align) = if i == 0 { (pos2(x, hr.min.y + 4.0), Align2::LEFT_TOP) } else { (pos2(x + c, hr.min.y + 4.0), Align2::RIGHT_TOP) };
        text_at(ui, pos, align, t, Font::Text, 12.0, pal.mute);
        x += c;
    }
    ui.painter().line_segment([pos2(hr.min.x, hr.max.y), pos2(hr.max.x, hr.max.y)], Stroke::new(1.0, pal.ink));
    for pl in &h.players {
        let (rr, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
        let mut x = rr.min.x;
        let vals = [
            pl.name.clone(),
            format!("{}", (pl.damage / h.duration.max(1.0)).round()),
            crate::overlay::thousands(pl.damage),
            format!("{}%", (pl.damage / total * 100.0).round()),
            pl.crit.map(|c| format!("{}%", (c * 100.0).round())).unwrap_or_else(|| "—".into()),
        ];
        for (i, (c, v)) in cols.iter().zip(&vals).enumerate() {
            if i == 0 {
                ui.painter().rect_filled(Rect::from_min_size(pos2(x, rr.center().y - 3.0), vec2(6.0, 6.0)), CornerRadius::ZERO, PLAYER_COLORS[pl.slot.clamp(0, 3) as usize]);
                text_at(ui, pos2(x + 16.0, rr.center().y), Align2::LEFT_CENTER, v, if pl.is_self { Font::TextBold } else { Font::Text }, 13.0, pal.ink);
            } else {
                text_at(ui, pos2(x + c, rr.center().y), Align2::RIGHT_CENTER, v, Font::NumBold, 12.0, pal.ink);
            }
            x += c;
        }
        ui.painter().line_segment([pos2(rr.min.x, rr.max.y - 0.5), pos2(rr.max.x, rr.max.y - 0.5)], Stroke::new(1.0, pal.line));
    }
    for (k, v) in &h.fields {
        let (rr, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        text_at(ui, pos2(rr.min.x, rr.center().y), Align2::LEFT_CENTER, k, Font::Text, 12.0, pal.mute);
        text_at(ui, pos2(rr.max.x, rr.center().y), Align2::RIGHT_CENTER, v, Font::Mono, 12.0, pal.ink);
    }

    // 时间轴
    ui.add_space(34.0);
    let (tr, _) = ui.allocate_exact_size(vec2(w, 60.0), Sense::hover());
    let ay = tr.min.y + 24.0;
    ui.painter().line_segment([pos2(tr.min.x, ay), pos2(tr.max.x, ay)], Stroke::new(1.0, pal.ink));
    let mut last_right = f32::NEG_INFINITY;
    for e in &h.events {
        let x = tr.min.x + (e.t / tmax).clamp(0.0, 1.0) * tr.width();
        let label = event_label(e);
        let c = if e.accent { pal.accent } else { pal.ink };
        widgets::diamond(ui, pos2(x, ay), 7.0, c);
        // 字放不下就只画菱形
        let half = widgets::galley(ui, &label, Font::Text, 11.0, pal.ink).size().x.max(30.0) / 2.0;
        let cx = x.clamp(tr.min.x + half, tr.max.x - half);
        if cx - half > last_right + 10.0 {
            text_at(ui, pos2(cx, tr.min.y + 8.0), Align2::CENTER_CENTER, &label, Font::Text, 11.0, pal.ink);
            text_at(ui, pos2(cx, ay + 16.0), Align2::CENTER_CENTER, &mmss(e.t), Font::NumBold, 10.0, pal.mute);
            last_right = cx + half;
        }
    }
}

fn event_label(e: &crate::records::EventRow) -> String {
    let i = i18n::current();
    match e.kind.as_str() {
        "break" | "sever" | "ailment" => i.game(&e.label),
        "capture" => i18n::t("result.capture"),
        "slay" => i18n::t("result.slay"),
        "down" => i18n::t("records.event_down"),
        "enrage" => i18n::t("records.event_enrage"),
        "tired" => i18n::t("records.event_tired"),
        other => other.to_string(),
    }
}

fn bestiary(w: &mut MainWindow, ui: &mut Ui, pal: &Pal) {
    let shared = w.shared.clone();
    if !w.records.read_once {
        w.records.read_once = true;
        if crate::bestiary::with(|b| b.save.is_none() && !b.reading) {
            crate::bestiary::read(shared.clone());
        }
    }
    let tabs = [(i18n::t("game_short.world"), true), (i18n::t("game_short.rise"), false), (i18n::t("game_short.wilds"), false)];
    widgets::tabs(ui, pal, &tabs, 0, false);
    ui.add_space(20.0);

    let (path, read_at, error, reading, hunters, hunter) = crate::bestiary::with(|b| {
        (
            b.path.clone(),
            b.read_at,
            b.error.clone(),
            b.reading,
            b.save.as_ref().map(|s| s.hunters.iter().map(|h| h.name.clone()).collect::<Vec<_>>()).unwrap_or_default(),
            b.hunter,
        )
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        label_mute(ui, pal, &i18n::t("label.save_file"), 12.0);
        let fail = error.is_some();
        label(ui, path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "SAVEDATA1000".into()), Font::Mono, 12.0, if fail { pal.accent } else { pal.mute });
        let t = if reading {
            i18n::t("records.reading")
        } else if let Some(e) = &error {
            i18n::tf("records.read_failed", &[("e", e)])
        } else if let Some(t) = read_at {
            i18n::tf("records.read_at", &[("t", &chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default())])
        } else {
            String::new()
        };
        label(ui, t, Font::Text, 12.0, if fail { pal.accent } else { pal.mute });
        if !reading && link(ui, pal, &i18n::t("action.reread")).clicked() {
            crate::bestiary::read(shared.clone());
        }
        if hunters.len() > 1 {
            let mut sel = hunter;
            if widgets::select(ui, pal, "hunter", &hunters, &mut sel, 120.0) {
                crate::bestiary::with(|b| b.hunter = sel);
            }
        }
    });
    if error.is_some() {
        label_mute(ui, pal, &i18n::t("label.manual"), 12.0);
    }
    ui.add_space(24.0);

    let rows = crate::bestiary::with(|b| crate::bestiary::rows(b));
    let i = i18n::current();
    // 进度
    let crownable: Vec<&crate::bestiary::Row> = rows.iter().filter(|r| !r.fixed).collect();
    let progress = [
        (i18n::t("records.big_crowns"), crownable.iter().filter(|r| r.big).count(), crownable.len()),
        (i18n::t("records.small_crowns"), crownable.iter().filter(|r| r.small).count(), crownable.len()),
        (i18n::t("records.big_research"), rows.iter().filter(|r| r.research >= r.research_cap).count(), rows.len()),
        (i18n::t("records.completed"), rows.iter().filter(|r| r.done()).count(), rows.len()),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 46.0;
        for (l, a, b) in &progress {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                label_mute(ui, pal, l, 12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    label(ui, a.to_string(), Font::Num, 26.0, pal.ink);
                    label(ui, format!("/{b}"), Font::NumBold, 13.0, pal.mute);
                });
                widgets::thin_bar(ui, pal.ink, pal.line, *a as f32 / (*b).max(1) as f32, vec2(92.0, 2.0));
            });
        }
    });
    ui.add_space(30.0);

    // 筛选
    let mut kinds: Vec<String> = rows.iter().map(|r| r.species.clone()).collect();
    kinds.sort();
    kinds.dedup();
    let mut kind_names = vec![i18n::t("records.all_kinds")];
    kind_names.extend(kinds.iter().map(|k| i.game(k)));
    let map_list = ["ancient_forest", "wildspire_waste", "coral_highlands", "rotten_vale", "elders_recess", "hoarfrost_reach", "guiding_lands"];
    let mut map_names = vec![i18n::t("records.all_maps")];
    map_names.extend(map_list.iter().map(|m| i.map(m)));
    let statuses = ["status_all", "status_open", "status_no_big", "status_no_small", "status_research", "status_done"].map(|s| i18n::t(&format!("records.{s}")));
    let st = &mut w.records;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        widgets::text_input(ui, pal, &mut st.query, &i18n::t("records.filter_query"), 190.0);
        widgets::select(ui, pal, "kind", &kind_names, &mut st.kind, 110.0);
        widgets::select(ui, pal, "map", &map_names, &mut st.map, 110.0);
        widgets::select(ui, pal, "status", &statuses, &mut st.status, 100.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch(ui, pal, &mut st.show_small);
            label_mute(ui, pal, &i18n::t("records.small_monsters"), 13.0);
        });
    });
    ui.add_space(14.0);

    let width = ui.available_width();
    let cols = [150.0, 34.0, 34.0, 168.0, 62.0, 0.0];
    let rest = width - cols.iter().sum::<f32>() - 14.0 * 5.0;
    let header = [i18n::t("records.monster"), i18n::t("records.big"), i18n::t("records.small"), i18n::t("records.research"), i18n::t("records.nav"), i18n::t("records.maps")];
    let (hr, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    let mut x = hr.min.x;
    for (c, t) in cols.iter().zip(&header) {
        text_at(ui, pos2(x, hr.min.y + 4.0), Align2::LEFT_TOP, t, Font::Text, 12.0, pal.mute);
        x += if *c > 0.0 { *c } else { rest } + 14.0;
    }
    ui.painter().line_segment([pos2(hr.min.x, hr.max.y), pos2(hr.max.x, hr.max.y)], Stroke::new(1.0, pal.ink));

    let q = st.query.trim().to_lowercase();
    let mut changes: Vec<(String, Box<dyn FnOnce(&mut crate::bestiary::Manual)>)> = Vec::new();
    for r in &rows {
        let name = i.monster(&r.key);
        let ok = (q.is_empty() || name.to_lowercase().contains(&q) || r.key.contains(&q))
            && (st.kind == 0 || kinds.get(st.kind - 1) == Some(&r.species))
            && (st.map == 0 || r.maps.contains(&map_list[st.map - 1]))
            && match st.status {
                1 => !r.done(),
                2 => !r.fixed && !r.big,
                3 => !r.fixed && !r.small,
                4 => r.research < r.research_cap,
                5 => r.done(),
                _ => true,
            };
        if !ok {
            continue;
        }
        let (rr, _) = ui.allocate_exact_size(vec2(width, 46.0), Sense::hover());
        let mut x = rr.min.x;
        let cy = rr.center().y;
        // 名字和种类
        let mut nx = x;
        if r.done() {
            widgets::diamond(ui, pos2(nx + 2.5, cy - 7.0), 5.0, pal.accent);
            nx += 12.0;
        }
        text_at(ui, pos2(nx, cy - 7.0), Align2::LEFT_CENTER, &name, Font::TextBold, 14.0, pal.ink);
        text_at(ui, pos2(x, cy + 9.0), Align2::LEFT_CENTER, &i.game(&r.species), Font::Text, 11.0, pal.mute);
        x += cols[0] + 14.0;
        // 大金、小金
        for (j, (on, big)) in [(r.big, true), (r.small, false)].into_iter().enumerate() {
            let cr = Rect::from_center_size(pos2(x + 13.0, cy), vec2(26.0, 22.0));
            let resp = ui.interact(cr, egui::Id::unique(("crown", &r.key, j)), if r.fixed { Sense::hover() } else { Sense::click() });
            crown_icon(ui, cr.center(), if big { 22.0 } else { 15.0 }, on, r.fixed, pal);
            if resp.clicked() {
                let v = !on;
                changes.push((r.key.clone(), Box::new(move |m: &mut crate::bestiary::Manual| if big { m.big = Some(v) } else { m.small = Some(v) })));
            }
            x += cols[1 + j] + 14.0;
        }
        // 研究格
        for k in 0..9u8 {
            let sr = Rect::from_min_size(pos2(x + k as f32 * 17.0, cy - 5.0), vec2(14.0, 10.0));
            let resp = ui.interact(sr.expand(1.5), egui::Id::unique(("res", &r.key, k)), Sense::click());
            if k >= r.research_cap {
                dashed(ui, sr, pal.faint);
            } else {
                ui.painter().rect(sr, CornerRadius::ZERO, if k < r.research { pal.ink } else { Color32::TRANSPARENT }, Stroke::new(1.0, pal.ink), StrokeKind::Inside);
                if resp.clicked() {
                    let v = if r.research == k + 1 { k } else { k + 1 };
                    changes.push((r.key.clone(), Box::new(move |m: &mut crate::bestiary::Manual| m.research = Some(v))));
                }
            }
        }
        x += cols[3] + 14.0;
        // 导航等级（手动）
        for k in 0..3u8 {
            let sr = Rect::from_min_size(pos2(x + k as f32 * 17.0, cy - 5.0), vec2(14.0, 10.0));
            let resp = ui.interact(sr.expand(1.5), egui::Id::unique(("nav", &r.key, k)), Sense::click());
            ui.painter().rect(sr, CornerRadius::ZERO, if k < r.nav { pal.ink } else { Color32::TRANSPARENT }, Stroke::new(1.0, pal.ink), StrokeKind::Inside);
            if resp.clicked() {
                let v = if r.nav == k + 1 { k } else { k + 1 };
                changes.push((r.key.clone(), Box::new(move |m: &mut crate::bestiary::Manual| m.nav = Some(v))));
            }
        }
        x += cols[4] + 14.0;
        let maps: Vec<String> = r.maps.iter().map(|m| i.map(m)).collect();
        let maps = if maps.is_empty() { "—".to_string() } else { maps.join(" ") };
        let g = ui.painter().layout(maps, Font::Text.id(12.0), pal.mute, rest);
        ui.painter().galley(pos2(x, cy - g.size().y / 2.0), g, pal.mute);
        ui.painter().line_segment([pos2(rr.min.x, rr.max.y - 0.5), pos2(rr.max.x, rr.max.y - 0.5)], Stroke::new(1.0, pal.line));
    }
    for (k, f) in changes {
        crate::bestiary::set(&k, f);
    }

    if st.show_small {
        ui.add_space(28.0);
        widgets::section(ui, pal, &i18n::t("records.small_monsters"), None);
        let smalls: Vec<String> = i.terms().keys("monster").into_iter().filter(|k| i.terms().is_large(k) == Some(false)).collect();
        for key in smalls {
            let (rr, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
            text_at(ui, pos2(rr.min.x, rr.center().y), Align2::LEFT_CENTER, &i.monster(&key), Font::TextBold, 14.0, pal.ink);
            let research = crate::bestiary::with(|b| b.store.world.get(&key).and_then(|m| m.research).unwrap_or(0));
            let x = rr.min.x + cols[0] + cols[1] + cols[2] + 14.0 * 3.0;
            for k in 0..9u8 {
                let sr = Rect::from_min_size(pos2(x + k as f32 * 17.0, rr.center().y - 5.0), vec2(14.0, 10.0));
                let resp = ui.interact(sr.expand(1.5), egui::Id::unique(("sres", &key, k)), Sense::click());
                ui.painter().rect(sr, CornerRadius::ZERO, if k < research { pal.ink } else { Color32::TRANSPARENT }, Stroke::new(1.0, pal.ink), StrokeKind::Inside);
                if resp.clicked() {
                    let v = if research == k + 1 { k } else { k + 1 };
                    crate::bestiary::set(&key, |m| m.research = Some(v));
                }
            }
            ui.painter().line_segment([pos2(rr.min.x, rr.max.y - 0.5), pos2(rr.max.x, rr.max.y - 0.5)], Stroke::new(1.0, pal.line));
        }
    }
}

fn dashed(ui: &Ui, r: Rect, c: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        ui.painter().extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, c), 2.0, 2.0));
    }
}

fn crown_icon(ui: &Ui, c: egui::Pos2, w: f32, on: bool, fixed: bool, pal: &Pal) {
    let s = w / 16.0;
    let pts: Vec<egui::Pos2> = [(1.0, 10.0), (2.0, 3.0), (5.5, 6.5), (8.0, 1.0), (10.5, 6.5), (14.0, 3.0), (15.0, 10.0)]
        .iter()
        .map(|(x, y)| pos2(c.x + (x - 8.0) * s, c.y + (y - 6.0) * s))
        .collect();
    if fixed {
        let mut closed = pts.clone();
        closed.push(pts[0]);
        ui.painter().extend(egui::Shape::dashed_line(&closed, Stroke::new(1.0, pal.line), 2.0, 2.0));
        return;
    }
    ui.painter().add(egui::Shape::Path(egui::epaint::PathShape {
        points: pts,
        closed: true,
        fill: if on { Color32::from_rgb(0xe0, 0xb2, 0x4a) } else { Color32::TRANSPARENT },
        stroke: egui::epaint::PathStroke::new(1.2, if on { pal.ink } else { pal.faint }),
    }));
}
