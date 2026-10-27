//! Main window widgets: square corners, thin lines; black blocks only on primary buttons and name tags.
//! Every control has hover, press and keyboard focus states, and eases between them.

use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use ui_kit::Font;

use super::style::Pal;

/// Type scale: one large number per page, page titles, headings, body text, small labels.
pub const T_DISPLAY: f32 = 44.0;
pub const T_TITLE: f32 = 28.0;
pub const T_HEAD: f32 = 16.0;
pub const T_BODY: f32 = 13.0;
pub const T_SMALL: f32 = 12.0;
/// Only for small numbers such as list indices
pub const T_MICRO: f32 = 10.0;

/// How long hover and press transitions take.
const EASE_SECS: f32 = 0.12;

pub fn rich(text: impl Into<String>, font: Font, size: f32, color: Color32) -> RichText {
    RichText::new(text).font(font.id(size)).color(color)
}

pub fn label(ui: &mut Ui, text: impl Into<String>, font: Font, size: f32, color: Color32) -> Response {
    ui.add(egui::Label::new(rich(text, font, size, color)).selectable(false))
}

pub fn galley(ui: &Ui, text: &str, font: Font, size: f32, color: Color32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_no_wrap(text.to_string(), font.id(size), color)
}

pub fn text_at(ui: &Ui, pos: Pos2, align: Align2, text: &str, font: Font, size: f32, color: Color32) -> Rect {
    ui.painter().text(pos, align, text, font.id(size), color)
}

/// Moves a value for `id` towards 1 while `on`, towards 0 otherwise, over a short time.
/// Asks for the next frame at a 16 ms pace while moving; a plain repaint request would starve the overlay.
pub fn ease(ui: &Ui, id: Id, on: bool) -> f32 {
    let ctx = ui.ctx();
    let now = ctx.input(|i| i.time);
    let key = id.with("ease");
    let (v, t) = ctx.data(|d| d.get_temp::<(f32, f64)>(key)).unwrap_or((if on { 1.0 } else { 0.0 }, now));
    let step = ((now - t) as f32 / EASE_SECS).max(0.0);
    let v = if on { (v + step).min(1.0) } else { (v - step).max(0.0) };
    ctx.data_mut(|d| d.insert_temp(key, (v, now)));
    if (on && v < 1.0) || (!on && v > 0.0) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    v
}

/// Hover strength for a response: 0 idle, 1 hovered, 2 pressed, eased.
fn hover_level(ui: &Ui, resp: &Response) -> f32 {
    let h = ease(ui, resp.id.with("hover"), resp.hovered());
    let p = ease(ui, resp.id.with("press"), resp.is_pointer_button_down_on());
    h + p
}

/// A 2px ring around a control that has keyboard focus.
fn focus_ring(ui: &Ui, pal: &Pal, rect: Rect, resp: &Response) {
    if resp.has_focus() && !resp.is_pointer_button_down_on() {
        ui.painter()
            .rect_stroke(rect.expand(3.0), CornerRadius::ZERO, Stroke::new(2.0, pal.hue), StrokeKind::Outside);
    }
}

fn pointer(ui: &Ui, resp: &Response) {
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Outline,
    Primary,
}

pub fn button(ui: &mut Ui, pal: &Pal, text: &str, kind: Btn, small: bool) -> Response {
    let size = if small { T_SMALL } else { T_BODY };
    let font = if kind == Btn::Primary { Font::TextBold } else { Font::Text };
    let g = galley(ui, text, font, size, pal.ink);
    let h = if small { 28.0 } else { 36.0 };
    let pad = if small { 12.0 } else { 16.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + pad * 2.0, h), Sense::click());
    let t = hover_level(ui, &resp);
    let disabled = !ui.is_enabled();
    let (bg, fg, border) = match kind {
        // Lighter on hover, darker while pressed
        Btn::Primary => {
            let bg = if t <= 1.0 {
                pal.hue.lerp_to_gamma(pal.paper, 0.16 * t)
            } else {
                pal.hue.lerp_to_gamma(pal.paper, 0.16 * (2.0 - t)).lerp_to_gamma(pal.ink, 0.2 * (t - 1.0))
            };
            (bg, pal.onhue, bg)
        }
        Btn::Outline => (pal.tint(t), pal.ink, pal.ink),
    };
    let p = ui.painter();
    // Pressed buttons sink by a pixel
    let r = rect.translate(vec2(0.0, (t - 1.0).max(0.0)));
    p.rect(
        r,
        CornerRadius::ZERO,
        bg,
        Stroke::new(1.0, if disabled { pal.faint } else { border }),
        StrokeKind::Inside,
    );
    let fg = if disabled { pal.faint } else { fg };
    text_at(ui, r.center(), Align2::CENTER_CENTER, text, font, size, fg);
    focus_ring(ui, pal, rect, &resp);
    pointer(ui, &resp);
    resp
}

