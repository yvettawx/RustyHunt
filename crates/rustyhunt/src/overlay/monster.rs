//! Monster module: target HP bar (Souls-style position), stamina, parts (two bars each), ailments, callouts.
//! Placed at the bottom by default; can also go on the right.

use egui::{Align2, Color32, Vec2, vec2};
use rh_core::model::{Ailment, Crown, Monster, Part, PartKind};
use rh_core::settings::{Density, ModuleConfig, MonsterLayout};
use ui_kit::color::fade;
use ui_kit::{Canvas, Font, Layer};

use super::{Frame, State, ToastStyle, mmss, thousands};

const BOTTOM_W: f32 = 820.0;
const SIDE_W: f32 = 380.0;

/// How a part or ailment should look this frame.
struct Look {
    alpha: f32,
    hot: bool,
}

fn part_look(f: &Frame, st: &State, m: &Monster, i: usize) -> Option<Look> {
    let age = st.activity.age(&format!("{}:p{i}", m.uid), f.now);
    let hot = age < f.idle_secs();
    let idle = f.profile.monster.idle_fade_secs;
    let _ = idle;
    match f.density {
        Density::Full => Some(Look { alpha: 1.0, hot }),
        Density::Standard => Some(Look {
            alpha: if hot { 1.0 } else { f.i18n_idle_opacity() },
            hot,
        }),
        Density::Minimal => hot.then_some(Look { alpha: 1.0, hot }),
    }
}

fn ailment_look(f: &Frame, st: &State, m: &Monster, a: &Ailment) -> Option<Look> {
    let age = st.activity.age(&format!("{}:a{}", m.uid, a.key), f.now);
    let hot = age < f.idle_secs() || a.active();
    let empty = a.buildup <= 0.0 && a.count == 0 && !a.active();
    match f.density {
        Density::Full => Some(Look { alpha: 1.0, hot }),
        Density::Standard if empty => None,
        Density::Standard => Some(Look {
            alpha: if hot { 1.0 } else { f.i18n_idle_opacity() },
            hot,
        }),
        Density::Minimal => hot.then_some(Look { alpha: 1.0, hot }),
    }
}

impl Frame<'_> {
    fn i18n_idle_opacity(&self) -> f32 {
        0.5
    }
}

fn part_value(f: &Frame, p: &Part) -> (String, bool) {
    if p.broken() {
        let k = if p.kind == PartKind::Severable { "part.severed" } else { "part.broken" };
        (f.t(k), true)
    } else if p.breaks_max > 0 {
        (format!("{}%", (p.durability_ratio() * 100.0).round()), false)
    } else {
        ("—".into(), false)
    }
}

fn ailment_value(a: &Ailment) -> String {
    if a.active() && a.timer_max > 2.0 {
        mmss(a.timer)
    } else if a.count > 0 && a.buildup <= 0.0 {
        format!("×{}", a.count)
    } else {
        format!("{}%", (a.buildup_ratio() * 100.0).round())
    }
}

/// Where the top of the text is when the name row is bottom-aligned (sink estimated from the font size).
fn top_for_bottom(bottom: f32, size: f32, font: Font) -> f32 {
    let line = match font {
        Font::Num | Font::NumHeavy | Font::NumBold => 1.22,
        _ => 1.32,
    };
    let descent = match font {
        Font::Num | Font::NumHeavy | Font::NumBold => 0.24,
        _ => 0.28,
    };
    bottom - size * (line - descent)
}

