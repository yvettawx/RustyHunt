//! 按模板画模块：先量尺寸，再按位置画。数据是 [`crate::fields`] 生成的 JSON。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use egui::epaint::{Mesh, PathShape, PathStroke, TextShape};
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Shape, Stroke, TextureHandle, Vec2, pos2, vec2};
use serde_json::Value;

use crate::color::fade;
use crate::draw::{Canvas, Layer};
use crate::expr::{self, Expr, Scope, as_f64, to_text, truthy};
use crate::fonts::Font;
use crate::template::{Animation, Kind, Node, Template};
use crate::theme::{BarShape, Pattern, resolve_color};

/// 模板用到的图片，按需载入成纹理。
#[derive(Default)]
pub struct Images {
    dir: Option<PathBuf>,
    loaded: HashMap<(String, u32, u32), Option<TextureHandle>>,
}

impl Images {
    pub fn new(dir: Option<PathBuf>) -> Images {
        Images {
            dir,
            loaded: HashMap::new(),
        }
    }

    pub fn set_dir(&mut self, dir: Option<PathBuf>) {
        if self.dir != dir {
            self.dir = dir;
            self.loaded.clear();
        }
    }

    fn get(&mut self, ctx: &egui::Context, src: &str, w: u32, h: u32) -> Option<(TextureHandle, Vec2)> {
        let dir = self.dir.clone()?;
        if src.contains("..") {
            return None;
        }
        let key = (src.to_string(), w, h);
        if !self.loaded.contains_key(&key) {
            let tex = load_image(ctx, &dir.join(src), w, h);
            self.loaded.insert(key.clone(), tex);
        }
        let t = self.loaded.get(&key)?.clone()?;
        let size = t.size_vec2();
        Some((t, size))
    }
}

fn load_image(ctx: &egui::Context, path: &Path, w: u32, h: u32) -> Option<TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let image = if ext == "svg" {
        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default()).ok()?;
        let s = tree.size();
        let (tw, th) = if w > 0 && h > 0 { (w, h) } else { (s.width().ceil() as u32, s.height().ceil() as u32) };
        let (tw, th) = (tw.clamp(1, crate::theme::MAX_IMAGE_SIDE), th.clamp(1, crate::theme::MAX_IMAGE_SIDE));
        let mut pix = resvg::tiny_skia::Pixmap::new(tw, th)?;
        let tf = resvg::tiny_skia::Transform::from_scale(tw as f32 / s.width(), th as f32 / s.height());
        resvg::render(&tree, tf, &mut pix.as_mut());
        egui::ColorImage::from_rgba_premultiplied([tw as usize, th as usize], pix.data())
    } else {
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        if img.width() > crate::theme::MAX_IMAGE_SIDE || img.height() > crate::theme::MAX_IMAGE_SIDE {
            log::warn!("{} 超过 {} 像素，跳过", path.display(), crate::theme::MAX_IMAGE_SIDE);
            return None;
        }
        egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
    };
    Some(ctx.load_texture(path.to_string_lossy(), image, egui::TextureOptions::LINEAR))
}

struct Env<'a> {
    root: &'a Value,
    vars: Vec<(String, Value)>,
}

impl Scope for Env<'_> {
    fn get(&self, name: &str) -> Option<&Value> {
        self.vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
            .or_else(|| self.root.get(name))
    }
}

/// 节点在这一帧的有效属性：基础属性加上成立的 state。
struct Eff {
    vals: BTreeMap<String, Value>,
    anim_alpha: f32,
    anim_dx: f32,
    anim_scale: f32,
}

impl Eff {
    fn num(&self, k: &str, d: f32) -> f32 {
        self.vals.get(k).map_or(d, |v| {
            if v.is_null() {
                d
            } else {
                let s = to_text(v);
                s.trim_end_matches("px").trim_end_matches('%').parse::<f32>().unwrap_or(as_f64(v) as f32)
            }
        })
    }

    fn text(&self, k: &str) -> String {
        self.vals.get(k).map(to_text).unwrap_or_default()
    }

    fn has(&self, k: &str) -> bool {
        self.vals.contains_key(k)
    }

    fn flag(&self, k: &str, d: bool) -> bool {
        self.vals.get(k).map_or(d, |v| match v {
            Value::String(s) => !matches!(s.as_str(), "false" | "0" | "no" | ""),
            other => truthy(other),
        })
    }
}