/// Primary button followed by a hotkey hint.
pub fn button_key(ui: &mut Ui, pal: &Pal, text: &str, key: &str) -> Response {
    let g1 = galley(ui, text, Font::TextBold, T_BODY, pal.onhue);
    let g2 = galley(ui, key, Font::Mono, T_MICRO, pal.onhue);
    let (rect, resp) = ui.allocate_exact_size(vec2(g1.size().x + g2.size().x + 42.0, 36.0), Sense::click());
    let t = hover_level(ui, &resp);
    let bg = pal.hue.lerp_to_gamma(pal.paper, 0.16 * t.min(1.0));
    let r = rect.translate(vec2(0.0, (t - 1.0).max(0.0)));
    ui.painter().rect_filled(r, CornerRadius::ZERO, bg);
    let x = r.min.x + 16.0;
    text_at(ui, pos2(x, r.center().y), Align2::LEFT_CENTER, text, Font::TextBold, T_BODY, pal.onhue);
    text_at(
        ui,
        pos2(x + g1.size().x + 10.0, r.center().y + 1.0),
        Align2::LEFT_CENTER,
        key,
        Font::Mono,
        T_MICRO,
        pal.onhue.gamma_multiply(0.55),
    );
    focus_ring(ui, pal, rect, &resp);
    pointer(ui, &resp);
    resp
}

/// Underlined text link; the underline takes the interaction colour on hover.
pub fn link(ui: &mut Ui, pal: &Pal, text: &str) -> Response {
    let g = galley(ui, text, Font::Text, T_BODY, pal.ink);
    let (rect, resp) = ui.allocate_exact_size(g.size() + vec2(0.0, 2.0), Sense::click());
    let color = if ui.is_enabled() { pal.ink } else { pal.faint };
    ui.painter().galley(rect.min, g, color);
    let t = ease(ui, resp.id.with("hover"), resp.hovered());
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)],
        Stroke::new(1.0 + t, pal.faint.lerp_to_gamma(pal.hue, t)),
    );
    focus_ring(ui, pal, rect, &resp);
    pointer(ui, &resp);
    resp
}

/// Square switch, 28x14; filled with the interaction colour when on.
pub fn switch(ui: &mut Ui, pal: &Pal, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(28.0, 14.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ease(ui, resp.id, *on);
    let h = ease(ui, resp.id.with("hover"), resp.hovered());
    let p = ui.painter();
    let line = if ui.is_enabled() { pal.ink } else { pal.faint };
    p.rect(
        rect,
        CornerRadius::ZERO,
        pal.tint(h).lerp_to_gamma(pal.hue, t),
        Stroke::new(1.0, line.lerp_to_gamma(pal.hue, t)),
        StrokeKind::Inside,
    );
    let knob = Rect::from_min_size(pos2(rect.min.x + 3.0 + 14.0 * t, rect.min.y + 3.0), vec2(8.0, 8.0));
    p.rect_filled(knob, CornerRadius::ZERO, pal.ink.lerp_to_gamma(pal.onhue, t));
    focus_ring(ui, pal, rect, &resp);
    pointer(ui, &resp);
    resp
}

/// Segmented choice: thin frame, the selected cell filled with the interaction colour.
pub fn seg(ui: &mut Ui, pal: &Pal, options: &[String], selected: &mut usize) -> bool {
    let mut changed = false;
    let widths: Vec<f32> = options.iter().map(|o| galley(ui, o, Font::Text, T_BODY, pal.ink).size().x + 24.0).collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(vec2(total, 28.0), Sense::hover());
    let mut x = rect.min.x;
    for (i, (o, w)) in options.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(*w, rect.height()));
        let resp = ui.interact(r, ui.scope_id().with(("seg", i, o)), Sense::click());
        let on = ease(ui, resp.id.with("on"), i == *selected);
        let t = hover_level(ui, &resp);
        ui.painter().rect_filled(r, CornerRadius::ZERO, pal.tint(t).lerp_to_gamma(pal.hue, on));
        text_at(
            ui,
            r.center(),
            Align2::CENTER_CENTER,
            o,
            Font::Text,
            T_BODY,
            pal.ink.lerp_to_gamma(pal.onhue, on),
        );
        if resp.clicked() && i != *selected {
            *selected = i;
            changed = true;
        }
        focus_ring(ui, pal, r, &resp);
        pointer(ui, &resp);
        x += w;
    }
    ui.painter()
        .rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, pal.ink), StrokeKind::Inside);
    changed
}

