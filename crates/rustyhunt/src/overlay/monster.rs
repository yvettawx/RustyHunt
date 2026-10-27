//! Monster module: target HP bar (Souls-style position), stamina, parts (two bars each), ailments, callouts.
//! Placed at the bottom by default; can also go on the right.

use egui::{Align2, Color32, Vec2, vec2};
use rh_core::model::{Crown, Monster};
use rh_core::settings::{Density, ModuleConfig, MonsterLayout};
use ui_kit::color::fade;
use ui_kit::{Canvas, Font, Layer};

use super::{Frame, State, ToastStyle, mmss, thousands};

const BOTTOM_W: f32 = 820.0;
const SIDE_W: f32 = 380.0;

/// One cell of the parts or ailments grid. Both use the same cell so neither outweighs the other.
struct Cell {
    name: String,
    /// Parts: (breaks, breaks_max) drawn as pips
    pips: Option<(i32, i32)>,
    /// Ailments: how many times it has triggered
    count: i32,
    /// Exact value or time left, right-aligned
    value: Option<String>,
    bar: f32,
    /// Second, thinner bar (part flinch)
    bar2: Option<f32>,
    /// Broken or severed: name dimmed, bar hatched
    done: bool,
    /// Ailment in effect
    active: bool,
    /// Just broke or just triggered
    flash: bool,
}

fn part_cells(f: &Frame, cfg: &ModuleConfig, st: &State, m: &Monster) -> Vec<Cell> {
    m.parts
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let hot = st.activity.age(&format!("{}:p{i}", m.uid), f.now) < f.idle_secs();
            match f.density {
                Density::Full => true,
                Density::Standard => f.profile.monster.all_parts || hot,
                Density::Minimal => hot,
            }
        })
        .map(|(i, p)| {
            let done = p.broken();
            Cell {
                name: f.i18n.game(&p.key),
                pips: (cfg.shows("m-pips") && p.breaks_max > 0).then_some((p.breaks, p.breaks_max)),
                count: 0,
                value: (cfg.shows("m-pval") && !done && p.breaks_max > 0).then(|| thousands(p.durability)),
                bar: if done || p.breaks_max == 0 { 0.0 } else { p.durability_ratio() },
                bar2: cfg.shows("m-b2").then(|| p.flinch_ratio()),
                done,
                active: false,
                flash: st.flash.get(&format!("{}:p{i}", m.uid)).is_some_and(|t| f.now - t < 0.8),
            }
        })
        .collect()
}

fn ailment_cells(f: &Frame, cfg: &ModuleConfig, st: &State, m: &Monster) -> Vec<Cell> {
    m.ailments
        .iter()
        .filter(|a| {
            let hot = st.activity.age(&format!("{}:a{}", m.uid, a.key), f.now) < f.idle_secs() || a.active();
            let empty = a.buildup <= 0.0 && a.count == 0 && !a.active();
            match f.density {
                Density::Full => true,
                Density::Standard => !empty,
                Density::Minimal => hot,
            }
        })
        .map(|a| {
            let active = a.active();
            let value = if active && a.timer_max > 2.0 {
                Some(mmss(a.timer))
            } else if cfg.shows("m-pval") && !active {
                Some(format!("{}", a.buildup.round()))
            } else {
                None
            };
            Cell {
                name: f.i18n.game(&a.key),
                pips: None,
                count: a.count,
                value,
                bar: if active { a.timer_ratio() } else { a.buildup_ratio() },
                bar2: None,
                done: false,
                active,
                flash: st.flash.get(&format!("{}:a{}", m.uid, a.key)).is_some_and(|t| f.now - t < 0.8),
            }
        })
        .collect()
}

const CELL_H: f32 = 25.0;
const CELL_GAP_Y: f32 = 8.0;

/// Lays cells out in `cols` columns starting at `y0`; returns the height used.
fn grid(cv: &Canvas, cells: &[Cell], y0: f32, w: f32, cols: usize) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    let gap = 16.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    for (i, c) in cells.iter().enumerate() {
        let x = (i % cols) as f32 * (cw + gap);
        let y = y0 + (i / cols) as f32 * (CELL_H + CELL_GAP_Y);
        cell(cv, c, x, y, cw);
    }
    let rows = cells.len().div_ceil(cols) as f32;
    rows * CELL_H + (rows - 1.0) * CELL_GAP_Y
}

