//! 主界面的控件：方角、细线、黑块只用在主按钮和名牌上。

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use ui_kit::Font;

use super::style::Pal;

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Outline,
    Primary,
}

pub fn button(ui: &mut Ui, pal: &Pal, text: &str, kind: Btn, small: bool) -> Response {
    let size = if small { 12.0 } else { 13.0 };
    let font = if kind == Btn::Primary { Font::TextBold } else { Font::Text };
    let g = galley(ui, text, font, size, pal.ink);
    let h = if small { 28.0 } else { 34.0 };
    let pad = if small { 12.0 } else { 16.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + pad * 2.0, h), Sense::click());
    let hov = resp.hovered();
    let (bg, fg, border) = match kind {
        Btn::Primary if hov => (pal.accent, Color32::from_rgb(0x14, 0x14, 0x14), pal.accent),
        Btn::Primary => (pal.block, pal.onblock, pal.block),
        Btn::Outline if hov => (pal.ink, pal.paper, pal.ink),
        Btn::Outline => (Color32::TRANSPARENT, pal.ink, pal.ink),
    };
    let disabled = !ui.is_enabled();
    let p = ui.painter();
    p.rect(rect, CornerRadius::ZERO, bg, Stroke::new(1.0, border), StrokeKind::Inside);
    let fg = if disabled { pal.faint } else { fg };
    text_at(ui, rect.center(), Align2::CENTER_CENTER, text, font, size, fg);
    if hov {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 主按钮后面带一个热键提示。
pub fn button_key(ui: &mut Ui, pal: &Pal, text: &str, key: &str) -> Response {
    let g1 = galley(ui, text, Font::TextBold, 13.0, pal.onblock);
    let g2 = galley(ui, key, Font::Mono, 10.0, pal.onblock);
    let (rect, resp) = ui.allocate_exact_size(vec2(g1.size().x + g2.size().x + 42.0, 34.0), Sense::click());
    let hov = resp.hovered();
    let (bg, fg) = if hov {
        (pal.accent, Color32::from_rgb(0x14, 0x14, 0x14))
    } else {
        (pal.block, pal.onblock)
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    let x = rect.min.x + 16.0;
    text_at(ui, pos2(x, rect.center().y), Align2::LEFT_CENTER, text, Font::TextBold, 13.0, fg);
    text_at(
        ui,
        pos2(x + g1.size().x + 10.0, rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        key,
        Font::Mono,
        10.0,
        fg.gamma_multiply(0.55),
    );
    resp
}

/// 带下划线的文字链接。
pub fn link(ui: &mut Ui, pal: &Pal, text: &str) -> Response {
    let g = galley(ui, text, Font::Text, 13.0, pal.ink);
    let (rect, resp) = ui.allocate_exact_size(g.size() + vec2(0.0, 2.0), Sense::click());
    let color = if ui.is_enabled() { pal.ink } else { pal.faint };
    ui.painter().galley(rect.min, g, color);
    let under = if resp.hovered() { pal.ink } else { pal.faint };
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)],
        Stroke::new(1.0, under),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 方形开关，28×14。
pub fn switch(ui: &mut Ui, pal: &Pal, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(28.0, 14.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    let stroke = Stroke::new(1.0, if ui.is_enabled() { pal.ink } else { pal.faint });
    p.rect(
        rect,
        CornerRadius::ZERO,
        if *on { pal.ink } else { Color32::TRANSPARENT },
        stroke,
        StrokeKind::Inside,
    );
    let knob = Rect::from_min_size(pos2(rect.min.x + 3.0 + 14.0 * t, rect.min.y + 3.0), vec2(8.0, 8.0));
    p.rect_filled(knob, CornerRadius::ZERO, if *on { pal.paper } else { pal.ink });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 分段选择：黑框，选中的一格反色。
pub fn seg(ui: &mut Ui, pal: &Pal, options: &[String], selected: &mut usize) -> bool {
    let mut changed = false;
    let widths: Vec<f32> = options.iter().map(|o| galley(ui, o, Font::Text, 12.0, pal.ink).size().x + 24.0).collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(vec2(total, 26.0), Sense::hover());
    let mut x = rect.min.x;
    for (i, (o, w)) in options.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(*w, rect.height()));
        let resp = ui.interact(r, ui.scope_id().with(("seg", i, o)), Sense::click());
        let on = i == *selected;
        if on {
            ui.painter().rect_filled(r, CornerRadius::ZERO, pal.ink);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::ZERO, pal.hover());
        }
        text_at(ui, r.center(), Align2::CENTER_CENTER, o, Font::Text, 12.0, if on { pal.paper } else { pal.ink });
        if resp.clicked() && !on {
            *selected = i;
            changed = true;
        }
        x += w;
    }
    ui.painter()
        .rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, pal.ink), StrokeKind::Inside);
    changed
}