pub struct Renderer<'a, 'c> {
    pub canvas: &'a Canvas<'c>,
    pub images: &'a mut Images,
    pub ctx: egui::Context,
    /// 动画用的时间（秒）
    pub time: f64,
    /// 本帧用到了动画，需要继续重绘
    pub animating: bool,
}

impl Renderer<'_, '_> {
    fn eff(&mut self, n: &Node, env: &Env) -> Eff {
        let mut vals: BTreeMap<String, Value> = n.attrs.iter().map(|(k, t)| (k.clone(), t.eval(env))).collect();
        let mut anim_alpha = 1.0;
        let mut anim_dx = 0.0;
        let mut anim_scale = 1.0;
        for s in &n.states {
            if !truthy(&expr::eval(&s.when, env)) {
                continue;
            }
            for (k, t) in &s.attrs {
                vals.insert(k.clone(), t.eval(env));
            }
            if let Some(a) = s.animate {
                self.animating = true;
                let ph = (self.time as f32 / s.period * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                match a {
                    Animation::Blink => anim_alpha *= if ph > 0.5 { 1.0 } else { 0.45 },
                    Animation::Breathe => anim_alpha *= 0.55 + 0.45 * ph,
                    Animation::Pulse => anim_scale *= 1.0 + 0.04 * ph,
                    Animation::Shift => anim_dx += 3.0 * (ph - 0.5),
                }
            }
        }
        Eff {
            vals,
            anim_alpha,
            anim_dx,
            anim_scale,
        }
    }

    fn color(&self, e: &Eff, k: &str, default: &str) -> Color32 {
        let v = e.text(k);
        let v = if v.is_empty() { default.to_string() } else { v };
        resolve_color(&v, &self.canvas.theme.tokens).unwrap_or(Color32::MAGENTA)
    }

    fn font(&self, e: &Eff, default: Font) -> Font {
        let f = e.text("font");
        if f.is_empty() {
            return default;
        }
        match self.canvas.theme.tokens.fonts.get(&f) {
            Some(t) => Font::from_token(&t.family, t.weight),
            None => Font::from_token(&f, 700),
        }
    }

    fn items(&self, list: &Expr, env: &Env, limit: usize) -> Vec<Value> {
        match expr::eval(list, env) {
            Value::Array(a) => a.into_iter().take(limit).collect(),
            _ => Vec::new(),
        }
    }

    /// 节点的尺寸（设计单位，未缩放）。
    fn measure(&mut self, n: &Node, env: &mut Env) -> Vec2 {
        let e = self.eff(n, env);
        if !e.flag("visible", true) {
            return Vec2::ZERO;
        }
        let s = e.num("scale", 1.0) * e.anim_scale;
        let size = match n.kind {
            Kind::Column | Kind::Row | Kind::Stack | Kind::If => {
                if n.kind == Kind::If && !e.flag("when", false) {
                    return Vec2::ZERO;
                }
                let sizes: Vec<(Vec2, Vec2)> = n
                    .children
                    .iter()
                    .map(|c| {
                        let off = vec2(self.eff(c, env).num("x", 0.0), self.eff(c, env).num("y", 0.0));
                        (self.measure(c, env), off)
                    })
                    .collect();
                stack_size(n.kind, &sizes, e.num("gap", 0.0), e.num("padding", 0.0))
            }
            Kind::For => {
                let (var, list) = n.each.as_ref().expect("for 有 each");
                let limit = (e.num("limit", crate::template::MAX_ITEMS as f32) as usize).min(crate::template::MAX_ITEMS);
                let items = self.items(list, env, limit);
                let mut sizes = Vec::new();
                for item in items {
                    env.vars.push((var.clone(), item));
                    for c in &n.children {
                        sizes.push((self.measure(c, env), Vec2::ZERO));
                    }
                    env.vars.pop();
                }
                let kind = if e.text("direction") == "row" { Kind::Row } else { Kind::Column };
                stack_size(kind, &sizes, e.num("gap", 0.0), 0.0)
            }
            Kind::Text => {
                let t = e.text("text");
                let sz = self.canvas.measure(&t, self.font(&e, Font::Text), e.num("size", 12.0), e.num("spacing", 0.0));
                vec2(e.num("width", sz.x), e.num("height", sz.y))
            }
            Kind::Tag => {
                let p = e.num("padding", 4.0);
                self.canvas
                    .tag_size(&e.text("text"), self.font(&e, Font::TextBold), e.num("size", 12.0), (p * 2.0, p * 0.75, p * 2.0, p * 0.5))
            }
            Kind::Bar => vec2(e.num("width", 100.0), e.num("height", 6.0)),
            Kind::Arc => {
                let r = e.num("radius", 20.0) + e.num("thickness", 3.0) / 2.0;
                vec2(r * 2.0, r * 2.0)
            }
            Kind::Circle => {
                let r = e.num("radius", 4.0);
                vec2(r * 2.0, r * 2.0)
            }
            Kind::Polygon => {
                let pts = parse_points(&e.text("points"));
                let b = bounds(&pts);
                vec2(e.num("width", b.x), e.num("height", b.y))
            }
            Kind::Path => {
                let subs = flatten_path(&e.text("d"));
                let b = bounds(&subs.iter().flat_map(|(p, _)| p.iter().copied()).collect::<Vec<_>>());
                vec2(e.num("width", b.x), e.num("height", b.y))
            }
            Kind::Image => {
                let (w, h) = (e.num("width", 0.0), e.num("height", 0.0));
                if w > 0.0 && h > 0.0 {
                    vec2(w, h)
                } else {
                    let src = e.text("src");
                    self.images
                        .get(&self.ctx, &src, 0, 0)
                        .map(|(_, s)| vec2(if w > 0.0 { w } else { s.x }, if h > 0.0 { h } else { s.y }))
                        .unwrap_or(Vec2::ZERO)
                }
            }
            Kind::Pips => {
                let n = e.num("count", 0.0).max(0.0);
                let size = e.num("size", 6.0);
                let gap = e.num("gap", 3.0);
                vec2((n * (size + gap) - gap).max(0.0), size)
            }
            Kind::Line => vec2(e.num("x1", 0.0).max(e.num("x2", 0.0)), e.num("y1", 0.0).max(e.num("y2", 0.0))),
            Kind::Spacer | Kind::Rect => vec2(e.num("width", 0.0), e.num("height", 0.0)),
        };
        size * s
    }

    fn paint(&mut self, n: &Node, env: &mut Env, x: f32, y: f32, alpha: f32) {
        let e = self.eff(n, env);
        if !e.flag("visible", true) {
            return;
        }
        let a = alpha * e.num("opacity", 1.0).clamp(0.0, 1.0) * e.anim_alpha;
        if a <= 0.003 {
            return;
        }
        let s = e.num("scale", 1.0) * e.anim_scale;
        let x = x + e.anim_dx;
        let base = self.canvas.faded(a);
        let cv = if (s - 1.0).abs() > 1e-3 { base.sub(x, y, s) } else { base.sub(x, y, 1.0) };
        let cv = &cv;

        match n.kind {
            Kind::Column | Kind::Row | Kind::Stack | Kind::If => {
                if n.kind == Kind::If && !e.flag("when", false) {
                    return;
                }
                let gap = e.num("gap", 0.0);
                let pad = e.num("padding", 0.0);
                let align = e.text("align");
                let sizes: Vec<Vec2> = n.children.iter().map(|c| self.measure(c, env) / 1.0).collect();
                let total = stack_size(n.kind, &sizes.iter().map(|s| (*s, Vec2::ZERO)).collect::<Vec<_>>(), gap, pad);
                let mut cursor = pad;
                for (c, sz) in n.children.iter().zip(sizes) {
                    let ce = self.eff(c, env);
                    let (ox, oy) = (ce.num("x", 0.0), ce.num("y", 0.0));
                    let (cx, cy) = match n.kind {
                        Kind::Row => (cursor, pad + cross(&align, total.y - pad * 2.0, sz.y)),
                        Kind::Stack => (pad, pad),
                        _ => (pad + cross(&align, total.x - pad * 2.0, sz.x), cursor),
                    };
                    let mut sub = Renderer {
                        canvas: cv,
                        images: self.images,
                        ctx: self.ctx.clone(),
                        time: self.time,
                        animating: false,
                    };
                    sub.paint(c, env, cx + ox, cy + oy, 1.0);
                    self.animating |= sub.animating;
                    match n.kind {
                        Kind::Row => cursor += sz.x + gap,
                        Kind::Stack => {}
                        _ => cursor += sz.y + gap,
                    }
                }
            }
            Kind::For => {
                let (var, list) = n.each.as_ref().expect("for 有 each");
                let limit = (e.num("limit", crate::template::MAX_ITEMS as f32) as usize).min(crate::template::MAX_ITEMS);
                let items = self.items(list, env, limit);
                let row = e.text("direction") == "row";
                let gap = e.num("gap", 0.0);
                let mut cursor = 0.0;
                for (i, item) in items.into_iter().enumerate() {
                    env.vars.push((var.clone(), item));
                    env.vars.push((format!("{var}_index"), Value::from(i)));
                    for c in &n.children {
                        let sz = self.measure(c, env);
                        let mut sub = Renderer {
                            canvas: cv,
                            images: self.images,
                            ctx: self.ctx.clone(),
                            time: self.time,
                            animating: false,
                        };
                        if row {
                            sub.paint(c, env, cursor, 0.0, 1.0);
                            cursor += sz.x + gap;
                        } else {
                            sub.paint(c, env, 0.0, cursor, 1.0);
                            cursor += sz.y + gap;
                        }
                        self.animating |= sub.animating;
                    }
                    env.vars.pop();
                    env.vars.pop();
                }
            }
            Kind::Text => {
                let t = e.text("text");
                let font = self.font(&e, Font::Text);
                let size = e.num("size", 12.0);
                let w = e.num("width", 0.0);
                let align = match e.text("align").as_str() {
                    "center" => Align2::CENTER_TOP,
                    "right" | "end" => Align2::RIGHT_TOP,
                    _ => Align2::LEFT_TOP,
                };
                let ax = if w > 0.0 {
                    match align {
                        Align2::CENTER_TOP => w / 2.0,
                        Align2::RIGHT_TOP => w,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };
                let color = self.color(&e, "color", "text");
                let rot = e.num("rotate", 0.0);
                if rot.abs() > 0.01 {
                    let g = cv.galley(&t, font, size, color, e.num("spacing", 0.0));
                    let shape = TextShape::new(cv.p(ax, 0.0), g, cv.c(color)).with_angle(rot.to_radians());
                    cv.painter.add(shape);
                } else {
                    cv.text_ex(ax, 0.0, &t, font, size, color, align, e.num("spacing", 0.0), e.flag("shadow", true));
                }
            }
            Kind::Tag => {
                let p = e.num("padding", 4.0);
                cv.tag(
                    0.0,
                    0.0,
                    &e.text("text"),
                    self.font(&e, Font::TextBold),
                    e.num("size", 12.0),
                    (p * 2.0, p * 0.75, p * 2.0, p * 0.5),
                    self.color(&e, "fill", "ink"),
                    self.color(&e, "color", "text"),
                );
            }
            Kind::Bar => {
                let w = e.num("width", 100.0);
                let h = e.num("height", 6.0);
                let mut layers = Vec::new();
                if e.has("ghost") {
                    layers.push(Layer::fill(e.num("ghost", 0.0), self.color(&e, "ghost-fill", "ghost")));
                }
                layers.push(Layer::fill(e.num("value", 0.0), self.color(&e, "fill", "text")));
                let kind = e.text("kind");
                let mut theme = cv.theme.clone();
                {
                    let st = theme.styles.bars.entry(kind.clone()).or_insert_with(|| cv.theme.styles.bar(&kind).clone());
                    if let Some(sh) = e.vals.get("shape").map(to_text) {
                        st.shape = match sh.as_str() {
                            "round" => BarShape::Round,
                            "parallelogram" => BarShape::Parallelogram,
                            _ => BarShape::Rect,
                        };
                    }
                    if e.has("segments") {
                        st.segments = e.num("segments", 0.0) as u32;
                    }
                    if e.has("gap") {
                        st.gap = e.num("gap", 2.0);
                    }
                    if e.has("skew") {
                        st.skew = e.num("skew", 0.0);
                    }
                    if e.has("radius") {
                        st.radius = e.num("radius", 0.0);
                    }
                    if e.has("caps") {
                        st.caps = e.flag("caps", false);
                    }
                    if e.has("edge") {
                        st.edge = e.flag("edge", true);
                    }
                    if e.has("glow") {
                        st.glow = e.num("glow", 0.0);
                    }
                    if let Some(p) = e.vals.get("empty").map(to_text) {
                        st.empty = match p.as_str() {
                            "hatch" => Pattern::Hatch,
                            "dots" => Pattern::Dots,
                            "grid" => Pattern::Grid,
                            _ => Pattern::None,
                        };
                    }
                }
                if e.has("track")
                    && let Some(c) = resolve_color(&e.text("track"), &cv.theme.tokens)
                {
                    theme.tokens.colors.insert("track".into(), c);
                }
                let tc = Canvas {
                    theme: &theme,
                    ..cv.clone()
                };
                tc.bar(&kind, 0.0, 0.0, w, h, &layers, false);
                if e.has("marker") {
                    let mx = w * e.num("marker", 0.0).clamp(0.0, 1.0);
                    tc.line((mx, -4.0), (mx, h + 4.0), 2.0, self.color(&e, "marker-color", "accent"));
                }
            }
            Kind::Arc => {
                let r = e.num("radius", 20.0);
                let th = e.num("thickness", 3.0);
                let c = vec2(r + th / 2.0, r + th / 2.0);
                let from = e.num("from", 0.0);
                let to = e.num("to", 360.0);
                let v = e.num("value", 1.0).clamp(0.0, 1.0);
                let rot = e.num("rotate", 0.0);
                if e.has("track") {
                    let col = self.color(&e, "track", "track");
                    arc(cv, c, r, from + rot, to + rot, th, col);
                }
                let col = self.color(&e, "stroke", "text");
                let glow = e.num("glow", 0.0);
                if glow > 0.0 {
                    for i in 1..=3 {
                        arc(cv, c, r, from + rot, from + rot + (to - from) * v, th + glow * i as f32, fade(col, 0.1));
                    }
                }
                arc(cv, c, r, from + rot, from + rot + (to - from) * v, th, col);
            }
            Kind::Rect => {
                let (w, h) = (e.num("width", 0.0), e.num("height", 0.0));
                let fill = if e.has("fill") { self.color(&e, "fill", "text") } else { Color32::TRANSPARENT };
                let stroke = if e.has("stroke") {
                    Stroke::new(e.num("stroke-width", 1.0) * cv.scale, cv.c(self.color(&e, "stroke", "text")))
                } else {
                    Stroke::NONE
                };
                let skew = e.num("skew", 0.0);
                let cut = e.num("cut", 0.0);
                let radius = e.num("radius", 0.0);
                let glow = e.num("glow", 0.0);
                let mut pts: Vec<(f32, f32)> = if cut > 0.0 {
                    vec![(cut, 0.0), (w, 0.0), (w, h - cut), (w - cut, h), (0.0, h), (0.0, cut)]
                } else {
                    vec![(skew, 0.0), (w, 0.0), (w - skew, h), (0.0, h)]
                };
                rotate_points(&mut pts, e.num("rotate", 0.0), (w / 2.0, h / 2.0));
                flip_points(&mut pts, &e.text("flip"), (w, h));
                if glow > 0.0 {
                    for i in 1..=3 {
                        let g = glow * i as f32 / 3.0;
                        cv.painter.rect_filled(cv.r(-g, -g, w + 2.0 * g, h + 2.0 * g), CornerRadius::ZERO, fade(cv.c(fill), 0.08));
                    }
                }
                if radius > 0.0 && skew == 0.0 && cut == 0.0 {
                    cv.painter.rect(
                        cv.r(0.0, 0.0, w, h),
                        CornerRadius::same((radius * cv.scale) as u8),
                        cv.c(fill),
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                } else {
                    let p: Vec<Pos2> = pts.iter().map(|&(px, py)| cv.p(px, py)).collect();
                    cv.painter.add(Shape::convex_polygon(p, cv.c(fill), stroke));
                }
                let pattern = match e.text("pattern").as_str() {
                    "hatch" => Pattern::Hatch,
                    "dots" => Pattern::Dots,
                    "grid" => Pattern::Grid,
                    _ => Pattern::None,
                };
                if pattern != Pattern::None {
                    let mut st = cv.theme.styles.bar.clone();
                    st.pattern_density = e.num("density", 5.0);
                    st.pattern_angle = e.num("angle", -45.0);
                    let mut theme = cv.theme.clone();
                    st.empty = pattern;
                    st.edge = false;
                    theme.styles.bars.insert("__pattern".into(), st);
                    theme.tokens.colors.insert("track".into(), Color32::TRANSPARENT);
                    let tc = Canvas { theme: &theme, ..cv.clone() };
                    tc.bar("__pattern", 0.0, 0.0, w, h, &[], false);
                }
            }
            Kind::Circle => {
                let r = e.num("radius", 4.0);
                let fill = if e.has("fill") { self.color(&e, "fill", "text") } else { Color32::TRANSPARENT };
                let stroke = if e.has("stroke") {
                    Stroke::new(e.num("stroke-width", 1.0) * cv.scale, cv.c(self.color(&e, "stroke", "text")))
                } else {
                    Stroke::NONE
                };
                let glow = e.num("glow", 0.0);
                if glow > 0.0 {
                    cv.painter.circle_filled(cv.p(r, r), (r + glow) * cv.scale, fade(cv.c(fill), 0.18));
                }
                cv.painter.circle(cv.p(r, r), r * cv.scale, cv.c(fill), stroke);
            }
            Kind::Polygon => {
                let mut pts = parse_points(&e.text("points"));
                let b = bounds(&pts);
                rotate_points(&mut pts, e.num("rotate", 0.0), (b.x / 2.0, b.y / 2.0));
                flip_points(&mut pts, &e.text("flip"), (b.x, b.y));
                fill_and_stroke(cv, &[(pts, true)], &e, self);
            }
            Kind::Path => {
                let mut subs = flatten_path(&e.text("d"));
                let all: Vec<(f32, f32)> = subs.iter().flat_map(|(p, _)| p.iter().copied()).collect();
                let b = bounds(&all);
                for (p, _) in subs.iter_mut() {
                    rotate_points(p, e.num("rotate", 0.0), (b.x / 2.0, b.y / 2.0));
                    flip_points(p, &e.text("flip"), (b.x, b.y));
                }
                if e.has("value") {
                    // 沿路径的进度条：先画整条轨道，再画前面一段
                    let width = e.num("stroke-width", 3.0);
                    if e.has("track") {
                        let col = self.color(&e, "track", "track");
                        for (p, closed) in &subs {
                            stroke_poly(cv, p, *closed, width, col);
                        }
                    }
                    let v = e.num("value", 0.0).clamp(0.0, 1.0);
                    let col = self.color(&e, "stroke", "text");
                    for (p, _) in &subs {
                        let part = prefix_by_length(p, v);
                        stroke_poly(cv, &part, false, width, col);
                    }
                } else {
                    fill_and_stroke(cv, &subs, &e, self);
                }
            }
            Kind::Image => {
                let (w, h) = (e.num("width", 0.0), e.num("height", 0.0));
                let src = e.text("src");
                let px = |v: f32| (v * cv.scale).round().max(0.0) as u32;
                let req = if src.to_ascii_lowercase().ends_with(".svg") { (px(w), px(h)) } else { (0, 0) };
                if let Some((tex, size)) = self.images.get(&self.ctx, &src, req.0, req.1) {
                    let (w, h) = (if w > 0.0 { w } else { size.x }, if h > 0.0 { h } else { size.y });
                    let tint = if e.has("tint") { cv.c(self.color(&e, "tint", "text")) } else { cv.c(Color32::WHITE) };
                    let nine: Vec<f32> = e.text("nine").split(',').filter_map(|v| v.trim().parse().ok()).collect();
                    let flip = e.text("flip");
                    if let [l, t, r, b] = nine[..] {
                        nine_slice(cv, &tex, size, (l, t, r, b), w, h, tint);
                    } else {
                        let mut uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                        if flip.contains('h') {
                            std::mem::swap(&mut uv.min.x, &mut uv.max.x);
                        }
                        if flip.contains('v') {
                            std::mem::swap(&mut uv.min.y, &mut uv.max.y);
                        }
                        cv.painter.image(tex.id(), cv.r(0.0, 0.0, w, h), uv, tint);
                    }
                }
            }
            Kind::Pips => {
                cv.pips(
                    0.0,
                    0.0,
                    e.num("count", 0.0) as i32,
                    e.num("filled", 0.0) as i32,
                    e.num("size", 6.0),
                    e.num("gap", 3.0),
                    self.color(&e, "color", "text"),
                );
            }
            Kind::Line => {
                cv.line(
                    (e.num("x1", 0.0), e.num("y1", 0.0)),
                    (e.num("x2", 0.0), e.num("y2", 0.0)),
                    e.num("stroke-width", 1.0),
                    self.color(&e, "stroke", "text"),
                );
            }
            Kind::Spacer => {}
        }
    }
}

fn cross(align: &str, total: f32, own: f32) -> f32 {
    match align {
        "center" => (total - own) / 2.0,
        "end" | "right" | "bottom" => total - own,
        _ => 0.0,
    }
}

fn stack_size(kind: Kind, sizes: &[(Vec2, Vec2)], gap: f32, pad: f32) -> Vec2 {
    let visible: Vec<&(Vec2, Vec2)> = sizes.iter().filter(|(s, _)| s.x > 0.0 || s.y > 0.0).collect();
    let n = visible.len() as f32;
    let g = if n > 1.0 { gap * (n - 1.0) } else { 0.0 };
    let v = match kind {
        Kind::Row => vec2(visible.iter().map(|(s, _)| s.x).sum::<f32>() + g, visible.iter().map(|(s, _)| s.y).fold(0.0, f32::max)),
        Kind::Stack => vec2(
            visible.iter().map(|(s, o)| s.x + o.x).fold(0.0, f32::max),
            visible.iter().map(|(s, o)| s.y + o.y).fold(0.0, f32::max),
        ),
        _ => vec2(visible.iter().map(|(s, _)| s.x).fold(0.0, f32::max), visible.iter().map(|(s, _)| s.y).sum::<f32>() + g),
    };
    v + vec2(pad * 2.0, pad * 2.0)
}

fn parse_points(s: &str) -> Vec<(f32, f32)> {
    s.split_whitespace()
        .filter_map(|p| {
            let (x, y) = p.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

fn bounds(pts: &[(f32, f32)]) -> Vec2 {
    vec2(
        pts.iter().map(|p| p.0).fold(0.0, f32::max),
        pts.iter().map(|p| p.1).fold(0.0, f32::max),
    )
}

fn rotate_points(pts: &mut [(f32, f32)], deg: f32, c: (f32, f32)) {
    if deg.abs() < 0.01 {
        return;
    }
    let (s, co) = deg.to_radians().sin_cos();
    for p in pts {
        let (x, y) = (p.0 - c.0, p.1 - c.1);
        *p = (c.0 + x * co - y * s, c.1 + x * s + y * co);
    }
}

fn flip_points(pts: &mut [(f32, f32)], flip: &str, size: (f32, f32)) {
    for p in pts {
        if flip.contains('h') {
            p.0 = size.0 - p.0;
        }
        if flip.contains('v') {
            p.1 = size.1 - p.1;
        }
    }
}

/// SVG path 展平成折线，每段带"是否闭合"。
fn flatten_path(d: &str) -> Vec<(Vec<(f32, f32)>, bool)> {
    let Ok(path) = kurbo::BezPath::from_svg(d) else {
        return Vec::new();
    };
    let mut out: Vec<(Vec<(f32, f32)>, bool)> = Vec::new();
    kurbo::flatten(&path, 0.25, |el| match el {
        kurbo::PathEl::MoveTo(p) => out.push((vec![(p.x as f32, p.y as f32)], false)),
        kurbo::PathEl::LineTo(p) => {
            if let Some(last) = out.last_mut() {
                last.0.push((p.x as f32, p.y as f32));
            }
        }
        kurbo::PathEl::ClosePath => {
            if let Some(last) = out.last_mut() {
                last.1 = true;
            }
        }
        _ => {}
    });
    out
}

fn prefix_by_length(pts: &[(f32, f32)], ratio: f32) -> Vec<(f32, f32)> {
    let seg = |a: (f32, f32), b: (f32, f32)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let total: f32 = pts.windows(2).map(|w| seg(w[0], w[1])).sum();
    let mut left = total * ratio;
    let mut out = vec![];
    if let Some(&first) = pts.first() {
        out.push(first);
    }
    for w in pts.windows(2) {
        let l = seg(w[0], w[1]);
        if left >= l {
            out.push(w[1]);
            left -= l;
        } else {
            let t = if l > 0.0 { left / l } else { 0.0 };
            out.push((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
            break;
        }
    }
    out
}

fn stroke_poly(cv: &Canvas, pts: &[(f32, f32)], closed: bool, width: f32, color: Color32) {
    if pts.len() < 2 {
        return;
    }
    let p: Vec<Pos2> = pts.iter().map(|&(x, y)| cv.p(x, y)).collect();
    cv.painter.add(Shape::Path(PathShape {
        points: p,
        closed,
        fill: Color32::TRANSPARENT,
        stroke: PathStroke::new(width * cv.scale, cv.c(color)),
    }));
}

fn fill_and_stroke(cv: &Canvas, subs: &[(Vec<(f32, f32)>, bool)], e: &Eff, r: &Renderer) {
    if e.has("fill") {
        let fill = cv.c(r.color(e, "fill", "text"));
        for (p, _) in subs {
            let tris = triangulate(p);
            let mut mesh = Mesh::default();
            for &(x, y) in p {
                mesh.colored_vertex(cv.p(x, y), fill);
            }
            for t in tris {
                mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
            }
            cv.painter.add(Shape::mesh(mesh));
        }
    }
    if e.has("stroke") {
        let col = r.color(e, "stroke", "text");
        for (p, closed) in subs {
            stroke_poly(cv, p, *closed || e.has("fill"), e.num("stroke-width", 1.0), col);
        }
    }
}

/// 简单多边形的耳切三角剖分，支持凹多边形（不支持洞）。
pub fn triangulate(pts: &[(f32, f32)]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let inside = |p: (f32, f32), a, b, c| cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0;
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            if idx.iter().any(|&j| j != ia && j != ib && j != ic && inside(pts[j], a, b, c)) {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

fn arc(cv: &Canvas, c: Vec2, r: f32, from: f32, to: f32, th: f32, color: Color32) {
    let steps = (((to - from).abs() / 4.0).ceil() as usize).clamp(2, 180);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / steps as f32 - 90.0).to_radians();
            cv.p(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    cv.painter.add(Shape::line(pts, Stroke::new(th * cv.scale, cv.c(color))));
}

fn nine_slice(cv: &Canvas, tex: &TextureHandle, size: Vec2, m: (f32, f32, f32, f32), w: f32, h: f32, tint: Color32) {
    let (l, t, r, b) = m;
    let xs = [0.0, l, w - r, w];
    let ys = [0.0, t, h - b, h];
    let us = [0.0, l / size.x, 1.0 - r / size.x, 1.0];
    let vs = [0.0, t / size.y, 1.0 - b / size.y, 1.0];
    for j in 0..3 {
        for i in 0..3 {
            let rect = Rect::from_min_max(cv.p(xs[i], ys[j]), cv.p(xs[i + 1], ys[j + 1]));
            let uv = Rect::from_min_max(pos2(us[i], vs[j]), pos2(us[i + 1], vs[j + 1]));
            cv.painter.image(tex.id(), rect, uv, tint);
        }
    }
}

/// 量模板的尺寸。
pub fn measure(t: &Template, canvas: &Canvas, images: &mut Images, ctx: &egui::Context, data: &Value) -> Vec2 {
    let mut env = Env {
        root: data,
        vars: Vec::new(),
    };
    let mut r = Renderer {
        canvas,
        images,
        ctx: ctx.clone(),
        time: 0.0,
        animating: false,
    };
    r.measure(&t.root, &mut env)
}

/// 画模板，返回（尺寸，是否有动画需要继续重绘）。
pub fn render(t: &Template, canvas: &Canvas, images: &mut Images, ctx: &egui::Context, data: &Value, time: f64) -> (Vec2, bool) {
    let mut env = Env {
        root: data,
        vars: Vec::new(),
    };
    let mut r = Renderer {
        canvas,
        images,
        ctx: ctx.clone(),
        time,
        animating: false,
    };
    let size = r.measure(&t.root, &mut env);
    r.paint(&t.root, &mut env, 0.0, 0.0, 1.0);
    (size, r.animating)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulates_concave_polygon() {
        // L 形，6 个点，4 个三角形
        let pts = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        assert_eq!(triangulate(&pts).len(), 4);
        let rev: Vec<_> = pts.iter().rev().copied().collect();
        assert_eq!(triangulate(&rev).len(), 4);
    }

    #[test]
    fn flattens_svg_paths() {
        let subs = flatten_path("M0 0 L10 0 L10 10 Z M20 20 Q30 0 40 20");
        assert_eq!(subs.len(), 2);
        assert!(subs[0].1);
        assert!(subs[1].0.len() > 3);
    }

    #[test]
    fn prefix_half_of_line() {
        let p = prefix_by_length(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 0.75);
        assert_eq!(p.last().copied(), Some((10.0, 5.0)));
    }

    #[test]
    fn stack_sizes() {
        let s = [(vec2(10.0, 5.0), Vec2::ZERO), (vec2(20.0, 3.0), Vec2::ZERO)];
        assert_eq!(stack_size(Kind::Row, &s, 2.0, 0.0), vec2(32.0, 5.0));
        assert_eq!(stack_size(Kind::Column, &s, 2.0, 1.0), vec2(22.0, 12.0));
    }
}
