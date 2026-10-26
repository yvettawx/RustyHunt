//! 其余模块：伤害统计、本区域怪物、时钟、据点、性能。

use egui::{Align2, Color32, Vec2, vec2};
use rh_core::model::{Crown, Target};
use rh_core::settings::{Density, ModuleConfig};
use ui_kit::color::fade;
use ui_kit::{Canvas, Font, Layer};

use super::hunter::module_header;
use super::{Frame, State, mmss, thousands};

/// 队员的颜色，按栏位
pub const SLOT_COLORS: [Color32; 4] = [
    Color32::from_rgb(0xe5, 0x48, 0x4d),
    Color32::from_rgb(0x3e, 0x82, 0xf7),
    Color32::from_rgb(0xf5, 0xc5, 0x42),
    Color32::from_rgb(0x4f, 0xae, 0x5b),
];

pub fn damage(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let s = f.snap;
    if s.party.is_empty() || s.party.iter().all(|p| p.damage <= 0.0) && !f.preview {
        return Vec2::ZERO;
    }
    let w = 270.0;
    let text = cv.tok("text");
    let accent = cv.tok("accent");
    let elapsed = s.elapsed.max(1.0);
    let total: f32 = s.party.iter().map(|p| p.damage).sum();
    let top = s.party.iter().map(|p| p.damage).fold(1.0, f32::max);
    let team = if cfg.shows("d-team") {
        format!("{} DPS", thousands(total / elapsed))
    } else {
        String::new()
    };
    let mut y = module_header(cv, &f.t("module.damage"), w, &team, None);
    let mut members: Vec<_> = s.party.iter().collect();
    members.sort_by(|a, b| b.damage.total_cmp(&a.damage));
    let only_self = !cfg.shows("d-others") || f.density == Density::Minimal;
    for (rank, m) in members.iter().enumerate() {
        if only_self && !m.is_self {
            continue;
        }
        let c = if m.is_self { cv.clone() } else { cv.faded(0.78) };
        if cfg.shows("d-rank") {
            c.text(0.0, y, &format!("{:02}", rank + 1), Font::NumHeavy, 13.0, if m.is_self { accent } else { cv.tok("text-mute") }, Align2::LEFT_TOP);
        }
        let nx = if cfg.shows("d-rank") { 30.0 } else { 0.0 };
        let color = SLOT_COLORS[(m.slot.clamp(0, 3)) as usize];
        c.rect(nx, y + 6.0, 5.0, 5.0, color);
        let name = if m.is_self {
            f.t("label.you")
        } else if cfg.shows("d-blur") {
            f.i18n.tf("label.hunter_n", &[("n", &(m.slot + 1).to_string())])
        } else {
            m.name.clone()
        };
        c.text(nx + 11.0, y, &name, Font::Text, 12.0, if m.is_self { accent } else { text }, Align2::LEFT_TOP);
        let v = thousands(m.damage / elapsed);
        let vs = c.measure(&v, Font::Num, 13.0, 0.0);
        c.text(w - vs.x, y - 1.0, &v, Font::Num, 13.0, text, Align2::LEFT_TOP);
        y += 17.0;
        if cfg.shows("d-bar") {
            c.bar("damage", nx, y, w - nx, 2.0, &[Layer::fill(m.damage / top, if m.is_self { accent } else { cv.tok("text-mute") })], false);
            y += 2.0;
        }
        if m.is_self && cfg.shows("d-split")
            && let Some(split) = &s.damage_split
        {
            y += 4.0;
            let mut x = nx;
            let mute = cv.tok("text-mute");
            let mut item = |label: &str, value: String| {
                let ls = c.text(x, y, label, Font::Text, 10.0, mute, Align2::LEFT_TOP);
                let vs = c.text(x + ls.x + 3.0, y, &value, Font::NumBold, 10.0, text, Align2::LEFT_TOP);
                x += ls.x + 3.0 + vs.x + 10.0;
            };
            if let Some(phys) = split.physical {
                item(&f.t("label.physical"), thousands(phys / elapsed));
            }
            if let Some(el) = split.elemental {
                item(&f.t("label.elemental"), thousands(el / elapsed));
            }
            item(&f.t("label.critical"), format!("{}%", (split.crit_rate() * 100.0).round()));
            y += 14.0;
        }
        y += 6.0;
    }
    if cfg.shows("d-curve") && f.live.dps_history.len() > 1 {
        let h = &f.live.dps_history;
        let mx = h.iter().copied().fold(1.0, f32::max) * 1.15;
        let n = h.len() as f32 - 1.0;
        let pts: Vec<(f32, f32)> = h
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f32 / n * w, y + 25.0 - v / mx * 22.0))
            .collect();
        cv.polyline(&pts, 1.5, accent);
        y += 28.0;
    }
    cv.crosshair(-18.0, -2.0);
    vec2(w, y)
}