/// Tabs: the selected one is bold with a 2px underline in the interaction colour. Returns the index clicked.
pub fn tabs(ui: &mut Ui, pal: &Pal, items: &[(String, bool)], selected: usize, big: bool) -> Option<usize> {
    let size = if big { T_HEAD } else { T_BODY };
    let gap = if big { 32.0 } else { 24.0 };
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (i, (t, enabled)) in items.iter().enumerate() {
            let on = i == selected;
            let font = if on { Font::TextBold } else { Font::Text };
            let g = galley(ui, t, font, size, pal.ink);
            let (rect, resp) = ui.allocate_exact_size(
                g.size() + vec2(0.0, if big { 12.0 } else { 8.0 }),
                if *enabled { Sense::click() } else { Sense::hover() },
            );
            let h = ease(ui, resp.id.with("hover"), resp.hovered() && *enabled);
            let color = if !enabled {
                pal.faint
            } else if on {
                pal.ink
            } else {
                pal.mute.lerp_to_gamma(pal.ink, h)
            };
            ui.painter().galley(rect.min, g, color);
            let u = ease(ui, resp.id.with("on"), on);
            if u > 0.0 {
                // The underline grows out from the middle when a tab is picked
                let w = rect.width() * u;
                let x = rect.center().x - w / 2.0;
                ui.painter().rect_filled(
                    Rect::from_min_max(pos2(x, rect.max.y - 2.0), pos2(x + w, rect.max.y)),
                    CornerRadius::ZERO,
                    pal.hue,
                );
            }
            if resp.clicked() && *enabled {
                clicked = Some(i);
            }
            focus_ring(ui, pal, rect, &resp);
            if *enabled {
                pointer(ui, &resp);
            }
        }
    });
    clicked
}

/// Thin slider: 1px track, the chosen part in the interaction colour, a square handle. Value and unit on the right.
pub fn range(ui: &mut Ui, pal: &Pal, id: &str, value: &mut f32, min: f32, max: f32, step: f32, unit: &str, width: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        // Total width = track + 66
        ui.spacing_mut().item_spacing.x = 0.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
        let _ = id;
        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let t = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let v = ((min + t * (max - min)) / step).round() * step;
            if (v - *value).abs() > f32::EPSILON {
                *value = v;
                changed = true;
            }
        }
        // Arrow keys step the value while it has keyboard focus
        if resp.has_focus() {
            let (l, r) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
            let d = if r {
                step
            } else if l {
                -step
            } else {
                0.0
            };
            if d != 0.0 {
                *value = (*value + d).clamp(min, max);
                changed = true;
            }
        }
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let h = ease(ui, resp.id.with("hover"), resp.hovered() || resp.dragged());
        let p = ui.painter();
        let y = rect.center().y;
        let x = rect.min.x + t * rect.width();
        p.line_segment(
            [pos2(rect.min.x, y), pos2(rect.max.x, y)],
            Stroke::new(1.0, pal.line.lerp_to_gamma(pal.ink, 0.5)),
        );
        p.line_segment([pos2(rect.min.x, y), pos2(x, y)], Stroke::new(2.0, pal.hue));
        let k = 10.0 + 2.0 * h;
        p.rect_filled(Rect::from_center_size(pos2(x, y), vec2(k, k)), CornerRadius::ZERO, pal.hue);
        focus_ring(ui, pal, Rect::from_center_size(pos2(x, y), vec2(k, k)), &resp);
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        ui.add_space(14.0);
        let txt = if step < 1.0 {
            format!("{:.2} {unit}", value)
        } else {
            format!("{} {unit}", value.round())
        };
        let (r, _) = ui.allocate_exact_size(vec2(52.0, 16.0), Sense::hover());
        text_at(
            ui,
            pos2(r.max.x, r.center().y),
            Align2::RIGHT_CENTER,
            txt.trim(),
            Font::NumBold,
            T_SMALL,
            pal.ink,
        );
    });
    changed
}