pub fn draw(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, st: &mut State) -> Vec2 {
    let Some(m) = f.target else { return Vec2::ZERO };
    let side = f.profile.monster.layout == MonsterLayout::Side;
    let w = if side { SIDE_W } else { BOTTOM_W };
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    let ratio = m.hp_ratio();

    // Trail: pause briefly after damage, then catch up
    let ghost = st.ghost.entry(m.uid).or_insert(m.hp);
    if *ghost < m.hp {
        *ghost = m.hp;
    }
    if *ghost > m.hp {
        let step = (m.hp_max * 0.01 + (*ghost - m.hp) * 1.6) * st.dt;
        *ghost = (*ghost - step.max(0.1)).max(m.hp);
        st.animating = true;
    }
    let ghost_ratio = if m.hp_max > 0.0 { *ghost / m.hp_max } else { 0.0 };

    let mut y = 0.0;
    if !side && cfg.shows("m-parts") && !m.parts.is_empty() {
        y += parts_grid(cv, f, cfg, st, m, w) + 12.0;
    }
    let top_of_header = y;

    // Name row
    let header_h = if side { 26.0 } else { 28.0 };
    let bottom = y + header_h;
    let name = f.i18n.monster(&m.key);
    let tag_size = 17.0;
    let tag_pad = (8.0, 3.0, 8.0, 2.0);
    let tsz = cv.tag_size(&name, Font::TextBold, tag_size, tag_pad);
    let mut x = 0.0;
    cv.tag(x, bottom - tsz.y, &name, Font::TextBold, tag_size, tag_pad, cv.tok("ink"), text);
    x += tsz.x + 10.0;
    if !side && cfg.shows("m-en") {
        let en = f.i18n.monster_in(&m.key, rh_core::i18n::Lang::En).to_uppercase();
        if !en.is_empty() && f.i18n.term_lang() != rh_core::i18n::Lang::En {
            let sz = cv.text_ex(
                x,
                top_for_bottom(bottom - 2.0, 11.0, Font::Num),
                &en,
                Font::Num,
                11.0,
                mute,
                Align2::LEFT_TOP,
                0.2,
                true,
            );
            x += sz.x + 10.0;
        }
    }
    if cfg.shows("m-crown") && matches!(m.crown, Crown::Gold | Crown::Silver | Crown::Mini) {
        let cw = if m.crown == Crown::Mini { 12.0 } else { 16.0 };
        cv.crown(x, bottom - 3.0 - cw * 0.75, cw, m.crown != Crown::Silver);
        x += cw + 10.0;
    }
    if cfg.shows("m-state") {
        let (label, color) = if m.enraged {
            (format!("{} {}", f.t("state.enraged"), mmss(m.enrage_timer)), Some(accent))
        } else if m.tired {
            (f.t("state.tired"), Some(cv.tok("cool")))
        } else if m.capturable() && m.alive() {
            (f.t("state.capturable"), Some(accent))
        } else {
            (String::new(), None)
        };
        if let Some(c) = color {
            let ty = top_for_bottom(bottom - 2.0, 12.0, Font::TextBold);
            cv.dot(x + 3.5, ty + 9.0, 3.5, c, true);
            let sz = cv.text(x + 13.0, ty, &label, Font::TextBold, 12.0, c, Align2::LEFT_TOP);
            x += 13.0 + sz.x + 10.0;
            if m.enraged {
                st.animating = true;
            }
        }
    }
    if cfg.shows("m-weak") && !m.weaknesses.is_empty() && f.density != Density::Minimal {
        let ty = top_for_bottom(bottom - 2.0, 10.0, Font::NumBold);
        let sz = cv.text_ex(x + 4.0, ty, &f.t("label.weak"), Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
        let mut wx = x + 4.0 + sz.x;
        for e in &m.weaknesses {
            let n = f.t(&format!("element.{}", e.key()));
            let s = cv.text(
                wx + 5.0,
                top_for_bottom(bottom - 2.0, 11.0, Font::TextBold),
                &n,
                Font::TextBold,
                11.0,
                text,
                Align2::LEFT_TOP,
            );
            wx += 5.0 + s.x;
        }
    }

    // Right side: HP value, combo damage, percent
    let mut rx = w;
    if cfg.shows("m-pct") {
        let big = if side { 30.0 } else { 34.0 };
        let small = 14.0;
        let pct = format!("{}", (ratio * 100.0).round() as i64);
        let ssz = cv.measure("%", Font::Num, small, 0.0);
        cv.text(
            rx - ssz.x,
            top_for_bottom(bottom + 2.0, small, Font::Num),
            "%",
            Font::Num,
            small,
            text,
            Align2::LEFT_TOP,
        );
        let bsz = cv.measure(&pct, Font::NumHeavy, big, -0.02);
        cv.text_ex(
            rx - ssz.x - 1.0 - bsz.x,
            top_for_bottom(bottom + 4.0, big, Font::NumHeavy),
            &pct,
            Font::NumHeavy,
            big,
            text,
            Align2::LEFT_TOP,
            -0.02,
            true,
        );
        rx -= ssz.x + 1.0 + bsz.x + 14.0;
    }
    if cfg.shows("m-dmg") && f.now - f.live.combo_at < 1.6 && f.live.combo_damage > 0.0 {
        let s = format!("−{}", thousands(f.live.combo_damage));
        let sz = cv.measure(&s, Font::Num, 15.0, 0.0);
        cv.text(
            rx - sz.x,
            top_for_bottom(bottom - 1.0, 15.0, Font::Num),
            &s,
            Font::Num,
            15.0,
            text,
            Align2::LEFT_TOP,
        );
        rx -= sz.x + 14.0;
        st.animating = true;
    }
    if cfg.shows("m-abs") && f.density != Density::Minimal && !side {
        let s = format!("{} / {}", thousands(m.hp), thousands(m.hp_max));
        let sz = cv.measure(&s, Font::NumBold, 9.0, 0.06);
        cv.text_ex(
            rx - sz.x,
            top_for_bottom(bottom - 2.0, 9.0, Font::NumBold),
            &s,
            Font::NumBold,
            9.0,
            mute,
            Align2::LEFT_TOP,
            0.06,
            true,
        );
    }
    y = bottom + 7.0;

    // HP bar
    let capz = m.capturable() && m.alive();
    let mut layers = Vec::new();
    if cfg.shows("m-ghost") {
        layers.push(Layer::fill(ghost_ratio, cv.tok("ghost")));
    }
    layers.push(Layer::fill(ratio, if capz { accent } else { text }));
    cv.bar_ex("monster-hp", 0.0, y, w, 7.0, &layers, true, true);
    if cfg.shows("m-cap") && m.capture_threshold > 0.0 && m.alive() {
        let mx = w * m.capture_threshold;
        cv.line((mx + 0.5, y - 6.0), (mx + 0.5, y + 13.0), 3.0, fade(cv.tok("shadow"), 0.5));
        cv.line((mx, y - 6.0), (mx, y + 13.0), 2.0, accent);
        cv.text_ex(
            mx + 5.0,
            y + 6.0,
            &f.t("label.capture"),
            Font::TextBold,
            9.0,
            accent,
            Align2::LEFT_TOP,
            0.0,
            true,
        );
    }
    y += 7.0;

    if cfg.shows("m-st") {
        y += 6.0;
        let (sr, color) = if m.tired { (1.0, cv.tok("cool")) } else { (m.stamina_ratio(), mute) };
        cv.bar("monster-stamina", 0.0, y, w, 2.0, &[Layer::fill(sr, color)], false);
        y += 2.0;
    }

    // Bottom row: stamina value; in the bottom layout ailments go in this row too
    y += 6.0;
    let mut row_h = 0.0f32;
    let mut fx = 0.0;
    if cfg.shows("m-stl") && f.density != Density::Minimal {
        let s = if m.tired {
            format!("{} {}", f.t("label.stamina"), f.t("state.tired_now"))
        } else {
            format!("{} {}%", f.t("label.stamina"), (m.stamina_ratio() * 100.0).round())
        };
        let sz = cv.text_ex(0.0, y, &s, Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
        row_h = row_h.max(sz.y);
        fx = if side { sz.x + 18.0 } else { 74.0 + 18.0 };
    }
    if !side && cfg.shows("m-ails") {
        for a in &m.ailments {
            let Some(look) = ailment_look(f, st, m, a) else { continue };
            let c = cv.faded(look.alpha);
            let color = if a.active() { accent } else { text };
            let name = f.i18n.game(&a.key);
            let v = ailment_value(a);
            // Skip an item that doesn't fit whole; never go past the right end of the HP bar
            let item_w = cv.measure(&name, Font::Text, 11.0, 0.0).x + 36.0 + cv.measure(&v, Font::NumBold, 10.0, 0.0).x;
            if fx + item_w > w {
                break;
            }
            let nsz = c.text(fx, y - 1.0, &name, Font::Text, 11.0, color, Align2::LEFT_TOP);
            let bx = fx + nsz.x + 6.0;
            let r = if a.active() { a.timer_ratio() } else { a.buildup_ratio() };
            c.bar(
                "ailment",
                bx,
                y + 5.0,
                24.0,
                2.0,
                &[Layer::fill(r, if a.active() { accent } else { text })],
                false,
            );
            let vsz = c.text(
                bx + 30.0,
                y,
                &v,
                Font::NumBold,
                10.0,
                if a.active() { accent } else { cv.tok("text-mute") },
                Align2::LEFT_TOP,
            );
            fx = bx + 30.0 + vsz.x + 12.0;
            row_h = row_h.max(nsz.y);
        }
    }
    y += row_h.max(12.0);

    if side {
        if cfg.shows("m-parts") && !m.parts.is_empty() {
            y += 16.0;
            y += section(cv, f, 0.0, y, w, &f.t("label.parts"), true) + 6.0;
            y += parts_rows(cv, f, cfg, st, m, y, w);
        }
        if cfg.shows("m-ails") && !m.ailments.is_empty() {
            y += 12.0;
            y += section(cv, f, 0.0, y, w, &f.t("label.ailments"), false) + 6.0;
            y += ailments_grid(cv, f, st, m, y, w);
        }
    }

    // Crosshair marker
    let ry = if side { top_of_header + 26.0 } else { bottom - 2.0 };
    cv.crosshair(-20.0, ry);
    cv.crosshair(w + 11.0, ry);

    // Callouts, above the module's top-left corner
    if cfg.shows("m-toast")
        && let Some(t) = st.toasts.back()
    {
        let age = (f.now - t.t) as f32;
        let dur = cv.theme.tokens.duration("toast");
        if age < dur {
            let a = (age / 0.25).min(1.0).min((dur - age) / 0.25).clamp(0.0, 1.0);
            let c = cv.faded(a);
            let dy = (1.0 - (age / 0.25).min(1.0)) * 4.0;
            let (bg, fg) = match t.style {
                ToastStyle::Accent => (accent, Color32::from_rgb(0x11, 0x11, 0x11)),
                ToastStyle::Plain => (cv.tok("ink"), text),
            };
            let tsz = c.tag(0.0, -34.0 + dy, &t.tag, Font::TextBold, 12.0, (7.0, 3.0, 7.0, 2.0), bg, fg);
            if !t.text.is_empty() {
                c.text(tsz.x + 8.0, -33.0 + dy, &t.text, Font::TextBold, 13.0, text, Align2::LEFT_TOP);
            }
            st.animating = true;
        }
    }

    vec2(w, y)
}

/// Section title: one word and a thin line; the parts section has a legend after it.
fn section(cv: &Canvas, f: &Frame, x: f32, y: f32, w: f32, label: &str, legend: bool) -> f32 {
    let mute = cv.tok("text-mute");
    let sz = cv.text_ex(x, y, label, Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
    let mut end = x + w;
    if legend {
        let items = [(f.t("label.durability"), 3.0, cv.tok("text")), (f.t("label.flinch"), 2.0, mute)];
        for (name, h, c) in items.iter().rev() {
            let tw = cv.measure(name, Font::TextBold, 10.0, 0.06).x;
            cv.text_ex(end - tw, y, name, Font::TextBold, 10.0, mute, Align2::LEFT_TOP, 0.06, true);
            cv.rect(end - tw - 15.0, y + sz.y / 2.0 - h / 2.0, 12.0, *h, *c);
            end -= tw + 15.0 + 8.0;
        }
    }
    cv.rule(x + sz.x + 8.0, end - 8.0, y + sz.y / 2.0, cv.tok("text-faint"));
    sz.y
}

fn parts_grid(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, st: &State, m: &Monster, w: f32) -> f32 {
    let n = m.parts.len();
    let cols = n.clamp(1, 8);
    let rows = n.div_ceil(cols);
    let gap = 16.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    let row_h = 25.0;
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    for (i, p) in m.parts.iter().enumerate() {
        let Some(look) = part_look(f, st, m, i) else { continue };
        let col = i % cols;
        let row = i / cols;
        let x = col as f32 * (cw + gap);
        let y = row as f32 * (row_h + 8.0);
        let c = cv.faded(look.alpha);
        let flash = st.flash.get(&format!("{}:p{i}", m.uid)).is_some_and(|t| f.now - t < 0.8);
        let broken = p.broken();
        let name_color = if flash {
            accent
        } else if broken {
            mute
        } else {
            text
        };
        let (v, word) = part_value(f, p);
        let vfont = if word { Font::Text } else { Font::NumBold };
        let vw = if cfg.shows("m-pval") { c.measure(&v, vfont, 9.0, 0.0).x + 6.0 } else { 0.0 };
        let pw = if cfg.shows("m-pips") && p.breaks_max > 0 {
            p.breaks_max as f32 * 9.0 + 6.0
        } else {
            0.0
        };
        let name = c.fit(&f.i18n.game(&p.key), Font::Text, 12.0, (cw - vw - pw).max(12.0));
        let nsz = c.text(x, y, &name, Font::Text, 12.0, name_color, Align2::LEFT_TOP);
        if cfg.shows("m-pips") && p.breaks_max > 0 {
            c.pips(x + nsz.x + 6.0, y + 5.0, p.breaks_max, p.breaks, 6.0, 3.0, text);
        }
        if cfg.shows("m-pval") {
            let vs = c.measure(&v, vfont, 9.0, 0.0);
            c.text(x + cw - vs.x, y + 3.0, &v, vfont, 9.0, mute, Align2::LEFT_TOP);
        }
        let by = y + 17.0;
        let b1 = if broken || p.breaks_max == 0 { 0.0 } else { p.durability_ratio() };
        c.bar("part", x, by, cw, 3.0, &[Layer::fill(b1, if flash { accent } else { text })], broken);
        if cfg.shows("m-b2") {
            c.bar(
                "part-flinch",
                x,
                by + 5.0,
                cw,
                2.0,
                &[Layer::fill(p.flinch_ratio(), if flash { accent } else { mute })],
                false,
            );
        }
        let _ = look.hot;
    }
    rows as f32 * row_h + (rows as f32 - 1.0) * 8.0
}

fn parts_rows(cv: &Canvas, f: &Frame, cfg: &ModuleConfig, st: &State, m: &Monster, y0: f32, w: f32) -> f32 {
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    let mut y = y0;
    let name_w = 52.0;
    let pips_w = 30.0;
    let val_w = 36.0;
    let gap = 10.0;
    let bar_w = w - name_w - pips_w - val_w - gap * 3.0;
    for (i, p) in m.parts.iter().enumerate() {
        let Some(look) = part_look(f, st, m, i) else {
            continue;
        };
        let c = cv.faded(look.alpha);
        let flash = st.flash.get(&format!("{}:p{i}", m.uid)).is_some_and(|t| f.now - t < 0.8);
        let broken = p.broken();
        let color = if flash {
            accent
        } else if broken {
            mute
        } else {
            text
        };
        let name = c.fit(&f.i18n.game(&p.key), Font::Text, 12.0, name_w);
        c.text(0.0, y + 2.0, &name, Font::Text, 12.0, color, Align2::LEFT_TOP);
        let bx = name_w + gap;
        let b1 = if broken || p.breaks_max == 0 { 0.0 } else { p.durability_ratio() };
        c.bar("part", bx, y + 7.0, bar_w, 3.0, &[Layer::fill(b1, if flash { accent } else { text })], broken);
        if cfg.shows("m-b2") {
            c.bar(
                "part-flinch",
                bx,
                y + 12.0,
                bar_w,
                2.0,
                &[Layer::fill(p.flinch_ratio(), if flash { accent } else { mute })],
                false,
            );
        }
        if cfg.shows("m-pips") && p.breaks_max > 0 {
            let pw = p.breaks_max as f32 * 9.0 - 3.0;
            c.pips(bx + bar_w + gap + pips_w - pw, y + 7.0, p.breaks_max, p.breaks, 6.0, 3.0, text);
        }
        if cfg.shows("m-pval") {
            let (v, word) = part_value(f, p);
            let font = if word { Font::Text } else { Font::NumBold };
            let vs = c.measure(&v, font, 10.0, 0.0);
            c.text(w - vs.x, y + 4.0, &v, font, 10.0, mute, Align2::LEFT_TOP);
        }
        y += 21.0;
    }
    y - y0
}

fn ailments_grid(cv: &Canvas, f: &Frame, st: &State, m: &Monster, y0: f32, w: f32) -> f32 {
    let cols = 4;
    let gapx = 12.0;
    let cw = (w - gapx * (cols as f32 - 1.0)) / cols as f32;
    let text = cv.tok("text");
    let accent = cv.tok("accent");
    let mut i: usize = 0;
    for a in &m.ailments {
        let Some(look) = ailment_look(f, st, m, a) else { continue };
        let x = (i % cols) as f32 * (cw + gapx);
        let y = y0 + (i / cols) as f32 * 26.0;
        let c = cv.faded(look.alpha);
        let color = if a.active() { accent } else { text };
        c.text(x, y, &f.i18n.game(&a.key), Font::Text, 11.0, color, Align2::LEFT_TOP);
        let v = ailment_value(a);
        let vs = c.measure(&v, Font::NumBold, 10.0, 0.0);
        c.text(
            x + cw - vs.x,
            y + 1.0,
            &v,
            Font::NumBold,
            10.0,
            if a.active() { accent } else { cv.tok("text-mute") },
            Align2::LEFT_TOP,
        );
        let r = if a.active() { a.timer_ratio() } else { a.buildup_ratio() };
        c.bar("ailment", x, y + 17.0, cw, 2.0, &[Layer::fill(r, color)], false);
        i += 1;
    }
    if i == 0 { 0.0 } else { (i.div_ceil(cols)) as f32 * 26.0 - 7.0 }
}