/// 页签：选中的加粗加 2px 下划线。返回点到的序号。
pub fn tabs(ui: &mut Ui, pal: &Pal, items: &[(String, bool)], selected: usize, big: bool) -> Option<usize> {
    let size = if big { 16.0 } else { 14.0 };
    let gap = if big { 28.0 } else { 22.0 };
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (i, (t, enabled)) in items.iter().enumerate() {
            let on = i == selected;
            let font = if on { Font::TextBold } else { Font::Text };
            let g = galley(ui, t, font, size, pal.ink);
            let (rect, resp) = ui.allocate_exact_size(
                g.size() + vec2(0.0, if big { 10.0 } else { 8.0 }),
                if *enabled { Sense::click() } else { Sense::hover() },
            );
            let color = if !enabled {
                pal.faint
            } else if on || resp.hovered() {
                pal.ink
            } else {
                pal.mute
            };
            ui.painter().galley(rect.min, g, color);
            if on {
                ui.painter()
                    .rect_filled(Rect::from_min_max(pos2(rect.min.x, rect.max.y - 2.0), rect.max), CornerRadius::ZERO, pal.ink);
            }
            if resp.clicked() && *enabled {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// 细线滑块：1px 轨道，10×10 方块。右边显示数值和单位。
pub fn range(ui: &mut Ui, pal: &Pal, id: &str, value: &mut f32, min: f32, max: f32, step: f32, unit: &str, width: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        // 总宽 = 轨道 + 66
        ui.spacing_mut().item_spacing.x = 0.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 14.0), Sense::click_and_drag());
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
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let p = ui.painter();
        p.line_segment(
            [pos2(rect.min.x, rect.center().y), pos2(rect.max.x, rect.center().y)],
            Stroke::new(1.0, pal.ink),
        );
        let x = rect.min.x + t * rect.width();
        p.rect_filled(Rect::from_center_size(pos2(x, rect.center().y), vec2(10.0, 10.0)), CornerRadius::ZERO, pal.ink);
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        ui.add_space(14.0);
        let txt = if step < 1.0 {
            format!("{:.2} {unit}", value)
        } else {
            format!("{} {unit}", value.round())
        };
        let (r, _) = ui.allocate_exact_size(vec2(52.0, 14.0), Sense::hover());
        text_at(ui, pos2(r.max.x, r.center().y), Align2::RIGHT_CENTER, txt.trim(), Font::NumBold, 12.0, pal.ink);
    });
    changed
}

/// 下拉选择：只有一条下划线。
pub fn select(ui: &mut Ui, pal: &Pal, id: &str, options: &[String], selected: &mut usize, width: f32) -> bool {
    let mut changed = false;
    let cur = options.get(*selected).cloned().unwrap_or_default();
    let resp = egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(rich(cur, Font::Text, 13.0, pal.ink))
        .show_ui(ui, |ui| {
            for (i, o) in options.iter().enumerate() {
                if ui.selectable_label(i == *selected, rich(o, Font::Text, 13.0, pal.ink)).clicked() {
                    *selected = i;
                    changed = true;
                }
            }
        });
    let r = resp.response.rect;
    ui.painter()
        .line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(1.0, pal.ink));
    changed
}

/// 区块标题：一个词加一条细线；右边可以放说明数值。
pub fn section(ui: &mut Ui, pal: &Pal, text: &str, right: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let r = text_at(ui, pos2(rect.min.x, rect.center().y), Align2::LEFT_CENTER, text, Font::TextBold, 13.0, pal.ink);
    let mut end = rect.max.x;
    if let Some(right) = right {
        let rr = text_at(ui, pos2(rect.max.x, rect.center().y), Align2::RIGHT_CENTER, right, Font::Text, 12.0, pal.mute);
        end = rr.min.x - 14.0;
    }
    ui.painter()
        .line_segment([pos2(r.max.x + 14.0, rect.center().y), pos2(end, rect.center().y)], Stroke::new(1.0, pal.line));
    ui.add_space(8.0);
}

/// 页面标题：大字、引线、右边的按钮。
pub fn page_header(ui: &mut Ui, pal: &Pal, title: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 20.0;
        let g = galley(ui, title, Font::TextBold, 30.0, pal.ink);
        let (r, _) = ui.allocate_exact_size(vec2(g.size().x, 40.0), Sense::hover());
        ui.painter().galley(pos2(r.min.x, r.center().y - g.size().y / 2.0), g, pal.ink);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            actions(ui);
            let w = ui.available_width() - 20.0;
            if w > 10.0 {
                let (lr, _) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
                let y = lr.center().y;
                ui.painter().line_segment([pos2(lr.min.x, y), pos2(lr.max.x, y)], Stroke::new(1.0, pal.line));
                ui.painter()
                    .rect_filled(Rect::from_min_size(pos2(lr.max.x - 5.0, y - 2.0), vec2(5.0, 5.0)), CornerRadius::ZERO, pal.ink);
            }
        });
    });
    ui.add_space(22.0);
}