/// Dropdown: just an underline.
pub fn select(ui: &mut Ui, pal: &Pal, id: &str, options: &[String], selected: &mut usize, width: f32) -> bool {
    let mut changed = false;
    let cur = options.get(*selected).cloned().unwrap_or_default();
    let resp = egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(rich(cur, Font::Text, T_BODY, pal.ink))
        .show_ui(ui, |ui| {
            for (i, o) in options.iter().enumerate() {
                if ui.selectable_label(i == *selected, rich(o, Font::Text, T_BODY, pal.ink)).clicked() {
                    *selected = i;
                    changed = true;
                }
            }
        });
    let r = resp.response.rect;
    let t = ease(ui, resp.response.id.with("hover"), resp.response.hovered());
    ui.painter().line_segment(
        [pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)],
        Stroke::new(1.0 + t, pal.ink.lerp_to_gamma(pal.hue, t)),
    );
    changed
}

/// Section title: one word and a thin line; an explanatory value can go on the right.
pub fn section(ui: &mut Ui, pal: &Pal, text: &str, right: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let r = text_at(
        ui,
        pos2(rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        Font::TextBold,
        T_HEAD,
        pal.ink,
    );
    let mut end = rect.max.x;
    if let Some(right) = right {
        let rr = text_at(
            ui,
            pos2(rect.max.x, rect.center().y),
            Align2::RIGHT_CENTER,
            right,
            Font::Text,
            T_SMALL,
            pal.mute,
        );
        end = rr.min.x - 16.0;
    }
    ui.painter()
        .line_segment([pos2(r.max.x + 16.0, rect.center().y), pos2(end, rect.center().y)], Stroke::new(1.0, pal.line));
    ui.add_space(8.0);
}

/// Page title: large text, a leader line, buttons on the right.
pub fn page_header(ui: &mut Ui, pal: &Pal, title: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        let g = galley(ui, title, Font::TextBold, T_TITLE, pal.ink);
        let (r, _) = ui.allocate_exact_size(vec2(g.size().x, 40.0), Sense::hover());
        ui.painter().galley(pos2(r.min.x, r.center().y - g.size().y / 2.0), g, pal.ink);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            actions(ui);
            let w = ui.available_width() - 24.0;
            if w > 10.0 {
                let (lr, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
                let y = lr.center().y;
                ui.painter().line_segment([pos2(lr.min.x, y), pos2(lr.max.x, y)], Stroke::new(1.0, pal.line));
                ui.painter()
                    .rect_filled(Rect::from_min_size(pos2(lr.max.x - 5.0, y - 2.0), vec2(5.0, 5.0)), CornerRadius::ZERO, pal.hue);
            }
        });
    });
    ui.add_space(24.0);
}

/// Settings row: text on the left, control on the right, a thin line below.
pub fn setting_row(ui: &mut Ui, pal: &Pal, label: &str, extra: Option<&str>, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width().min(640.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::hover());
    let t = ease(ui, resp.id.with("hover"), resp.contains_pointer());
    ui.painter().rect_filled(rect.expand2(vec2(8.0, 0.0)), CornerRadius::ZERO, pal.tint(t * 0.6));
    let lr = text_at(ui, pos2(rect.min.x, rect.center().y), Align2::LEFT_CENTER, label, Font::Text, T_BODY, pal.ink);
    if let Some(e) = extra {
        text_at(
            ui,
            pos2(lr.max.x + 8.0, rect.center().y + 1.0),
            Align2::LEFT_CENTER,
            e,
            Font::Text,
            T_SMALL,
            pal.mute,
        );
    }
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)],
        Stroke::new(1.0, pal.line),
    );
    let ctl = Rect::from_min_max(pos2(rect.min.x + w * 0.35, rect.min.y), rect.max);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(ctl).layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            control(ui);
        },
    );
}