pub fn area(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    if f.density == Density::Minimal && !f.preview {
        return Vec2::ZERO;
    }
    let list: Vec<_> = f.snap.large_monsters().collect();
    if list.is_empty() {
        return Vec2::ZERO;
    }
    let w = 230.0;
    let text = cv.tok("text");
    let accent = cv.tok("accent");
    let mut y = module_header(cv, &f.t("module.area"), w, &list.len().to_string(), None);
    let target = f.target.map(|m| m.uid);
    for m in list {
        let is_t = Some(m.uid) == target || m.target == Target::Own;
        let c = if is_t { cv.clone() } else { cv.faded(0.72) };
        c.diamond(4.0, y + 9.0, 6.0, if is_t { accent } else { Color32::TRANSPARENT }, if is_t { accent } else { text });
        let name = f.i18n.monster(&m.key);
        let ns = c.text(17.0, y + 2.0, &name, Font::Text, 12.0, text, Align2::LEFT_TOP);
        let mut x = 17.0 + ns.x + 5.0;
        if matches!(m.crown, Crown::Gold | Crown::Silver | Crown::Mini) {
            c.crown(x, y + 5.0, 12.0, m.crown != Crown::Silver);
            x += 17.0;
        }
        if m.tired {
            let ts = c.text(x, y + 3.0, &f.t("state.tired"), Font::Text, 10.0, cv.tok("cool"), Align2::LEFT_TOP);
            x += ts.x + 5.0;
        } else if m.enraged {
            let ts = c.text(x, y + 3.0, &f.t("state.enraged"), Font::Text, 10.0, accent, Align2::LEFT_TOP);
            x += ts.x + 5.0;
        }
        let bx = x.max(110.0) + 4.0;
        if cfg.shows("a-bar") {
            c.bar("area", bx, y + 9.0, w - bx - 43.0, 2.0, &[Layer::fill(m.hp_ratio(), text)], false);
        }
        if cfg.shows("a-pct") {
            let v = format!("{}%", (m.hp_ratio() * 100.0).round());
            let vs = c.measure(&v, Font::NumBold, 11.0, 0.0);
            c.text(w - vs.x, y + 3.0, &v, Font::NumBold, 11.0, text, Align2::LEFT_TOP);
        }
        y += 21.0;
    }
    cv.crosshair(-18.0, -2.0);
    vec2(w, y)
}

pub fn clock(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let mute = cv.tok("text-mute");
    let mut x = 0.0;
    let mut h = 0.0f32;
    if cfg.shows("c-quest")
        && let Some(q) = f.snap.quest.as_ref().filter(|_| f.snap.in_quest())
    {
        let ls = cv.text_ex(0.0, 9.0, &f.t("label.time_left"), Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
        let warn = q.time_left < 300.0;
        let ts = cv.text(ls.x + 10.0, 0.0, &mmss(q.time_left), Font::NumHeavy, 24.0, if warn { cv.tok("accent") } else { cv.tok("text") }, Align2::LEFT_TOP);
        x = ls.x + 10.0 + ts.x;
        h = h.max(ts.y);
    }
    if cfg.shows("c-game")
        && let Some((hh, mm)) = f.snap.world_time
    {
        let s = f.i18n.tf("label.game_time", &[("time", &format!("{hh:02}:{mm:02}"))]);
        let gs = cv.text(x + if x > 0.0 { 10.0 } else { 0.0 }, 10.0, &s, Font::NumBold, 12.0, mute, Align2::LEFT_TOP);
        x += 10.0 + gs.x;
        h = h.max(24.0);
    }
    if x <= 0.0 {
        return Vec2::ZERO;
    }
    cv.crosshair(-18.0, 4.0);
    vec2(x, h)
}

pub fn base(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let Some(b) = &f.snap.base else { return Vec2::ZERO };
    let w = 220.0;
    let text = cv.tok("text");
    let accent = cv.tok("accent");
    let mut y = module_header(cv, &f.t("module.base"), w, "", None);
    let rows: Vec<(&str, String, String, bool)> = vec![
        (
            "h-argosy",
            f.i18n.facility("argosy"),
            if b.argosy_in_town {
                f.i18n.tf("unit.days", &[("n", &b.argosy_days.to_string())])
            } else {
                f.t("state.away")
            },
            false,
        ),
        (
            "h-tail",
            f.i18n.facility("tailraider_safari"),
            if b.tailraiders_deployed {
                f.i18n.tf("unit.times", &[("n", &b.tailraiders_quests.to_string())])
            } else {
                f.t("state.idle")
            },
            !b.tailraiders_deployed,
        ),
        (
            "h-farm",
            f.i18n.facility("botanical_research"),
            if b.harvest_items >= b.harvest_capacity {
                f.t("state.full")
            } else {
                format!("{}/{}", b.harvest_items, b.harvest_capacity)
            },
            b.harvest_items >= b.harvest_capacity,
        ),
        ("h-steam", f.i18n.facility("steamworks"), thousands(b.steam_stored as f32), false),
    ];
    for (key, name, value, full) in rows {
        if !cfg.shows(key) {
            continue;
        }
        cv.text(0.0, y + 3.0, &name, Font::Text, 12.0, text, Align2::LEFT_TOP);
        let vs = cv.measure(&value, Font::Num, 12.0, 0.0);
        cv.text(w - vs.x, y + 3.0, &value, Font::Num, 12.0, if full { accent } else { text }, Align2::LEFT_TOP);
        y += 21.0;
    }
    cv.crosshair(-18.0, -2.0);
    vec2(w, y)
}

pub fn perf(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let mute = cv.tok("text-mute");
    let text = cv.tok("text");
    let mut x = 0.0;
    let mut item = |label: &str, value: String| {
        let ls = cv.text(x, 0.0, label, Font::TextBold, 10.0, mute, Align2::LEFT_TOP);
        let vs = cv.text(x + ls.x + 4.0, 0.0, &value, Font::NumBold, 10.0, text, Align2::LEFT_TOP);
        x += ls.x + 4.0 + vs.x + 12.0;
    };
    if cfg.shows("f-frame") {
        item(&f.t("label.frame"), format!("{:.2} ms", f.frame_us as f32 / 1000.0));
    }
    if cfg.shows("f-read") {
        item(&f.t("label.read"), format!("{} Hz", f.settings.perf.read_hz));
    }
    let _ = fade;
    vec2(x, 14.0)
}