/// 设置行：左边文字，右边控件，下面一条细线。
pub fn setting_row(ui: &mut Ui, pal: &Pal, label: &str, extra: Option<&str>, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width().min(640.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, 52.0), Sense::hover());
    let lr = text_at(ui, pos2(rect.min.x, rect.center().y), Align2::LEFT_CENTER, label, Font::Text, 14.0, pal.ink);
    if let Some(e) = extra {
        text_at(
            ui,
            pos2(lr.max.x + 10.0, rect.center().y + 1.0),
            Align2::LEFT_CENTER,
            e,
            Font::Text,
            11.0,
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
            ui.spacing_mut().item_spacing.x = 10.0;
            control(ui);
        },
    );
}

/// 紧凑的开关行（模块页的"显示内容"）。
pub fn toggle_row(ui: &mut Ui, pal: &Pal, label: &str, on: &mut bool) -> bool {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 33.0), Sense::hover());
    text_at(
        ui,
        pos2(rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        Font::Text,
        13.0,
        if *on { pal.ink } else { pal.faint },
    );
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)],
        Stroke::new(1.0, pal.line),
    );
    let sr = Rect::from_min_size(pos2(rect.max.x - 28.0, rect.center().y - 7.0), vec2(28.0, 14.0));
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
        changed = switch(ui, pal, on).changed();
    });
    changed
}

/// 热键框：等宽字，细框。
pub fn key_box(ui: &mut Ui, pal: &Pal, text: &str, active: bool) -> Response {
    let g = galley(ui, text, Font::Mono, 12.0, pal.ink);
    let (rect, resp) = ui.allocate_exact_size(vec2((g.size().x + 20.0).max(130.0), 26.0), Sense::click());
    let border = if active {
        pal.accent
    } else if resp.hovered() {
        pal.ink
    } else {
        pal.line
    };
    ui.painter().rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, border), StrokeKind::Inside);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, pal.ink);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// 只有下划线的输入框。
pub fn text_input(ui: &mut Ui, pal: &Pal, value: &mut String, hint: &str, width: f32) -> Response {
    let resp = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(rich(hint, Font::Text, 13.0, pal.faint))
            .font(Font::Text.id(13.0))
            .text_color(pal.ink)
            .frame(egui::Frame::NONE)
            .desired_width(width),
    );
    let r = resp.rect;
    ui.painter()
        .line_segment([pos2(r.min.x, r.max.y + 2.0), pos2(r.max.x, r.max.y + 2.0)], Stroke::new(1.0, pal.ink));
    resp
}

/// 黑底名牌。
pub fn tag(ui: &mut Ui, pal: &Pal, text: &str, size: f32) -> Response {
    let g = galley(ui, text, Font::TextBold, size, pal.onblock);
    let pad = vec2(size * 0.5, size * 0.2);
    let (rect, resp) = ui.allocate_exact_size(g.size() + pad * 2.0, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, pal.block);
    ui.painter().galley(rect.min + pad, g, pal.onblock);
    resp
}

/// 小菱形，用来标"使用中""当前"。
pub fn diamond(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    let h = size / 2.0;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![center + vec2(0.0, -h), center + vec2(h, 0.0), center + vec2(0.0, h), center + vec2(-h, 0.0)],
        color,
        Stroke::NONE,
    ));
}

/// 一行事实：上面小字标签，下面粗数字。
pub fn fact(ui: &mut Ui, pal: &Pal, label: &str, value: &str, big: f32, accent: bool) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        label_mute(ui, pal, label, 12.0);
        let font = if big >= 20.0 { Font::Num } else { Font::NumBold };
        self::label(ui, value, font, big, if accent { pal.accent } else { pal.ink });
    });
}

pub fn label_mute(ui: &mut Ui, pal: &Pal, text: &str, size: f32) -> Response {
    label(ui, text, Font::Text, size, pal.mute)
}

/// 一条水平细线。
pub fn hline(ui: &mut Ui, pal: &Pal) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, pal.line);
}

/// 进度：细线条。
pub fn thin_bar(ui: &mut Ui, color: Color32, track: Color32, ratio: f32, size: Vec2) {
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, track);
    let fill = Rect::from_min_size(r.min, vec2(r.width() * ratio.clamp(0.0, 1.0), r.height()));
    ui.painter().rect_filled(fill, CornerRadius::ZERO, color);
}
