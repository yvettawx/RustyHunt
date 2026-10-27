//! Monster module: target HP bar (Souls-style position), stamina, parts (two bars each), ailments, callouts.
//! Placed at the bottom by default; can also go on the right.

use egui::{Align2, Color32, Vec2, vec2};
use rh_core::model::{Crown, Monster};
use rh_core::settings::{Density, ModuleConfig, MonsterLayout};
use ui_kit::color::fade;
use ui_kit::{Canvas, Font, Layer};

use super::{Frame, State, ToastStyle, mmss, thousands};

/// One cell of the parts or ailments grid. Both use the same cell so neither outweighs the other.
struct Cell {
    name: String,
    /// Ailments: how many times it has triggered
    count: i32,
    /// Exact value or time left, right-aligned
    value: Option<String>,
    /// The maximum after the value, smaller ("/600")
    max: Option<String>,
    bar: f32,
    /// Parts that break more than once: (breaks so far, breaks possible); the bar is split per break
    segments: Option<(i32, i32)>,
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
            // Breakable parts count down durability, with flinch as the thin bar; the others only flinch
            let breakable = p.breaks_max > 0;
            let (cur, max) = if breakable {
                (p.durability, p.durability_max)
            } else {
                (p.flinch, p.flinch_max)
            };
            let numbers = cfg.shows("m-pval") && !done && max > 0.0;
            Cell {
                name: f.i18n.game(&p.key),
                count: 0,
                value: numbers.then(|| format!("{}", cur.max(0.0).round())),
                max: numbers.then(|| format!("/{}", max.round())),
                bar: if done {
                    0.0
                } else if breakable {
                    p.durability_ratio()
                } else {
                    p.flinch_ratio()
                },
                segments: (p.breaks_max > 1).then_some((p.breaks, p.breaks_max)),
                bar2: (cfg.shows("m-b2") && breakable).then(|| p.flinch_ratio()),
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
            let numbers = cfg.shows("m-pval") && !active && a.buildup_max > 0.0;
            let value = if active && a.timer_max > 2.0 {
                Some(mmss(a.timer))
            } else if numbers {
                Some(format!("{}", a.buildup.round()))
            } else {
                None
            };
            Cell {
                name: f.i18n.game(&a.key),
                count: a.count,
                value,
                max: numbers.then(|| format!("/{}", a.buildup_max.round())),
                bar: if active { a.timer_ratio() } else { a.buildup_ratio() },
                segments: None,
                bar2: None,
                done: false,
                active,
                flash: st.flash.get(&format!("{}:a{}", m.uid, a.key)).is_some_and(|t| f.now - t < 0.8),
            }
        })
        .collect()
}

const CELL_H: f32 = 28.0;
const CELL_GAP_Y: f32 = 9.0;
/// Text baseline inside a cell, from its top
const CELL_BASE: f32 = 13.0;
const CELL_BAR: f32 = 5.0;
const CELL_BAR2: f32 = 3.0;

/// Lays cells out in `cols` columns starting at `y0`; returns the height used.
fn grid(cv: &Canvas, cells: &[Cell], y0: f32, w: f32, cols: usize) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    let gap = 18.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    for (i, c) in cells.iter().enumerate() {
        let x = (i % cols) as f32 * (cw + gap);
        let y = y0 + (i / cols) as f32 * (CELL_H + CELL_GAP_Y);
        cell(cv, c, x, y, cw);
    }
    let rows = cells.len().div_ceil(cols) as f32;
    rows * CELL_H + (rows - 1.0) * CELL_GAP_Y
}

