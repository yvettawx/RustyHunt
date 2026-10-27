//! The hunter's own modules: player (HP, stamina, sharpness, status effects), buffs, weapon gauge.

use egui::{Align2, Color32, Vec2, vec2};
use rh_core::model::{AbnormalityKind, SharpnessLevel, WeaponState};
use rh_core::modules;
use rh_core::settings::ModuleConfig;
use ui_kit::color::{fade, rgba};
use ui_kit::{Canvas, Font, Layer};

use super::{Frame, State, buff_time, mmss};

const SHARP_COLORS: [Color32; 7] = [
    Color32::from_rgb(0xd0, 0x3a, 0x32),
    Color32::from_rgb(0xe8, 0x89, 0x2f),
    Color32::from_rgb(0xe6, 0xcf, 0x38),
    Color32::from_rgb(0x5f, 0xb5, 0x48),
    Color32::from_rgb(0x3f, 0x7f, 0xd9),
    Color32::from_rgb(0xf2, 0xf0, 0xe6),
    Color32::from_rgb(0xa0, 0x66, 0xd3),
];

/// Status effects that tint the health bar, most serious first, with their colour token.
const STATUS_TINTS: [(&str, &str); 10] = [
    ("ABNORMALITY_VENOM", "status-venom"),
    ("ABNORMALITY_POISON", "status-poison"),
    ("ABNORMALITY_BLEED", "status-bleed"),
    ("ABNORMALITY_DRAGON", "status-dragon"),
    ("ABNORMALITY_FIRE", "status-fire"),
    ("ABNORMALITY_BLAST", "status-blast"),
    ("ABNORMALITY_EFFLUVIA", "status-effluvia"),
    ("ABNORMALITY_THUNDER", "status-thunder"),
    ("ABNORMALITY_ICE", "status-ice"),
    ("ABNORMALITY_WATER", "status-water"),
];

/// Effects the game shows as a gauge rather than an icon; drawn in the player module, not with buffs.
pub const GAUGES: [&str; 1] = ["ABNORMALITY_FROSTCRAFT"];

fn status_color(cv: &Canvas, key: &str) -> Option<Color32> {
    STATUS_TINTS.iter().find(|(k, _)| *k == key).map(|(_, t)| cv.tok(t))
}

