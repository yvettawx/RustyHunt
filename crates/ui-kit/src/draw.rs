//! Basic pieces for drawing the overlay: shadowed text, name tags, bars, break pips, the crosshair marker, crowns.
//! Coordinates are design units (on a 1080-high reference frame), converted to the screen by [`Canvas`].

use std::sync::Arc;

use egui::epaint::{PathShape, PathStroke};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, Galley, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use crate::color::fade;
use crate::fonts::Font;
use crate::theme::{BarShape, BarStyle, PanelShape, Pattern, PipShape, TagShape, Theme};

#[derive(Clone)]
pub struct Canvas<'a> {
    pub painter: Painter,
    /// Where design coordinate (0, 0) is on screen
    pub origin: Pos2,
    /// Screen points per design unit
    pub scale: f32,
    pub opacity: f32,
    pub theme: &'a Theme,
}

/// One layer of a bar: paints `color` from `from` to `to` (0 to 1).
#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub from: f32,
    pub to: f32,
    pub color: Color32,
}

impl Layer {
    pub fn fill(ratio: f32, color: Color32) -> Layer {
        Layer {
            from: 0.0,
            to: ratio.clamp(0.0, 1.0),
            color,
        }
    }
}

impl<'a> Canvas<'a> {
    pub fn new(painter: Painter, origin: Pos2, scale: f32, theme: &'a Theme) -> Canvas<'a> {
        Canvas {
            painter,
            origin,
            scale,
            opacity: 1.0,
            theme,
        }
    }

    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    pub fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), vec2(w, h) * self.scale)
    }

    /// Converts a screen rect back to design coordinates.
    pub fn to_design(&self, r: Rect) -> Rect {
        Rect::from_min_max(
            pos2((r.min.x - self.origin.x) / self.scale, (r.min.y - self.origin.y) / self.scale),
            pos2((r.max.x - self.origin.x) / self.scale, (r.max.y - self.origin.y) / self.scale),
        )
    }

    pub fn faded(&self, alpha: f32) -> Canvas<'a> {
        Canvas {
            opacity: self.opacity * alpha,
            ..self.clone()
        }
    }

    /// A sub-canvas translated to (x, y) and scaled by `s`.
    pub fn sub(&self, x: f32, y: f32, s: f32) -> Canvas<'a> {
        Canvas {
            origin: self.p(x, y),
            scale: self.scale * s,
            ..self.clone()
        }
    }

    pub fn c(&self, c: Color32) -> Color32 {
        if self.opacity >= 0.999 { c } else { fade(c, self.opacity) }
    }

    pub fn tok(&self, name: &str) -> Color32 {
        self.theme.color(name)
    }

    pub fn galley(&self, text: &str, font: Font, size: f32, color: Color32, spacing: f32) -> Arc<Galley> {
        let mut job = LayoutJob::default();
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.id((size * self.scale).max(1.0)),
                color,
                extra_letter_spacing: spacing * size * self.scale,
                ..Default::default()
            },
        );
        self.painter.layout_job(job)
    }

    /// Text size, in design units.
    pub fn measure(&self, text: &str, font: Font, size: f32, spacing: f32) -> Vec2 {
        if text.is_empty() {
            return Vec2::ZERO;
        }
        self.galley(text, font, size, Color32::WHITE, spacing).size() / self.scale
    }

    /// Truncates from the end with an ellipsis when it doesn't fit.
    pub fn fit(&self, text: &str, font: Font, size: f32, max_w: f32) -> String {
        if self.measure(text, font, size, 0.0).x <= max_w {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        for n in (1..chars.len()).rev() {
            let s: String = chars[..n].iter().collect::<String>() + "…";
            if self.measure(&s, font, size, 0.0).x <= max_w {
                return s;
            }
        }
        "…".into()
    }

    /// Shadowed text. Returns its size in design units.
    pub fn text(&self, x: f32, y: f32, text: &str, font: Font, size: f32, color: Color32, align: Align2) -> Vec2 {
        self.text_ex(x, y, text, font, size, color, align, 0.0, true)
    }

    pub fn text_ex(&self, x: f32, y: f32, text: &str, font: Font, size: f32, color: Color32, align: Align2, spacing: f32, shadow: bool) -> Vec2 {
        if text.is_empty() {
            return Vec2::ZERO;
        }
        let g = self.galley(text, font, size, Color32::WHITE, spacing);
        let sz = g.size();
        let pos = align.anchor_size(self.p(x, y), sz).min;
        let strength = self.theme.styles.text_shadow * self.opacity;
        if shadow && strength > 0.0 {
            let sh = self.tok("shadow");
            let d = (self.scale * 1.0).max(0.6);
            for (dx, dy, a) in [(-d, 0.0, 0.5), (d, 0.0, 0.5), (0.0, -d, 0.5), (0.0, d, 0.6), (0.0, d * 1.6, 0.35)] {
                self.painter
                    .galley_with_override_text_color(pos + vec2(dx, dy), g.clone(), fade(sh, a * strength));
            }
        }
        self.painter.galley_with_override_text_color(pos, g, self.c(color));
        sz / self.scale
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        self.painter.rect_filled(self.r(x, y, w, h), CornerRadius::ZERO, self.c(color));
    }

    pub fn line(&self, a: (f32, f32), b: (f32, f32), width: f32, color: Color32) {
        self.painter
            .line_segment([self.p(a.0, a.1), self.p(b.0, b.1)], Stroke::new(width * self.scale, self.c(color)));
    }

    /// A thin line with a little shadow, for the line after module titles and the end caps of bars.
    pub fn rule(&self, x0: f32, x1: f32, y: f32, color: Color32) {
        self.line((x0, y + 0.5), (x1, y + 0.5), 1.0, fade(self.tok("shadow"), 0.4));
        self.line((x0, y), (x1, y), 1.0, color);
    }

    /// Black name tag. Returns its size.
    pub fn tag(&self, x: f32, y: f32, text: &str, font: Font, size: f32, pad: (f32, f32, f32, f32), bg: Color32, fg: Color32) -> Vec2 {
        let t = self.measure(text, font, size, 0.06);
        let w = t.x + pad.0 + pad.2;
        let h = t.y + pad.1 + pad.3;
        self.shape_box(x, y, w, h, bg);
        self.text_ex(x + pad.0, y + pad.1, text, font, size, fg, Align2::LEFT_TOP, 0.06, false);
        vec2(w, h)
    }

    pub fn tag_size(&self, text: &str, font: Font, size: f32, pad: (f32, f32, f32, f32)) -> Vec2 {
        let t = self.measure(text, font, size, 0.06);
        vec2(t.x + pad.0 + pad.2, t.y + pad.1 + pad.3)
    }

    /// Fills a box in the theme's name tag shape.
    pub fn shape_box(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        let st = &self.theme.styles.tag;
        match st.shape {
            TagShape::Rect => self.rect(x, y, w, h, color),
            TagShape::Round => {
                let r = (st.radius * self.scale).min(h * self.scale / 2.0);
                self.painter.rect_filled(self.r(x, y, w, h), CornerRadius::same(r as u8), self.c(color));
            }
            TagShape::Cut => {
                let c = st.cut.min(h / 2.0);
                let pts = vec![
                    self.p(x + c, y),
                    self.p(x + w, y),
                    self.p(x + w, y + h - c),
                    self.p(x + w - c, y + h),
                    self.p(x, y + h),
                    self.p(x, y + c),
                ];
                self.painter.add(Shape::convex_polygon(pts, self.c(color), Stroke::NONE));
            }
        }
    }

    /// Module backing: not drawn by default (unobtrusive); themes can add one.
    pub fn panel(&self, x: f32, y: f32, w: f32, h: f32) {
        let p = &self.theme.styles.panel;
        let (x, y, w, h) = (x - p.padding, y - p.padding, w + p.padding * 2.0, h + p.padding * 2.0);
        let stroke = Stroke::new(1.0, self.c(p.stroke));
        match p.shape {
            PanelShape::None => {}
            PanelShape::Rect => {
                self.painter
                    .rect(self.r(x, y, w, h), CornerRadius::ZERO, self.c(p.fill), stroke, egui::StrokeKind::Inside);
            }
            PanelShape::Round => {
                self.painter.rect(
                    self.r(x, y, w, h),
                    CornerRadius::same((p.radius * self.scale) as u8),
                    self.c(p.fill),
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
            PanelShape::Cut => {
                let c = p.cut;
                let pts = vec![
                    self.p(x + c, y),
                    self.p(x + w, y),
                    self.p(x + w, y + h - c),
                    self.p(x + w - c, y + h),
                    self.p(x, y + h),
                    self.p(x, y + c),
                ];
                self.painter.add(Shape::convex_polygon(pts, self.c(p.fill), stroke));
            }
        }
    }

    fn bar_outline(&self, style: &BarStyle, x: f32, y: f32, w: f32, h: f32) -> Vec<Pos2> {
        match style.shape {
            BarShape::Parallelogram => {
                let s = style.skew.min(w / 2.0);
                vec![self.p(x + s, y), self.p(x + w, y), self.p(x + w - s, y + h), self.p(x, y + h)]
            }
            _ => vec![self.p(x, y), self.p(x + w, y), self.p(x + w, y + h), self.p(x, y + h)],
        }
    }

    fn fill_shape(&self, painter: &Painter, style: &BarStyle, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        match style.shape {
            BarShape::Round => {
                let r = (style.radius.max(h / 2.0) * self.scale).min(h * self.scale / 2.0);
                painter.rect_filled(self.r(x, y, w, h), CornerRadius::same(r.round() as u8), color);
            }
            BarShape::Rect if style.radius > 0.0 => {
                painter.rect_filled(self.r(x, y, w, h), CornerRadius::same((style.radius * self.scale) as u8), color);
            }
            _ => {
                painter.add(Shape::convex_polygon(self.bar_outline(style, x, y, w, h), color, Stroke::NONE));
            }
        }
    }

    fn pattern(&self, painter: &Painter, pattern: Pattern, style: &BarStyle, area: Rect, color: Color32) {
        let step = style.pattern_density * self.scale;
        let stroke = Stroke::new(self.scale.max(0.6), color);
        match pattern {
            Pattern::None => {}
            Pattern::Hatch => {
                let h = area.height();
                let tan = style.pattern_angle.to_radians().tan().abs().max(0.2);
                let run = h / tan;
                let mut x = area.min.x - run;
                while x < area.max.x {
                    painter.line_segment([pos2(x, area.max.y), pos2(x + run, area.min.y)], stroke);
                    x += step;
                }
            }
            Pattern::Dots => {
                let mut y = area.min.y + step / 2.0;
                while y < area.max.y {
                    let mut x = area.min.x + step / 2.0;
                    while x < area.max.x {
                        painter.circle_filled(pos2(x, y), self.scale * 0.7, color);
                        x += step;
                    }
                    y += step;
                }
            }
            Pattern::Grid => {
                let mut x = area.min.x;
                while x < area.max.x {
                    painter.line_segment([pos2(x, area.min.y), pos2(x, area.max.y)], stroke);
                    x += step;
                }
                let mut y = area.min.y;
                while y < area.max.y {
                    painter.line_segment([pos2(area.min.x, y), pos2(area.max.x, y)], stroke);
                    y += step;
                }
            }
        }
    }

    /// Draws a bar in the theme's style. `kind` picks the `<bar for="...">` override;
    /// `hatch` and `caps` are this bar's default look (hatched track, end caps),
    /// and a theme style written for this kind of bar takes precedence.
    pub fn bar(&self, kind: &str, x: f32, y: f32, w: f32, h: f32, layers: &[Layer], hatch: bool) {
        self.bar_ex(kind, x, y, w, h, layers, hatch, false);
    }

    pub fn bar_ex(&self, kind: &str, x: f32, y: f32, w: f32, h: f32, layers: &[Layer], hatch: bool, caps: bool) {
        let explicit = self.theme.styles.bars.get(kind);
        let style = explicit.unwrap_or(&self.theme.styles.bar);
        let (hatch, caps) = if explicit.is_some() { (false, false) } else { (hatch, caps) };
        let h = h * style.height;
        let segs: Vec<(f32, f32)> = if style.segments > 1 {
            let n = style.segments as f32;
            let sw = (w - style.gap * (n - 1.0)) / n;
            (0..style.segments).map(|i| (x + i as f32 * (sw + style.gap), sw)).collect()
        } else {
            vec![(x, w)]
        };
        let track = self.c(self.tok("track"));
        let edge = self.c(self.tok("edge"));
        let pattern = if style.empty != Pattern::None {
            style.empty
        } else if hatch {
            Pattern::Hatch
        } else {
            Pattern::None
        };

        if style.glow > 0.0 {
            let accent = layers.last().map(|l| l.color).unwrap_or(self.tok("accent"));
            for i in 1..=3 {
                let g = style.glow * i as f32 / 3.0;
                self.fill_shape(&self.painter, style, x - g, y - g, w + g * 2.0, h + g * 2.0, fade(self.c(accent), 0.08));
            }
        }

        for &(sx, sw) in &segs {
            if style.edge {
                let e = 1.0 / self.scale;
                self.fill_shape(&self.painter, style, sx - e, y - e, sw + 2.0 * e, h + 2.0 * e, edge);
            }
            self.fill_shape(&self.painter, style, sx, y, sw, h, track);
            if pattern != Pattern::None {
                let clip = self.painter.with_clip_rect(self.r(sx, y, sw, h).intersect(self.painter.clip_rect()));
                self.pattern(&clip, pattern, style, self.r(sx, y, sw, h), self.c(self.tok("hatch")));
            }
            for l in layers {
                if l.to <= l.from {
                    continue;
                }
                let lx0 = x + w * l.from;
                let lx1 = x + w * l.to;
                let x0 = lx0.max(sx);
                let x1 = lx1.min(sx + sw);
                if x1 <= x0 {
                    continue;
                }
                let band = Rect::from_min_max(self.p(x0, y - 1.0), self.p(x1, y + h + 1.0));
                let clip = self.painter.with_clip_rect(band.intersect(self.painter.clip_rect()));
                self.fill_shape(&clip, style, sx, y, sw, h, self.c(l.color));
            }
        }

        if style.caps || caps {
            self.caps(x, y, w, h);
        }
    }

    /// Small vertical caps at both ends of a bar.
    pub fn caps(&self, x: f32, y: f32, w: f32, h: f32) {
        let c = self.tok("text");
        for cx in [x - 5.0, x + w + 5.0] {
            self.line((cx + 0.6, y - 3.0), (cx + 0.6, y + h + 3.0), 2.5, fade(self.tok("shadow"), 0.5));
            self.line((cx, y - 3.0), (cx, y + h + 3.0), 1.5, c);
        }
    }

    /// Break pips: `n` of them, the first `filled` solid. Pass `right_align` to lay them out right to left.
    pub fn pips(&self, x: f32, y: f32, n: i32, filled: i32, size: f32, gap: f32, color: Color32) -> f32 {
        let shape = self.theme.styles.pip;
        for i in 0..n.max(0) {
            let px = x + i as f32 * (size + gap);
            let r = self.r(px, y, size, size);
            let on = i < filled;
            let stroke = Stroke::new(self.scale.max(0.8), self.c(color));
            let fill = if on { self.c(color) } else { Color32::TRANSPARENT };
            match shape {
                PipShape::Square => {
                    self.painter.rect(r, CornerRadius::ZERO, fill, stroke, egui::StrokeKind::Inside);
                }
                PipShape::Circle => {
                    self.painter.circle(r.center(), size * self.scale / 2.0, fill, stroke);
                }
                PipShape::Diamond => self.diamond(px + size / 2.0, y + size / 2.0, size, fill, color),
            }
        }
        n.max(0) as f32 * (size + gap) - if n > 0 { gap } else { 0.0 }
    }

    pub fn diamond(&self, cx: f32, cy: f32, size: f32, fill: Color32, stroke: Color32) {
        let h = size / 2.0;
        let pts = vec![self.p(cx, cy - h), self.p(cx + h, cy), self.p(cx, cy + h), self.p(cx - h, cy)];
        self.painter
            .add(Shape::convex_polygon(pts, self.c(fill), Stroke::new(self.scale.max(0.8), self.c(stroke))));
    }

    pub fn dot(&self, cx: f32, cy: f32, r: f32, color: Color32, glow: bool) {
        if glow {
            self.painter.circle_filled(self.p(cx, cy), r * 2.2 * self.scale, fade(self.c(color), 0.18));
        }
        self.painter.circle_filled(self.p(cx, cy), r * self.scale, self.c(color));
    }

    /// Crosshair marker (Rhine style), translucent.
    pub fn crosshair(&self, x: f32, y: f32) {
        let c = fade(self.tok("text"), 0.45);
        self.line((x + 4.5, y), (x + 4.5, y + 9.0), 1.0, c);
        self.line((x, y + 4.5), (x + 9.0, y + 4.5), 1.0, c);
    }

    /// Crown icon, 16x12 design units.
    pub fn crown(&self, x: f32, y: f32, w: f32, gold: bool) {
        let s = w / 16.0;
        let pts: Vec<(f32, f32)> = vec![(1.0, 10.0), (2.0, 3.0), (5.5, 6.5), (8.0, 1.0), (10.5, 6.5), (14.0, 3.0), (15.0, 10.0)];
        let fill = if gold { self.tok("crown-gold") } else { self.tok("crown-silver") };
        let poly: Vec<Pos2> = pts.iter().map(|(px, py)| self.p(x + px * s, y + py * s)).collect();
        self.painter.add(Shape::Path(PathShape {
            points: poly,
            closed: true,
            fill: self.c(fill),
            stroke: PathStroke::new(self.scale.max(0.8), self.c(Color32::from_rgb(0x11, 0x11, 0x11))),
        }));
    }

    pub fn polyline(&self, pts: &[(f32, f32)], width: f32, color: Color32) {
        if pts.len() < 2 {
            return;
        }
        let shadow: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y + 0.8)).collect();
        self.painter
            .add(Shape::line(shadow, Stroke::new(width * self.scale * 1.8, fade(self.tok("shadow"), 0.35))));
        let p: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::line(p, Stroke::new(width * self.scale, self.c(color))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_fill_clamps() {
        let l = Layer::fill(1.5, Color32::WHITE);
        assert_eq!(l.to, 1.0);
        let l = Layer::fill(-0.5, Color32::WHITE);
        assert_eq!(l.to, 0.0);
    }
}