fn cell(cv: &Canvas, c: &Cell, x: f32, y: f32, w: f32) {
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    let hot = c.flash || c.active;
    // Right side, from the right edge inward: value, then pips or the trigger count
    let mut right = x + w;
    if let Some(v) = &c.value {
        let vs = cv.measure(v, Font::NumBold, 11.0, 0.0);
        cv.text(
            right - vs.x,
            y + 1.0,
            v,
            Font::NumBold,
            11.0,
            if c.active { accent } else { mute },
            Align2::LEFT_TOP,
        );
        right -= vs.x + 5.0;
    }
    if let Some((n, max)) = c.pips {
        let pw = max as f32 * 9.0 - 3.0;
        cv.pips(right - pw, y + 4.0, max, n, 6.0, 3.0, if c.flash { accent } else { text });
        right -= pw + 5.0;
    }
    if c.count > 0 {
        let s = c.count.to_string();
        let pad = (4.0, 1.0, 4.0, 0.0);
        let ts = cv.tag_size(&s, Font::NumBold, 10.0, pad);
        let (bg, fg) = if c.flash { (accent, cv.tok("ink")) } else { (text, cv.tok("ink")) };
        cv.tag(right - ts.x, y + 1.0, &s, Font::NumBold, 10.0, pad, bg, fg);
        right -= ts.x + 5.0;
    }
    let name_color = if hot {
        accent
    } else if c.done {
        mute
    } else {
        text
    };
    let name = cv.fit(&c.name, Font::Text, 12.0, (right - x).max(12.0));
    cv.text(x, y, &name, Font::Text, 12.0, name_color, Align2::LEFT_TOP);
    let bar_color = if hot { accent } else { text };
    cv.bar("part", x, y + 17.0, w, 3.0, &[Layer::fill(c.bar, bar_color)], c.done);
    if let Some(r) = c.bar2 {
        cv.bar("part-flinch", x, y + 22.0, w, 2.0, &[Layer::fill(r, if hot { accent } else { mute })], false);
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
    let cols = if side { 3 } else { 8 };
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
    let parts = if cfg.shows("m-parts") { part_cells(f, cfg, st, m) } else { Vec::new() };
    let ailments = if cfg.shows("m-ails") { ailment_cells(f, cfg, st, m) } else { Vec::new() };
    if parts.iter().chain(&ailments).any(|c| c.flash) {
        st.animating = true;
    }

    // Bottom layout: parts above the name row, ailments under the HP bar
    let mut y = 0.0;
    if !side && !parts.is_empty() {
        y += grid(cv, &parts, 0.0, w, cols) + 12.0;
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
    let notes = f.notes.get(&m.key);
    // Beats the largest or smallest size in the Hunter's Notes
    if cfg.shows("m-record")
        && let Some(big) = notes.and_then(|r| crate::bestiary::size_record(r, m.size))
    {
        let label = f.t(if big { "label.record_big" } else { "label.record_small" });
        let sz = cv.text(
            x,
            top_for_bottom(bottom - 2.0, 12.0, Font::TextBold),
            &label,
            Font::TextBold,
            12.0,
            text,
            Align2::LEFT_TOP,
        );
        x += sz.x + 10.0;
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
        x = wx + 10.0;
    }
    if cfg.shows("m-notes")
        && f.density != Density::Minimal
        && let Some(r) = notes
    {
        notes_text(cv, f, r, x, top_for_bottom(bottom - 2.0, 11.0, Font::NumBold), 11.0, mute, text);
    }

    // Right side: current HP, large, then the maximum
    if cfg.shows("m-abs") {
        let big = if side { 26.0 } else { 30.0 };
        let max_s = format!("/{}", thousands(m.hp_max));
        let ms = cv.measure(&max_s, Font::NumBold, 12.0, 0.0);
        cv.text(
            w - ms.x,
            top_for_bottom(bottom + 1.0, 12.0, Font::NumBold),
            &max_s,
            Font::NumBold,
            12.0,
            mute,
            Align2::LEFT_TOP,
        );
        let cur = thousands(m.hp);
        let cs = cv.measure(&cur, Font::NumHeavy, big, -0.02);
        cv.text_ex(
            w - ms.x - 3.0 - cs.x,
            top_for_bottom(bottom + 4.0, big, Font::NumHeavy),
            &cur,
            Font::NumHeavy,
            big,
            text,
            Align2::LEFT_TOP,
            -0.02,
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

    if side && !parts.is_empty() {
        y += 14.0;
        y += grid(cv, &parts, y, w, cols);
    }
    if !ailments.is_empty() {
        y += 14.0;
        y += grid(cv, &ailments, y, w, cols);
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

/// "Research 4/9  Scoutfly 2" from the Hunter's Notes; returns the width.
pub fn notes_text(cv: &Canvas, f: &Frame, r: &crate::bestiary::Row, x: f32, y: f32, size: f32, label: Color32, value: Color32) -> f32 {
    let mut cx = x;
    for (key, v) in [
        ("records.research", format!("{}/{}", r.research, r.research_cap)),
        ("records.nav", r.nav.to_string()),
    ] {
        let ls = cv.text(cx, y, &f.t(key), Font::Text, size, label, Align2::LEFT_TOP);
        let vs = cv.text(cx + ls.x + 3.0, y, &v, Font::NumBold, size, value, Align2::LEFT_TOP);
        cx += ls.x + 3.0 + vs.x + 8.0;
    }
    cx - 8.0 - x
}