/// Compact switch row (the "Show" list on the Modules page); the whole row toggles.
pub fn toggle_row(ui: &mut Ui, pal: &Pal, label: &str, on: &mut bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let t = hover_level(ui, &resp);
    ui.painter().rect_filled(rect, CornerRadius::ZERO, pal.tint(t * 0.6));
    let mut changed = false;
    if resp.clicked() {
        *on = !*on;
        changed = true;
    }
    text_at(
        ui,
        pos2(rect.min.x + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        Font::Text,
        T_BODY,
        if *on { pal.ink } else { pal.mute },
    );
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)],
        Stroke::new(1.0, pal.line),
    );
    let sr = Rect::from_min_size(pos2(rect.max.x - 32.0, rect.center().y - 7.0), vec2(28.0, 14.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
        changed |= switch(ui, pal, on).changed();
    });
    pointer(ui, &resp);
    changed
}

/// Hotkey box: monospace text, thin frame.
pub fn key_box(ui: &mut Ui, pal: &Pal, text: &str, active: bool) -> Response {
    let g = galley(ui, text, Font::Mono, T_SMALL, pal.ink);
    let (rect, resp) = ui.allocate_exact_size(vec2((g.size().x + 20.0).max(128.0), 28.0), Sense::click());
    let t = ease(ui, resp.id.with("hover"), resp.hovered());
    let border = if active { pal.hue } else { pal.line.lerp_to_gamma(pal.ink, t) };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(if active { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - g.size() / 2.0, g, pal.ink);
    focus_ring(ui, pal, rect, &resp);
    pointer(ui, &resp);
    resp
}

/// Input box with just an underline, thicker and in the interaction colour while typing.
pub fn text_input(ui: &mut Ui, pal: &Pal, value: &mut String, hint: &str, width: f32) -> Response {
    let resp = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(rich(hint, Font::Text, T_BODY, pal.faint))
            .font(Font::Text.id(T_BODY))
            .text_color(pal.ink)
            .frame(egui::Frame::NONE)
            .desired_width(width),
    );
    let r = resp.rect;
    let t = ease(ui, resp.id.with("focus"), resp.has_focus());
    ui.painter().line_segment(
        [pos2(r.min.x, r.max.y + 2.0), pos2(r.max.x, r.max.y + 2.0)],
        Stroke::new(1.0 + t, pal.ink.lerp_to_gamma(pal.hue, t)),
    );
    resp
}

/// Name tag on black.
pub fn tag(ui: &mut Ui, pal: &Pal, text: &str, size: f32) -> Response {
    let g = galley(ui, text, Font::TextBold, size, pal.onblock);
    let pad = vec2(size * 0.5, size * 0.2);
    let (rect, resp) = ui.allocate_exact_size(g.size() + pad * 2.0, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, pal.block);
    ui.painter().galley(rect.min + pad, g, pal.onblock);
    resp
}

/// Small diamond marking "in use" or "current".
pub fn diamond(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    let h = size / 2.0;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![center + vec2(0.0, -h), center + vec2(h, 0.0), center + vec2(0.0, h), center + vec2(-h, 0.0)],
        color,
        Stroke::NONE,
    ));
}

/// One fact: small label above, bold number below.
pub fn fact(ui: &mut Ui, pal: &Pal, label: &str, value: &str, big: f32, accent: bool) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        label_mute(ui, pal, label, T_SMALL);
        let font = if big >= 20.0 { Font::Num } else { Font::NumBold };
        self::label(ui, value, font, big, if accent { pal.hue } else { pal.ink });
    });
}

pub fn label_mute(ui: &mut Ui, pal: &Pal, text: &str, size: f32) -> Response {
    label(ui, text, Font::Text, size, pal.mute)
}

/// A thin horizontal line.
pub fn hline(ui: &mut Ui, pal: &Pal) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, pal.line);
}

/// Progress: a thin bar.
pub fn thin_bar(ui: &mut Ui, color: Color32, track: Color32, ratio: f32, size: Vec2) {
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, track);
    let fill = Rect::from_min_size(r.min, vec2(r.width() * ratio.clamp(0.0, 1.0), r.height()));
    ui.painter().rect_filled(fill, CornerRadius::ZERO, color);
}