/// One cell: name on the left and value/max on the right, on one baseline, then the bars.
/// A broken part's bar is hatched all the way.
fn cell(cv: &Canvas, c: &Cell, x: f32, y: f32, w: f32) {
    let text = cv.tok("text");
    let mute = cv.tok("text-mute");
    let accent = cv.tok("accent");
    let hot = c.flash || c.active;
    let base = y + CELL_BASE;
    // Right side, from the right edge inward: max, value, then the trigger count
    let mut right = x + w;
    if let Some(m) = &c.max {
        let ms = cv.measure(m, Font::NumBold, 10.0, 0.0);
        cv.text(
            right - ms.x,
            top_for_bottom(base, 10.0, Font::NumBold),
            m,
            Font::NumBold,
            10.0,
            mute,
            Align2::LEFT_TOP,
        );
        right -= ms.x + 1.0;
    }
    if let Some(v) = &c.value {
        let vs = cv.measure(v, Font::NumBold, 12.0, 0.0);
        let color = if c.active { accent } else { text };
        cv.text(
            right - vs.x,
            top_for_bottom(base, 12.0, Font::NumBold),
            v,
            Font::NumBold,
            12.0,
            color,
            Align2::LEFT_TOP,
        );
        right -= vs.x + 6.0;
    }
    if c.count > 0 {
        let s = c.count.to_string();
        let pad = (4.0, 1.0, 4.0, 0.0);
        let ts = cv.tag_size(&s, Font::NumBold, 10.0, pad);
        let (bg, fg) = if c.flash { (accent, cv.tok("ink")) } else { (text, cv.tok("ink")) };
        cv.tag(right - ts.x, base + 2.0 - ts.y, &s, Font::NumBold, 10.0, pad, bg, fg);
        right -= ts.x + 6.0;
    }
    let name_color = if hot {
        accent
    } else if c.done {
        mute
    } else {
        text
    };
    let name = cv.fit(&c.name, Font::Text, 12.0, (right - x).max(12.0));
    cv.text(x, top_for_bottom(base, 12.0, Font::Text), &name, Font::Text, 12.0, name_color, Align2::LEFT_TOP);
    let bar_color = if hot { accent } else { text };
    let by = base + 5.0;
    match c.segments {
        // One segment per break, used up from the right: broken ones hatched, the current one filling
        Some((broken, n)) if n > 1 && !c.done => {
            let gap = 3.0;
            let sw = (w - gap * (n - 1) as f32) / n as f32;
            let current = n - broken.clamp(0, n) - 1;
            for k in 0..n {
                let sx = x + k as f32 * (sw + gap);
                let fill = match k.cmp(&current) {
                    std::cmp::Ordering::Less => 1.0,
                    std::cmp::Ordering::Equal => c.bar,
                    std::cmp::Ordering::Greater => 0.0,
                };
                cv.bar("part", sx, by, sw, CELL_BAR, &[Layer::fill(fill, bar_color)], k > current);
            }
        }
        _ => cv.bar("part", x, by, w, CELL_BAR, &[Layer::fill(c.bar, bar_color)], c.done),
    }
    if let Some(r) = c.bar2 {
        cv.bar(
            "part-flinch",
            x,
            by + CELL_BAR + 2.0,
            w,
            CELL_BAR2,
            &[Layer::fill(r, if hot { accent } else { mute })],
            false,
        );
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
    let w = rh_core::modules::width(rh_core::modules::MONSTER, side, cfg.width);
    // Cells at least about 88 wide, at most eight to a row
    let cols = (((w + 16.0) / 136.0).floor() as usize).clamp(1, 8);
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
    cv.tag(0.0, bottom - tsz.y, &name, Font::TextBold, tag_size, tag_pad, cv.tok("ink"), text);
    // Everything after the name tag sits on the baseline of its text, the same distance apart
    let base = bottom - tsz.y + tag_pad.1 + tag_size * 1.04;
    let gap = 14.0;
    let mut x = tsz.x + gap;
    if !side && cfg.shows("m-en") {
        let en = f.i18n.monster_in(&m.key, rh_core::i18n::Lang::En).to_uppercase();
        if !en.is_empty() && f.i18n.term_lang() != rh_core::i18n::Lang::En {
            let sz = cv.text_ex(
                x,
                top_for_bottom(base, 11.0, Font::Num),
                &en,
                Font::Num,
                11.0,
                mute,
                Align2::LEFT_TOP,
                0.2,
                true,
            );
            x += sz.x + gap;
        }
    }
    if cfg.shows("m-crown") && matches!(m.crown, Crown::Gold | Crown::Silver | Crown::Mini) {
        // As in the game: the mini crown is a small gold one, the large crowns gold or silver
        let cw = if m.crown == Crown::Mini { 11.0 } else { 18.0 };
        cv.crown(x, base - cw * 10.0 / 16.0, cw, m.crown != Crown::Silver);
        x += cw + gap;
    }
    let notes = f.notes.get(&m.key);
    // Beats the largest or smallest size in the Hunter's Notes
    if cfg.shows("m-record")
        && let Some(big) = notes.and_then(|r| crate::bestiary::size_record(r, m.size))
    {
        let label = f.t(if big { "label.record_big" } else { "label.record_small" });
        let sz = cv.text(
            x,
            top_for_bottom(base, 12.0, Font::TextBold),
            &label,
            Font::TextBold,
            12.0,
            text,
            Align2::LEFT_TOP,
        );
        x += sz.x + gap;
    }
    if cfg.shows("m-state") {
        let (label, color) = if m.enraged {
            (format!("{} {}", f.t("state.enraged"), mmss(m.enrage_timer)), Some(cv.tok("enrage")))
        } else if m.tired {
            (f.t("state.tired"), Some(cv.tok("cool")))
        } else if m.capturable() && m.alive() {
            (f.t("state.capturable"), Some(accent))
        } else {
            (String::new(), None)
        };
        if let Some(c) = color {
            cv.dot(x + 3.5, base - 4.5, 3.5, c, true);
            let sz = cv.text(
                x + 12.0,
                top_for_bottom(base, 12.0, Font::TextBold),
                &label,
                Font::TextBold,
                12.0,
                c,
                Align2::LEFT_TOP,
            );
            x += 12.0 + sz.x + gap;
        }
    }
    if cfg.shows("m-weak") && !m.weaknesses.is_empty() && f.density != Density::Minimal {
        let sz = cv.text(
            x,
            top_for_bottom(base, 11.0, Font::Text),
            &f.t("label.weak"),
            Font::Text,
            11.0,
            mute,
            Align2::LEFT_TOP,
        );
        let mut wx = x + sz.x;
        for e in &m.weaknesses {
            let n = f.t(&format!("element.{}", e.key()));
            let s = cv.text(
                wx + 4.0,
                top_for_bottom(base, 12.0, Font::TextBold),
                &n,
                Font::TextBold,
                12.0,
                text,
                Align2::LEFT_TOP,
            );
            wx += 4.0 + s.x;
        }
        x = wx + gap;
    }
    if cfg.shows("m-notes")
        && f.density != Density::Minimal
        && let Some(r) = notes
    {
        notes_text(cv, f, r, x, top_for_bottom(base, 11.0, Font::Text), 11.0, mute, text);
    }

    // Right side: current HP, large, then the maximum
    if cfg.shows("m-abs") {
        let big = if side { 26.0 } else { 30.0 };
        let max_s = format!("/{}", thousands(m.hp_max));
        let ms = cv.measure(&max_s, Font::NumBold, 12.0, 0.0);
        cv.text(
            w - ms.x,
            top_for_bottom(bottom + 2.0, 12.0, Font::NumBold),
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
            top_for_bottom(bottom + 2.0, big, Font::NumHeavy),
            &cur,
            Font::NumHeavy,
            big,
            text,
            Align2::LEFT_TOP,
            -0.02,
            true,
        );
    }
    y = bottom + 8.0;
    let hp_h = 10.0;

    // HP bar
    let capz = m.capturable() && m.alive();
    let mut layers = Vec::new();
    if cfg.shows("m-ghost") {
        layers.push(Layer::fill(ghost_ratio, cv.tok("ghost")));
    }
    let fill = if capz {
        accent
    } else if m.enraged {
        cv.tok("enrage")
    } else {
        cv.tok("monster-hp")
    };
    layers.push(Layer::fill(ratio, fill));
    // One flash when the monster becomes enraged
    let since = st.flash.get(&format!("{}:enrage", m.uid)).map_or(f64::MAX, |t| f.now - t) as f32;
    if since < 0.6 {
        layers.push(Layer::fill(ratio, fade(text, 1.0 - since / 0.6)));
        st.animating = true;
    }
    cv.bar_ex("monster-hp", 0.0, y, w, hp_h, &layers, true, true);
    if cfg.shows("m-cap") && m.capture_threshold > 0.0 && m.alive() {
        let mx = w * m.capture_threshold;
        cv.line((mx + 0.5, y - 5.0), (mx + 0.5, y + hp_h + 5.0), 3.0, fade(cv.tok("shadow"), 0.5));
        cv.line((mx, y - 5.0), (mx, y + hp_h + 5.0), 2.0, accent);
    }
    y += hp_h;

    if cfg.shows("m-st") {
        y += 4.0;
        let (sr, color) = if m.tired { (1.0, cv.tok("cool")) } else { (m.stamina_ratio(), mute) };
        cv.bar("monster-stamina", 0.0, y, w, 3.0, &[Layer::fill(sr, color)], false);
        y += 3.0;
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
        let vs = cv.text(cx + ls.x + 4.0, y, &v, Font::NumBold, size, value, Align2::LEFT_TOP);
        cx += ls.x + 4.0 + vs.x + 10.0;
    }
    cx - 10.0 - x
}