/// Module title: one word and a thin line, with an optional value on the right (no backing).
pub fn module_header(cv: &Canvas, label: &str, w: f32, right: &str, rule_w: Option<f32>) -> f32 {
    let mute = cv.tok("text-mute");
    let sz = cv.text_ex(0.0, 0.0, label, Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
    let mut end = w;
    if !right.is_empty() {
        let rs = cv.measure(right, Font::NumBold, 10.0, 0.06);
        cv.text_ex(w - rs.x, 0.0, right, Font::NumBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
        end = w - rs.x - 8.0;
    }
    let x0 = sz.x + 8.0;
    let x1 = rule_w.map_or(end, |rw| x0 + rw);
    cv.rule(x0, x1, sz.y / 2.0, cv.tok("text-faint"));
    sz.y + 7.0
}

pub fn player(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, st: &mut State) -> Vec2 {
    let p = &f.snap.player;
    if p.hp_max <= 0.0 {
        return Vec2::ZERO;
    }
    let (k, _) = modules::player_metrics(f.game);
    let px = |v: f32| v.max(0.0) * k;
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let danger = cv.tok("danger");
    let low = p.hp / p.hp_max < f.profile.player.low_hp / 100.0;
    let wmax = p.hp_max.max(p.stamina_max);

    // The trailing bar catches up with current HP
    if st.player_ghost < p.hp {
        st.player_ghost = p.hp;
    }
    if f.now - st.player_hit_at > 0.7 && st.player_ghost > p.hp {
        st.player_ghost = (st.player_ghost - 40.0 * st.dt).max(p.hp);
        st.animating = true;
    } else if st.player_ghost > p.hp {
        st.animating = true;
    }

    // First row: name tag, healing, red HP, value
    let header_h = 22.0;
    if f.density != rh_core::settings::Density::Minimal && !p.name.is_empty() {
        cv.tag(0.0, header_h - 19.0, &p.name, Font::TextBold, 12.0, (7.0, 3.0, 7.0, 2.0), cv.tok("ink"), text);
    }
    let healing = p.hp_healing_to > p.hp + 0.5;
    if healing {
        let nsz = cv.tag_size(&p.name, Font::TextBold, 12.0, (7.0, 3.0, 7.0, 2.0));
        cv.text(
            nsz.x + 10.0,
            header_h - 16.0,
            &f.t("state.healing"),
            Font::TextBold,
            11.0,
            cv.tok("ok"),
            Align2::LEFT_TOP,
        );
    }
    if cfg.shows("p-num") {
        let right = px(p.hp_max);
        let max_s = format!("/{}", p.hp_max.round());
        let cur_s = format!("{}", p.hp.round());
        let ms = cv.measure(&max_s, Font::NumBold, 12.0, 0.0);
        cv.text(right - ms.x, header_h - 15.0, &max_s, Font::NumBold, 12.0, mute, Align2::LEFT_TOP);
        let cs = cv.measure(&cur_s, Font::Num, 22.0, 0.0);
        cv.text(
            right - ms.x - 2.0 - cs.x,
            header_h - 25.0,
            &cur_s,
            Font::Num,
            22.0,
            if low { danger } else { text },
            Align2::LEFT_TOP,
        );
    }

    // HP bar: trail, red HP, healing, current
    let y = header_h + 5.0;
    let w = px(p.hp_max);
    let r = |v: f32| (v / p.hp_max).clamp(0.0, 1.0);
    let blink = if low {
        st.animating = true;
        0.72 + 0.28 * ((f.now as f32 * std::f32::consts::TAU / 1.1).cos() * 0.5 + 0.5)
    } else {
        1.0
    };
    let mut layers = vec![Layer::fill(r(st.player_ghost), rgba(242, 240, 233, 0.42))];
    if p.hp_recoverable > p.hp {
        layers.push(Layer {
            from: r(p.hp),
            to: r(p.hp_recoverable),
            color: if low { rgba(229, 72, 77, 0.32) } else { fade(danger, 0.85) },
        });
    }
    if healing {
        st.animating = true;
        let pulse = 0.6 + 0.4 * ((f.now as f32 * std::f32::consts::TAU).cos() * 0.5 + 0.5);
        layers.push(Layer {
            from: r(p.hp),
            to: r(p.hp_healing_to),
            color: fade(cv.tok("heal"), pulse),
        });
    }
    let tint = STATUS_TINTS.iter().find(|(k, _)| p.debuffs().any(|d| d.key == *k)).map(|(_, t)| cv.tok(t));
    let fill = match (tint, low) {
        (Some(c), _) => fade(c, blink),
        (None, true) => fade(danger, blink),
        (None, false) => text,
    };
    layers.push(Layer::fill(r(p.hp), fill));
    cv.bar_ex("player-hp", 0.0, y, w, 14.0, &layers, false, true);
    let mut bottom = y + 14.0;

    // Stamina bar and value
    if cfg.shows("p-st") && p.stamina_max > 0.0 {
        let sy = bottom + 5.0;
        let tired = p.stamina < p.stamina_max * 0.25;
        let c = if tired { cv.tok("accent") } else { mute };
        cv.bar("stamina", 0.0, sy, px(p.stamina_max), 6.0, &[Layer::fill(p.stamina / p.stamina_max, c)], false);
        if cfg.shows("p-num") {
            let s = format!("{}", p.stamina.round());
            let sz = cv.text(
                px(p.stamina_max) + 8.0,
                sy - 2.0,
                &s,
                Font::NumBold,
                11.0,
                if tired { cv.tok("accent") } else { text },
                Align2::LEFT_TOP,
            );
            cv.text(
                px(p.stamina_max) + 8.0 + sz.x + 1.0,
                sy - 1.0,
                &format!("/{}", p.stamina_max.round()),
                Font::NumBold,
                10.0,
                mute,
                Align2::LEFT_TOP,
            );
        }
        bottom = sy + 6.0;
    }

    // Ticks: a short one every 25 points, a long one every 50
    if cfg.shows("p-ruler") && f.density != rh_core::settings::Density::Minimal {
        let ry = bottom + 3.0;
        let c = fade(text, 0.55);
        let mut v = 0.0;
        while v <= wmax + 0.1 {
            let long = (v as i32) % 50 == 0;
            cv.rect(px(v), ry + if long { 0.0 } else { 3.0 }, 1.0, if long { 5.0 } else { 2.0 }, c);
            v += 25.0;
        }
        bottom = ry + 5.0;
    }
    let col_w = px(wmax) + 64.0;

    // Status effects: next to the right end of the HP bar
    let mut right = col_w;
    if cfg.shows("p-ails") {
        let mut ax = col_w + 16.0;
        for d in p.debuffs() {
            let name = f.i18n.game(&d.key);
            let c = status_color(cv, &d.key).unwrap_or(cv.tok("accent"));
            let tsz = cv.tag(
                ax,
                27.0,
                &name,
                Font::TextBold,
                11.0,
                (6.0, 2.0, 6.0, 1.0),
                c,
                Color32::from_rgb(0x11, 0x11, 0x11),
            );
            let t = buff_time(d.timer, d.infinite);
            let ts = cv.text(ax + tsz.x + 5.0, 27.0, &t, Font::Num, 12.0, c, Align2::LEFT_TOP);
            let rw = tsz.x + 5.0 + ts.x;
            let ratio = if d.timer_max > 0.0 { d.timer / d.timer_max } else { 1.0 };
            cv.bar("debuff", ax, 27.0 + tsz.y + 4.0, rw, 2.0, &[Layer::fill(ratio, c)], false);
            ax += rw + 12.0;
            right = right.max(ax);
        }
    }

    // Sharpness: a thick bar sized like the game's, the hits left as a number; the colour names the level
    if cfg.shows("p-sharp")
        && let Some(sh) = &p.sharpness
    {
        let sy = bottom + 10.0;
        let seg_w = 150.0;
        let bar_h = 9.0;
        let total = sh.thresholds.iter().copied().filter(|&t| t > 0).max().unwrap_or(1).max(1) as f32;
        let mut start = 0;
        let mut x = 0.0;
        cv.rect(-1.0, sy - 1.0, seg_w + 2.0, bar_h + 2.0, cv.tok("edge"));
        for (i, &end) in sh.thresholds.iter().enumerate() {
            if end <= start {
                continue;
            }
            let len = (end - start) as f32 / total * seg_w;
            let color = SHARP_COLORS[i.min(6)];
            // Darken the part that's used up
            let used = ((sh.current - start) as f32 / (end - start) as f32).clamp(0.0, 1.0);
            cv.rect(x, sy, (len - 1.0).max(0.0), bar_h, color);
            if used < 1.0 {
                cv.rect(x + len * used, sy, (len * (1.0 - used) - 1.0).max(0.0), bar_h, rgba(0, 0, 0, 0.75));
            }
            x += len;
            start = end;
        }
        let mk = (sh.current as f32 / total * seg_w).clamp(0.0, seg_w);
        cv.rect(mk - 1.0, sy - 3.0, 2.0, bar_h + 6.0, text);
        let hits = sh.hits_left();
        let warn = hits < 10 && sh.level != SharpnessLevel::Red;
        cv.text_ex(
            seg_w + 10.0,
            sy - 7.0,
            &hits.to_string(),
            Font::NumHeavy,
            20.0,
            if warn { cv.tok("accent") } else { SHARP_COLORS[sh.level.index().min(6)] },
            Align2::LEFT_TOP,
            0.0,
            true,
        );
        bottom = sy + bar_h;
        right = right.max(seg_w + 60.0);

        // Frostcraft and similar: a gauge under the sharpness bar
        for a in p.abnormalities.iter().filter(|a| GAUGES.contains(&a.key.as_str())) {
            let gy = bottom + 4.0;
            let ratio = if a.timer_max > 0.0 { a.timer / a.timer_max } else { 0.0 };
            cv.bar("frostcraft", 0.0, gy, seg_w, 4.0, &[Layer::fill(ratio, cv.tok("frostcraft"))], false);
            bottom = gy + 4.0;
        }
    }
    cv.crosshair(-18.0, 26.0);
    vec2(right.max(col_w), bottom)
}

pub fn buffs(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let p = &f.snap.player;
    let text = cv.tok("text");
    let accent = cv.tok("accent");
    let mut y = 0.0;
    let mut w = 0.0f32;
    let trays = &f.profile.buffs.trays;
    let player_on = f.profile.module(modules::PLAYER).is_some_and(|m| m.enabled);
    let low_secs = f.profile.buffs.low_secs;
    let wrap = modules::width(modules::BUFFS, false, cfg.width);
    let mut any = false;
    for (ti, tray) in trays.iter().enumerate() {
        let list: Vec<_> = p
            .abnormalities
            .iter()
            .filter(|a| a.kind != AbnormalityKind::Debuff && !GAUGES.contains(&a.key.as_str()))
            .filter(|a| tray.ids.is_empty() || tray.ids.contains(&a.id) || tray.ids.contains(&a.key))
            .collect();
        if list.is_empty() {
            continue;
        }
        if !any {
            y += module_header(cv, &f.t("module.buffs"), 260.0, "", Some(40.0));
            any = true;
        }
        let _ = ti;
        let mut x = 0.0;
        let mut row_h = 0.0f32;
        for a in list {
            let name = f.i18n.game(&a.key);
            let is_low = !a.infinite && a.timer < low_secs && cfg.shows("b-low");
            let (bg, fg) = if is_low {
                (accent, Color32::from_rgb(0x11, 0x11, 0x11))
            } else {
                (cv.tok("ink"), text)
            };
            let tsz = cv.tag(x, y, &name, Font::TextBold, 10.0, (5.0, 2.0, 5.0, 1.0), bg, fg);
            x += tsz.x;
            if cfg.shows("b-time") {
                let s = buff_time(a.timer, a.infinite);
                let ts = cv.text(x + 5.0, y, &s, Font::Num, 12.0, if is_low { accent } else { text }, Align2::LEFT_TOP);
                x += 5.0 + ts.x;
            }
            x += 10.0;
            row_h = row_h.max(tsz.y);
            if x > wrap {
                w = w.max(x);
                x = 0.0;
                y += row_h + 6.0;
            }
        }
        w = w.max(x);
        y += row_h + 6.0;
    }
    // With the player module off, status effects show here
    if cfg.shows("b-debuff") && !player_on {
        let mut x = 0.0;
        let mut row_h = 0.0f32;
        for d in p.debuffs() {
            if !any {
                y += module_header(cv, &f.t("module.buffs"), 260.0, "", Some(40.0));
                any = true;
            }
            let c = status_color(cv, &d.key).unwrap_or(accent);
            let tsz = cv.tag(
                x,
                y,
                &f.i18n.game(&d.key),
                Font::TextBold,
                10.0,
                (5.0, 2.0, 5.0, 1.0),
                c,
                Color32::from_rgb(0x11, 0x11, 0x11),
            );
            let ts = cv.text(x + tsz.x + 5.0, y, &buff_time(d.timer, d.infinite), Font::Num, 12.0, c, Align2::LEFT_TOP);
            x += tsz.x + 5.0 + ts.x + 10.0;
            row_h = row_h.max(tsz.y);
        }
        w = w.max(x);
        y += row_h;
    }
    if !any && !f.preview {
        return Vec2::ZERO;
    }
    if !any {
        y += module_header(cv, &f.t("module.buffs"), 260.0, "", Some(40.0));
    }
    cv.crosshair(-18.0, -2.0);
    vec2(w.max(260.0), y)
}

pub fn weapon(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, _st: &mut State) -> Vec2 {
    let p = &f.snap.player;
    if matches!(p.weapon_state, WeaponState::None) {
        return Vec2::ZERO;
    }
    let w = modules::width(modules::WEAPON, false, cfg.width);
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    let mut y = module_header(cv, &f.t(&format!("weapon.{}", p.weapon.key())), w, "", None);
    let show_gauge = cfg.shows("w-gauge");
    let show_num = cfg.shows("w-num");
    let row = |y: f32, label: &str, ratio: f32, color: Color32, value: &str| -> f32 {
        cv.text(0.0, y - 1.0, label, Font::Text, 11.0, mute, Align2::LEFT_TOP);
        if show_gauge {
            cv.bar("weapon", 44.0, y + 4.0, w - 44.0 - 52.0, 5.0, &[Layer::fill(ratio, color)], false);
        }
        if show_num && !value.is_empty() {
            let vs = cv.measure(value, Font::Num, 12.0, 0.0);
            cv.text(w - vs.x, y - 2.0, value, Font::Num, 12.0, text, Align2::LEFT_TOP);
        }
        y + 18.0
    };
    match &p.weapon_state {
        WeaponState::Longsword { gauge, level, level_timer, .. } => {
            y = row(y, &f.t("weapon.spirit_gauge"), gauge / 100.0, text, "");
            let (lv_name, lv_color) = match level {
                1 => (f.t("weapon.spirit_white"), Color32::from_rgb(0xf2, 0xf0, 0xe9)),
                2 => (f.t("weapon.spirit_yellow"), Color32::from_rgb(0xf2, 0xcf, 0x3a)),
                3 => (f.t("weapon.spirit_red"), Color32::from_rgb(0xe8, 0x5a, 0x31)),
                _ => (f.t("weapon.spirit_none"), Color32::TRANSPARENT),
            };
            cv.text(0.0, y - 1.0, &f.t("weapon.spirit_level"), Font::Text, 11.0, mute, Align2::LEFT_TOP);
            cv.diamond(48.0, y + 6.0, 8.0, lv_color, Color32::from_rgba_unmultiplied(0, 0, 0, 150));
            cv.text(58.0, y - 1.0, &lv_name, Font::Text, 12.0, text, Align2::LEFT_TOP);
            if *level > 0 && show_num {
                let s = mmss(*level_timer);
                let vs = cv.measure(&s, Font::Num, 12.0, 0.0);
                cv.text(w - vs.x, y - 2.0, &s, Font::Num, 12.0, text, Align2::LEFT_TOP);
            }
            y += 18.0;
        }
        WeaponState::DualBlades { gauge, demon_mode, arch_demon } => {
            let v = if *demon_mode {
                f.t("weapon.demon_mode")
            } else if *arch_demon {
                f.t("weapon.arch_demon")
            } else {
                String::new()
            };
            y = row(y, &f.t("weapon.demon_gauge"), gauge / 100.0, if *demon_mode { accent } else { text }, &v);
        }
        WeaponState::InsectGlaive {
            red,
            white,
            orange,
            kinsect_stamina,
            kinsect_stamina_max,
        } => {
            let mut x = 0.0;
            for (name, t, c) in [
                (f.t("weapon.extract_red"), red, Color32::from_rgb(0xe5, 0x48, 0x4d)),
                (f.t("weapon.extract_white"), white, Color32::from_rgb(0xf2, 0xf0, 0xe9)),
                (f.t("weapon.extract_orange"), orange, Color32::from_rgb(0xff, 0x9a, 0x3c)),
            ] {
                let on = *t > 0.0;
                cv.rect(x, y + 3.0, 8.0, 8.0, if on { c } else { fade(c, 0.3) });
                let ns = cv.text(x + 13.0, y, &name, Font::Text, 11.0, if on { text } else { mute }, Align2::LEFT_TOP);
                x += 13.0 + ns.x + 5.0;
                if on && show_num {
                    let ts = cv.text(x, y, &format!("{}s", t.ceil()), Font::NumBold, 11.0, text, Align2::LEFT_TOP);
                    x += ts.x;
                }
                x += 12.0;
            }
            y += 18.0;
            let r = if *kinsect_stamina_max > 0.0 {
                kinsect_stamina / kinsect_stamina_max
            } else {
                0.0
            };
            y = row(y, &f.t("weapon.kinsect"), r, text, "");
        }
        WeaponState::ChargeBlade {
            phials,
            phials_max,
            shield,
            sword,
            axe,
            ..
        } => {
            cv.text(0.0, y - 1.0, &f.t("weapon.phials"), Font::Text, 11.0, mute, Align2::LEFT_TOP);
            for i in 0..*phials_max {
                let on = i < *phials;
                let px = 44.0 + i as f32 * 13.0;
                let stroke = egui::Stroke::new(cv.scale, cv.c(text));
                cv.painter.rect(
                    cv.r(px, y + 3.0, 10.0, 6.0),
                    egui::CornerRadius::ZERO,
                    if on { cv.c(text) } else { Color32::TRANSPARENT },
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
            if show_num {
                let s = format!("{phials}/{phials_max}");
                let vs = cv.measure(&s, Font::Num, 12.0, 0.0);
                cv.text(w - vs.x, y - 2.0, &s, Font::Num, 12.0, text, Align2::LEFT_TOP);
            }
            y += 18.0;
            let mut x = 0.0;
            for (name, t) in [
                (f.t("weapon.shield_charge"), shield),
                (f.t("weapon.sword_charge"), sword),
                (f.t("weapon.axe_charge"), axe),
            ] {
                let ns = cv.text(x, y - 1.0, &name, Font::Text, 11.0, mute, Align2::LEFT_TOP);
                let v = if *t > 0.0 { mmss(*t) } else { "—".into() };
                let vs = cv.text(x + ns.x + 6.0, y - 2.0, &v, Font::Num, 12.0, text, Align2::LEFT_TOP);
                x += ns.x + 6.0 + vs.x + 14.0;
            }
            y += 18.0;
        }
        WeaponState::SwitchAxe { gauge, awakened_timer, .. } => {
            let awake = *awakened_timer > 0.0;
            let v = if awake { f.t("weapon.awakened") } else { String::new() };
            y = row(y, &f.t("weapon.switch_gauge"), gauge / 100.0, if awake { accent } else { text }, &v);
            if awake {
                y = row(y, &f.t("weapon.amped"), 1.0, accent, &mmss(*awakened_timer));
            }
        }
        WeaponState::None => {}
    }
    cv.crosshair(-18.0, -2.0);
    vec2(w, y - 4.0)
}
