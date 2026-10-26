//! 主题：令牌、组件样式、模块模板三层。
//!
//! 主题文件是 XML，命名空间 `urn:rustyhunt:theme:1`，格式见 `docs/theme.xsd`。
//! 下层没写的部分继承内置默认值。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::Color32;

use crate::color::{self, rgba};
use crate::template::{self, Template};

pub const NAMESPACE: &str = "urn:rustyhunt:theme:1";
pub const SCHEMA_VERSION: u32 = 1;

/// 主题包的大小上限
pub const MAX_PACK_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_IMAGE_SIDE: u32 = 4096;

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError {
    pub file: PathBuf,
    pub line: u32,
    pub message: String,
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.file.file_name().and_then(|n| n.to_str()).unwrap_or("theme.xml");
        if self.line > 0 {
            write!(f, "{name} 第 {} 行：{}", self.line, self.message)
        } else {
            write!(f, "{name}：{}", self.message)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub tags: Vec<String>,
    /// 需要的程序版本，例如 "1.0"
    pub app: String,
    pub preview: Option<PathBuf>,
    /// 主界面的底色基于浅色还是深色
    pub dark: bool,
}

/// 第一层：令牌。颜色按名字存，模板里也能用名字引用。
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub colors: BTreeMap<String, Color32>,
    pub fonts: BTreeMap<String, FontToken>,
    pub durations: BTreeMap<String, f32>,
    pub sizes: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontToken {
    pub family: String,
    pub weight: u16,
    /// 主题包里自带的字体文件
    pub src: Option<PathBuf>,
}

impl Tokens {
    pub fn color(&self, name: &str) -> Color32 {
        self.colors.get(name).copied().unwrap_or(Color32::MAGENTA)
    }

    pub fn duration(&self, name: &str) -> f32 {
        self.durations.get(name).copied().unwrap_or(0.25)
    }

    pub fn size(&self, name: &str, default: f32) -> f32 {
        self.sizes.get(name).copied().unwrap_or(default)
    }
}

/// 局内（overlay）的默认令牌。
pub fn overlay_defaults() -> BTreeMap<String, Color32> {
    [
        ("text", Color32::from_rgb(0xf2, 0xf0, 0xe9)),
        ("text-mute", rgba(242, 240, 233, 0.66)),
        ("text-faint", rgba(242, 240, 233, 0.34)),
        ("ink", Color32::from_rgb(0x11, 0x11, 0x11)),
        ("accent", Color32::from_rgb(0xff, 0x6b, 0x1a)),
        ("danger", Color32::from_rgb(0xe5, 0x48, 0x4d)),
        ("cool", Color32::from_rgb(0x8c, 0xc8, 0xff)),
        ("ok", Color32::from_rgb(0x8f, 0xe0, 0xa0)),
        ("ghost", rgba(255, 196, 140, 0.9)),
        ("heal", rgba(140, 220, 150, 0.85)),
        ("track", rgba(0, 0, 0, 0.42)),
        ("edge", rgba(0, 0, 0, 0.55)),
        ("shadow", rgba(0, 0, 0, 0.95)),
        ("hatch", rgba(242, 240, 233, 0.30)),
        ("crown-gold", Color32::from_rgb(0xe9, 0xc2, 0x5b)),
        ("crown-silver", Color32::from_rgb(0xd9, 0xdd, 0xe2)),
        ("qurio", Color32::from_rgb(0xc4, 0x8c, 0xff)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// 主界面的默认令牌：浅色和深色两套。
pub fn window_defaults(dark: bool) -> BTreeMap<String, Color32> {
    let hex = |s: &str| color::parse(s).unwrap();
    let pairs: [(&str, &str); 9] = if dark {
        [
            ("window.paper", "#191918"),
            ("window.ink", "#ebeae4"),
            ("window.mute", "#8a8882"),
            ("window.faint", "#57554f"),
            ("window.line", "#2c2b29"),
            ("window.block", "#ebeae4"),
            ("window.onblock", "#141414"),
            ("window.accent", "#ff6b1a"),
            ("window.desk", "#0a0a0a"),
        ]
    } else {
        [
            ("window.paper", "#ecebe6"),
            ("window.ink", "#141414"),
            ("window.mute", "#86847d"),
            ("window.faint", "#b4b2ab"),
            ("window.line", "#d5d3cc"),
            ("window.block", "#141414"),
            ("window.onblock", "#ecebe6"),
            ("window.accent", "#ff6b1a"),
            ("window.desk", "#2b2b2a"),
        ]
    };
    pairs.into_iter().map(|(k, v)| (k.to_string(), hex(v))).collect()
}

impl Tokens {
    pub fn defaults(dark: bool) -> Tokens {
        let mut colors = overlay_defaults();
        colors.extend(window_defaults(dark));
        let font = |family: &str, weight| FontToken {
            family: family.to_string(),
            weight,
            src: None,
        };
        Tokens {
            colors,
            fonts: [
                ("number".to_string(), font("Montserrat", 800)),
                ("number-heavy".to_string(), font("Montserrat", 900)),
                ("number-bold".to_string(), font("Montserrat", 700)),
                ("text".to_string(), font("Microsoft YaHei", 400)),
                ("text-bold".to_string(), font("Microsoft YaHei", 700)),
            ]
            .into_iter()
            .collect(),
            durations: [("fade", 0.5), ("flash", 0.8), ("toast", 2.2), ("bar", 0.25)]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            sizes: [("idle-opacity", 0.5), ("shadow", 1.0)].into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarShape {
    #[default]
    Rect,
    Round,
    Parallelogram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pattern {
    #[default]
    None,
    Hatch,
    Dots,
    Grid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BarStyle {
    pub shape: BarShape,
    pub skew: f32,
    pub radius: f32,
    /// 条高度的倍率；模块里每种条的基础高度不同
    pub height: f32,
    pub segments: u32,
    pub gap: f32,
    pub empty: Pattern,
    pub pattern_density: f32,
    pub pattern_angle: f32,
    pub caps: bool,
    pub edge: bool,
    pub glow: f32,
}

impl Default for BarStyle {
    fn default() -> Self {
        BarStyle {
            shape: BarShape::Rect,
            skew: 0.0,
            radius: 0.0,
            height: 1.0,
            segments: 0,
            gap: 2.0,
            empty: Pattern::None,
            pattern_density: 5.0,
            pattern_angle: -45.0,
            caps: false,
            edge: true,
            glow: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagShape {
    #[default]
    Rect,
    Cut,
    Round,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagStyle {
    pub shape: TagShape,
    pub cut: f32,
    pub radius: f32,
}

impl Default for TagStyle {
    fn default() -> Self {
        TagStyle {
            shape: TagShape::Rect,
            cut: 4.0,
            radius: 3.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PipShape {
    #[default]
    Square,
    Diamond,
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwitchShape {
    #[default]
    Square,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelShape {
    /// 不画底板（默认，"无感"）
    #[default]
    None,
    Rect,
    Cut,
    Round,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelStyle {
    pub shape: PanelShape,
    pub fill: Color32,
    pub stroke: Color32,
    pub padding: f32,
    pub cut: f32,
    pub radius: f32,
}

impl Default for PanelStyle {
    fn default() -> Self {
        PanelStyle {
            shape: PanelShape::None,
            fill: rgba(10, 10, 10, 0.55),
            stroke: Color32::TRANSPARENT,
            padding: 10.0,
            cut: 8.0,
            radius: 6.0,
        }
    }
}

/// 第二层：组件样式。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Styles {
    pub bar: BarStyle,
    /// 单独给某种条覆盖样式：monster-hp、part、ailment、player-hp、stamina、damage
    pub bars: BTreeMap<String, BarStyle>,
    pub tag: TagStyle,
    pub pip: PipShape,
    pub switch: SwitchShape,
    pub panel: PanelStyle,
    /// 文字阴影：0 关，1 默认
    pub text_shadow: f32,
}

impl Styles {
    pub fn bar(&self, kind: &str) -> &BarStyle {
        self.bars.get(kind).unwrap_or(&self.bar)
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub manifest: Manifest,
    pub tokens: Tokens,
    pub styles: Styles,
    /// 第三层：替换整个模块的模板，按模块编号
    pub modules: BTreeMap<String, Template>,
    /// 主题包目录；内置主题为 None
    pub dir: Option<PathBuf>,
    pub builtin: bool,
    /// 只影响某个模块时出错：这个模块退回默认，其他照常
    pub module_errors: Vec<ThemeError>,
}

impl Theme {
    pub fn builtin(dark: bool) -> Theme {
        Theme {
            manifest: Manifest {
                id: if dark { "dark" } else { "light" }.into(),
                name: if dark { "theme.dark" } else { "theme.light" }.into(),
                description: String::new(),
                version: "1.0.0".into(),
                author: String::new(),
                tags: vec![],
                app: "1.0".into(),
                preview: None,
                dark,
            },
            tokens: Tokens::defaults(dark),
            styles: Styles {
                text_shadow: 1.0,
                ..Default::default()
            },
            modules: BTreeMap::new(),
            dir: None,
            builtin: true,
            module_errors: Vec::new(),
        }
    }

    pub fn color(&self, name: &str) -> Color32 {
        self.tokens.color(name)
    }

    /// 读取主题包目录（里面有 theme.xml）。整个文件格式错误时返回错误；
    /// 单个模块模板出错时只记在 `module_errors` 里。
    pub fn load_dir(dir: &Path) -> Result<Theme, ThemeError> {
        let file = dir.join("theme.xml");
        let text = std::fs::read_to_string(&file).map_err(|e| ThemeError {
            file: file.clone(),
            line: 0,
            message: e.to_string(),
        })?;
        let mut theme = parse(&text, &file, Some(dir))?;
        theme.dir = Some(dir.to_path_buf());
        Ok(theme)
    }
}

struct Ctx<'a> {
    file: &'a Path,
    doc: &'a roxmltree::Document<'a>,
}

impl Ctx<'_> {
    fn err(&self, node: roxmltree::Node, message: impl Into<String>) -> ThemeError {
        ThemeError {
            file: self.file.to_path_buf(),
            line: self.doc.text_pos_at(node.range().start).row,
            message: message.into(),
        }
    }

    fn attr_f32(&self, node: roxmltree::Node, name: &str, default: f32) -> Result<f32, ThemeError> {
        match node.attribute(name) {
            None => Ok(default),
            Some(v) => v.trim_end_matches("px").parse().map_err(|_| self.err(node, format!("{name}=\"{v}\" 不是数字"))),
        }
    }

    fn attr_bool(&self, node: roxmltree::Node, name: &str, default: bool) -> Result<bool, ThemeError> {
        match node.attribute(name) {
            None => Ok(default),
            Some("true" | "1" | "yes") => Ok(true),
            Some("false" | "0" | "no") => Ok(false),
            Some(v) => Err(self.err(node, format!("{name}=\"{v}\" 应该是 true 或 false"))),
        }
    }

    fn attr_color(&self, node: roxmltree::Node, name: &str, tokens: &Tokens) -> Result<Option<Color32>, ThemeError> {
        match node.attribute(name) {
            None => Ok(None),
            Some(v) => resolve_color(v, tokens)
                .map(Some)
                .ok_or_else(|| self.err(node, format!("{name}=\"{v}\" 不是颜色，也不是已有的令牌"))),
        }
    }
}

/// 颜色值可以直接写，也可以写令牌名。
pub fn resolve_color(v: &str, tokens: &Tokens) -> Option<Color32> {
    color::parse(v).or_else(|| tokens.colors.get(v).copied())
}

/// 解析 theme.xml 文本。`dir` 是主题包目录，用来读 `<include>`。
pub fn parse(text: &str, file: &Path, dir: Option<&Path>) -> Result<Theme, ThemeError> {
    let doc = roxmltree::Document::parse(text).map_err(|e| ThemeError {
        file: file.to_path_buf(),
        line: e.pos().row,
        message: format!("XML 格式错误：{e}"),
    })?;
    let cx = Ctx { file, doc: &doc };
    let root = doc.root_element();
    if root.tag_name().name() != "theme" {
        return Err(cx.err(root, "根元素必须是 <theme>"));
    }
    match root.tag_name().namespace() {
        Some(NAMESPACE) => {}
        Some(other) => {
            return Err(cx.err(root, format!("不支持的命名空间 {other}，当前程序支持 {NAMESPACE}")));
        }
        None => return Err(cx.err(root, format!("缺少 xmlns=\"{NAMESPACE}\""))),
    }

    let need = |name: &str| {
        root.attribute(name)
            .map(str::to_string)
            .ok_or_else(|| cx.err(root, format!("<theme> 缺少 {name} 属性")))
    };
    let dark = match root.attribute("base") {
        None | Some("dark") => true,
        Some("light") => false,
        Some(v) => return Err(cx.err(root, format!("base=\"{v}\" 应该是 light 或 dark"))),
    };
    let manifest = Manifest {
        id: need("id")?,
        name: need("name")?,
        description: root.attribute("description").unwrap_or_default().to_string(),
        version: root.attribute("version").unwrap_or("0.0.0").to_string(),
        author: root.attribute("author").unwrap_or_default().to_string(),
        tags: root
            .attribute("tags")
            .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default(),
        app: root.attribute("app").unwrap_or("1.0").to_string(),
        preview: root.attribute("preview").map(PathBuf::from),
        dark,
    };
    if !manifest.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(cx.err(root, "id 只能用字母、数字、- 和 _"));
    }

    let mut theme = Theme {
        manifest,
        tokens: Tokens::defaults(dark),
        styles: Styles {
            text_shadow: 1.0,
            ..Default::default()
        },
        modules: BTreeMap::new(),
        dir: None,
        builtin: false,
        module_errors: Vec::new(),
    };

    for child in root.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "tokens" => parse_tokens(&cx, child, &mut theme.tokens, dir)?,
            "styles" => parse_styles(&cx, child, &mut theme)?,
            "module" => parse_module(&cx, child, &mut theme),
            "include" => {
                let Some(src) = child.attribute("src") else {
                    return Err(cx.err(child, "<include> 缺少 src"));
                };
                let Some(dir) = dir else { continue };
                let path = dir.join(src);
                if !path.starts_with(dir) || src.contains("..") {
                    return Err(cx.err(child, "include 只能引用主题包里的文件"));
                }
                let inc = std::fs::read_to_string(&path).map_err(|e| cx.err(child, format!("读不到 {src}：{e}")))?;
                let idoc = roxmltree::Document::parse(&inc).map_err(|e| ThemeError {
                    file: path.clone(),
                    line: e.pos().row,
                    message: format!("XML 格式错误：{e}"),
                })?;
                let icx = Ctx { file: &path, doc: &idoc };
                for m in idoc.root_element().descendants().filter(|n| n.has_tag_name("module")) {
                    parse_module(&icx, m, &mut theme);
                }
            }
            other => return Err(cx.err(child, format!("不认识的元素 <{other}>"))),
        }
    }
    Ok(theme)
}

fn parse_tokens(cx: &Ctx, node: roxmltree::Node, tokens: &mut Tokens, dir: Option<&Path>) -> Result<(), ThemeError> {
    for t in node.children().filter(|n| n.is_element()) {
        let name = t
            .attribute("name")
            .ok_or_else(|| cx.err(t, format!("<{}> 缺少 name", t.tag_name().name())))?
            .to_string();
        match t.tag_name().name() {
            "color" => {
                let v = t.attribute("value").ok_or_else(|| cx.err(t, "<color> 缺少 value"))?;
                let c = resolve_color(v, tokens).ok_or_else(|| cx.err(t, format!("value=\"{v}\" 不是颜色")))?;
                tokens.colors.insert(name, c);
            }
            "font" => {
                let weight = cx.attr_f32(t, "weight", 400.0)? as u16;
                let src = t.attribute("src").map(PathBuf::from);
                if let (Some(src), Some(dir)) = (&src, dir)
                    && !dir.join(src).is_file()
                {
                    return Err(cx.err(t, format!("字体文件 {} 不存在", src.display())));
                }
                tokens.fonts.insert(
                    name,
                    FontToken {
                        family: t.attribute("family").unwrap_or("Montserrat").to_string(),
                        weight,
                        src,
                    },
                );
            }
            "duration" => {
                let ms = cx.attr_f32(t, "ms", 250.0)?;
                tokens.durations.insert(name, (ms / 1000.0).clamp(0.0, 5.0));
            }
            "size" => {
                let v = cx.attr_f32(t, "value", 1.0)?;
                tokens.sizes.insert(name, v);
            }
            other => return Err(cx.err(t, format!("<tokens> 里不认识 <{other}>"))),
        }
    }
    Ok(())
}

fn parse_pattern(cx: &Ctx, node: roxmltree::Node, v: Option<&str>) -> Result<Pattern, ThemeError> {
    Ok(match v {
        None | Some("none") => Pattern::None,
        Some("hatch") => Pattern::Hatch,
        Some("dots") => Pattern::Dots,
        Some("grid") => Pattern::Grid,
        Some(v) => return Err(cx.err(node, format!("empty=\"{v}\" 应该是 none、hatch、dots 或 grid"))),
    })
}

fn bar_style(cx: &Ctx, node: roxmltree::Node, base: &BarStyle) -> Result<BarStyle, ThemeError> {
    let shape = match node.attribute("shape") {
        None => base.shape,
        Some("rect") => BarShape::Rect,
        Some("round") => BarShape::Round,
        Some("parallelogram") => BarShape::Parallelogram,
        Some(v) => return Err(cx.err(node, format!("shape=\"{v}\" 应该是 rect、round 或 parallelogram"))),
    };
    Ok(BarStyle {
        shape,
        skew: cx.attr_f32(node, "skew", base.skew)?,
        radius: cx.attr_f32(node, "radius", base.radius)?,
        height: cx.attr_f32(node, "height", base.height)?.clamp(0.2, 6.0),
        segments: cx.attr_f32(node, "segments", base.segments as f32)?.clamp(0.0, 64.0) as u32,
        gap: cx.attr_f32(node, "gap", base.gap)?,
        empty: if node.has_attribute("empty") {
            parse_pattern(cx, node, node.attribute("empty"))?
        } else {
            base.empty
        },
        pattern_density: cx.attr_f32(node, "density", base.pattern_density)?.clamp(2.0, 40.0),
        pattern_angle: cx.attr_f32(node, "angle", base.pattern_angle)?,
        caps: cx.attr_bool(node, "caps", base.caps)?,
        edge: cx.attr_bool(node, "edge", base.edge)?,
        glow: cx.attr_f32(node, "glow", base.glow)?.clamp(0.0, 8.0),
    })
}

fn parse_styles(cx: &Ctx, node: roxmltree::Node, theme: &mut Theme) -> Result<(), ThemeError> {
    for s in node.children().filter(|n| n.is_element()) {
        match s.tag_name().name() {
            "bar" => match s.attribute("for") {
                None => theme.styles.bar = bar_style(cx, s, &theme.styles.bar)?,
                Some(kind) => {
                    let base = theme.styles.bar.clone();
                    let st = bar_style(cx, s, &base)?;
                    theme.styles.bars.insert(kind.to_string(), st);
                }
            },
            "tag" => {
                let t = &mut theme.styles.tag;
                t.shape = match s.attribute("shape") {
                    None => t.shape,
                    Some("rect") => TagShape::Rect,
                    Some("cut") => TagShape::Cut,
                    Some("round") => TagShape::Round,
                    Some(v) => return Err(cx.err(s, format!("shape=\"{v}\" 应该是 rect、cut 或 round"))),
                };
                t.cut = cx.attr_f32(s, "cut", t.cut)?;
                t.radius = cx.attr_f32(s, "radius", t.radius)?;
            }
            "pip" => {
                theme.styles.pip = match s.attribute("shape") {
                    None | Some("square") => PipShape::Square,
                    Some("diamond") => PipShape::Diamond,
                    Some("circle") => PipShape::Circle,
                    Some(v) => return Err(cx.err(s, format!("shape=\"{v}\" 应该是 square、diamond 或 circle"))),
                };
            }
            "switch" => {
                theme.styles.switch = match s.attribute("shape") {
                    None | Some("square") => SwitchShape::Square,
                    Some("round") => SwitchShape::Round,
                    Some(v) => return Err(cx.err(s, format!("shape=\"{v}\" 应该是 square 或 round"))),
                };
            }
            "panel" => {
                let tokens = theme.tokens.clone();
                let p = &mut theme.styles.panel;
                p.shape = match s.attribute("shape") {
                    None => p.shape,
                    Some("none") => PanelShape::None,
                    Some("rect") => PanelShape::Rect,
                    Some("cut") => PanelShape::Cut,
                    Some("round") => PanelShape::Round,
                    Some(v) => return Err(cx.err(s, format!("shape=\"{v}\" 应该是 none、rect、cut 或 round"))),
                };
                if let Some(c) = cx.attr_color(s, "fill", &tokens)? {
                    p.fill = c;
                }
                if let Some(c) = cx.attr_color(s, "stroke", &tokens)? {
                    p.stroke = c;
                }
                p.padding = cx.attr_f32(s, "padding", p.padding)?;
                p.cut = cx.attr_f32(s, "cut", p.cut)?;
                p.radius = cx.attr_f32(s, "radius", p.radius)?;
            }
            "text" => {
                theme.styles.text_shadow = cx.attr_f32(s, "shadow", theme.styles.text_shadow)?.clamp(0.0, 3.0);
            }
            other => return Err(cx.err(s, format!("<styles> 里不认识 <{other}>"))),
        }
    }
    Ok(())
}

fn parse_module(cx: &Ctx, node: roxmltree::Node, theme: &mut Theme) {
    let Some(id) = node.attribute("id") else {
        theme.module_errors.push(cx.err(node, "<module> 缺少 id"));
        return;
    };
    match template::parse_module(cx.file, cx.doc, node, &theme.tokens) {
        Ok(t) => {
            theme.modules.insert(id.to_string(), t);
        }
        Err(e) => {
            log::warn!("主题模块 {id} 出错，退回默认样式：{e}");
            theme.module_errors.push(e);
        }
    }
}

/// 比较版本号（只看主、次版本）：`required` 是否不高于 `current`。
pub fn version_ok(required: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32) {
        let mut it = v.split('.').map(|x| x.trim().parse::<u32>().unwrap_or(0));
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    parse(required) <= parse(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"<theme xmlns="urn:rustyhunt:theme:1" id="ember" name="熔岩" version="0.9.0" author="Momo" app="1.0">
  <tokens>
    <color name="accent" value="#ffb347"/>
    <color name="text" value="#f0e3d6"/>
    <color name="hp" value="accent"/>
    <font name="number" family="Montserrat" weight="800"/>
    <duration name="fade" ms="400"/>
  </tokens>
  <styles>
    <bar shape="parallelogram" skew="6" height="1.2" segments="10" gap="2" empty="hatch" caps="true"/>
    <bar for="part" shape="rect" segments="0"/>
    <tag shape="cut" cut="4"/>
  </styles>
</theme>"##;

    #[test]
    fn parses_sample_theme() {
        let t = parse(SAMPLE, Path::new("theme.xml"), None).unwrap();
        assert_eq!(t.manifest.id, "ember");
        assert_eq!(t.manifest.name, "熔岩");
        assert_eq!(t.tokens.color("accent"), color::parse("#ffb347").unwrap());
        assert_eq!(t.tokens.color("hp"), color::parse("#ffb347").unwrap());
        assert_eq!(t.tokens.duration("fade"), 0.4);
        assert_eq!(t.styles.bar.shape, BarShape::Parallelogram);
        assert_eq!(t.styles.bar.segments, 10);
        assert!(t.styles.bar.caps);
        assert_eq!(t.styles.bar("part").segments, 0);
        assert_eq!(t.styles.bar("part").empty, Pattern::Hatch);
        assert_eq!(t.styles.bar("monster-hp").segments, 10);
        assert_eq!(t.styles.tag.shape, TagShape::Cut);
        // 没写的令牌继承默认
        assert_eq!(t.tokens.color("danger"), Color32::from_rgb(0xe5, 0x48, 0x4d));
    }

    #[test]
    fn reports_line_numbers() {
        let bad = "<theme xmlns=\"urn:rustyhunt:theme:1\" id=\"x\" name=\"x\">\n<tokens>\n<color name=\"a\" value=\"nope\"/>\n</tokens></theme>";
        let e = parse(bad, Path::new("theme.xml"), None).unwrap_err();
        assert_eq!(e.line, 3);
        assert!(e.message.contains("nope"));

        let e = parse("<theme id=\"x\" name=\"x\"/>", Path::new("t.xml"), None).unwrap_err();
        assert!(e.message.contains("xmlns"));

        let e = parse("<theme", Path::new("t.xml"), None).unwrap_err();
        assert!(e.message.contains("XML"));
    }

    #[test]
    fn version_compare() {
        assert!(version_ok("1.0", "1.0.0"));
        assert!(version_ok("0.3", "1.0.0"));
        assert!(!version_ok("1.2", "1.0.0"));
        assert!(!version_ok("2.0", "1.9"));
    }
}
