//! 模块模板：用 XML 写的界面描述。主题用它替换内置模块，
//! 插件用它描述自己的局内模块。
//!
//! 元素：容器 `column` `row` `stack` `for` `if`；图元 `text` `tag` `bar` `arc`
//! `rect` `circle` `polygon` `path` `image` `pips` `line` `spacer`。
//! 任何元素下可以放 `<state when="表达式" .../>`，条件成立时覆盖属性或加动画。

use std::collections::BTreeMap;
use std::path::Path;

use crate::expr::{self, Expr, Text};
use crate::fields;
use crate::theme::{ThemeError, Tokens};

/// 单个模板的节点数上限，保证 overlay 单帧耗时
pub const MAX_NODES: usize = 512;
/// `for` 展开的项数上限
pub const MAX_ITEMS: usize = 64;
/// 带动画的状态数上限
pub const MAX_ANIMATIONS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Column,
    Row,
    Stack,
    For,
    If,
    Text,
    Tag,
    Bar,
    Arc,
    Rect,
    Circle,
    Polygon,
    Path,
    Image,
    Pips,
    Line,
    Spacer,
}

impl Kind {
    fn from_name(n: &str) -> Option<Kind> {
        Some(match n {
            "column" => Kind::Column,
            "row" => Kind::Row,
            "stack" => Kind::Stack,
            "for" => Kind::For,
            "if" => Kind::If,
            "text" => Kind::Text,
            "tag" => Kind::Tag,
            "bar" => Kind::Bar,
            "arc" | "ring" => Kind::Arc,
            "rect" => Kind::Rect,
            "circle" => Kind::Circle,
            "polygon" => Kind::Polygon,
            "path" => Kind::Path,
            "image" => Kind::Image,
            "pips" => Kind::Pips,
            "line" => Kind::Line,
            "spacer" => Kind::Spacer,
            _ => return None,
        })
    }

    pub fn is_container(self) -> bool {
        matches!(self, Kind::Column | Kind::Row | Kind::Stack | Kind::For | Kind::If)
    }

    /// 每种元素允许的属性（state 里也只能写这些）。
    pub fn attrs(self) -> &'static [&'static str] {
        let own: &[&str] = match self {
            Kind::Column | Kind::Row | Kind::Stack => &["gap", "align", "padding"],
            Kind::For => &["each", "limit", "gap", "direction", "align"],
            Kind::If => &["when"],
            Kind::Text => &["text", "font", "size", "color", "align", "spacing", "shadow"],
            Kind::Tag => &["text", "font", "size", "color", "fill", "shape", "cut", "radius", "padding"],
            Kind::Bar => &[
                "value", "ghost", "fill", "ghost-fill", "track", "kind", "shape", "segments", "gap", "empty", "caps", "skew",
                "radius", "edge", "marker", "marker-color",
            ],
            Kind::Arc => &["value", "radius", "thickness", "from", "to", "stroke", "track", "cap"],
            Kind::Rect => &["fill", "stroke", "stroke-width", "radius", "cut", "shape", "pattern", "density", "angle"],
            Kind::Circle => &["radius", "fill", "stroke", "stroke-width"],
            Kind::Polygon => &["points", "fill", "stroke", "stroke-width"],
            Kind::Path => &["d", "fill", "stroke", "stroke-width", "value", "track"],
            Kind::Image => &["src", "nine", "tint"],
            Kind::Pips => &["count", "filled", "size", "gap", "color"],
            Kind::Line => &["x1", "y1", "x2", "y2", "stroke", "stroke-width"],
            Kind::Spacer => &[],
        };
        own
    }

    pub fn common_attrs() -> &'static [&'static str] {
        &["opacity", "rotate", "scale", "skew", "flip", "glow", "visible", "x", "y", "width", "height"]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animation {
    Blink,
    Breathe,
    Pulse,
    Shift,
}

#[derive(Debug, Clone)]
pub struct State {
    pub when: Expr,
    pub attrs: BTreeMap<String, Text>,
    pub animate: Option<Animation>,
    pub period: f32,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: Kind,
    pub attrs: BTreeMap<String, Text>,
    pub states: Vec<State>,
    pub children: Vec<Node>,
    /// `for` 的变量名和列表
    pub each: Option<(String, Expr)>,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct Template {
    pub root: Node,
    pub nodes: usize,
}

struct Parser<'a, 'b> {
    file: &'a Path,
    doc: &'a roxmltree::Document<'b>,
    tokens: &'a Tokens,
    sample: serde_json::Value,
    nodes: usize,
    animations: usize,
    /// 当前可用的循环变量 -> 对应的列表路径
    vars: Vec<(String, Vec<String>)>,
}

impl Parser<'_, '_> {
    fn err(&self, node: roxmltree::Node, msg: impl Into<String>) -> ThemeError {
        ThemeError {
            file: self.file.to_path_buf(),
            line: self.doc.text_pos_at(node.range().start).row,
            message: msg.into(),
        }
    }

    /// 检查表达式里的字段：循环变量换成它对应的列表项，再到样例数据里找。
    fn check_expr(&self, node: roxmltree::Node, e: &Expr) -> Result<(), ThemeError> {
        let mut paths = Vec::new();
        expr::paths(e, &mut paths);
        for p in paths {
            let Some(first) = p.first() else { continue };
            let full: Vec<String> = match self.vars.iter().rev().find(|(v, _)| v == first) {
                Some((_, list)) => {
                    let mut f = list.clone();
                    f.push("[]".into());
                    f.extend(p[1..].iter().cloned());
                    f
                }
                None => p.clone(),
            };
            if full.len() == 1 && self.tokens.colors.contains_key(&full[0]) {
                continue;
            }
            if !fields::path_exists(&self.sample, &full) {
                return Err(self.err(node, format!("没有字段 {}", p.join(".").replace(".[]", "[]"))));
            }
        }
        Ok(())
    }

    fn attr_text(&self, node: roxmltree::Node, name: &str, value: &str) -> Result<Text, ThemeError> {
        let t = expr::parse_text(value).map_err(|e| self.err(node, format!("{name}：{e}")))?;
        for e in t.exprs() {
            self.check_expr(node, e)?;
        }
        Ok(t)
    }

    fn node(&mut self, n: roxmltree::Node) -> Result<Node, ThemeError> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(self.err(n, format!("模板超过 {MAX_NODES} 个节点")));
        }
        let name = n.tag_name().name();
        let kind = Kind::from_name(name).ok_or_else(|| self.err(n, format!("不认识的元素 <{name}>")))?;
        let mut attrs = BTreeMap::new();
        let mut each = None;
        for a in n.attributes() {
            let an = a.name();
            if kind == Kind::For && an == "each" {
                let (var, list) = a
                    .value()
                    .split_once(" in ")
                    .ok_or_else(|| self.err(n, "each 的写法是 \"变量 in 列表\""))?;
                let var = var.trim().to_string();
                let list_expr = expr::parse(list.trim()).map_err(|e| self.err(n, e))?;
                self.check_expr(n, &list_expr)?;
                each = Some((var, list_expr));
                continue;
            }
            if !kind.attrs().contains(&an) && !Kind::common_attrs().contains(&an) {
                return Err(self.err(n, format!("<{name}> 没有属性 {an}")));
            }
            attrs.insert(an.to_string(), self.attr_text(n, an, a.value())?);
        }
        if kind == Kind::For && each.is_none() {
            return Err(self.err(n, "<for> 缺少 each"));
        }
        if kind == Kind::If && !attrs.contains_key("when") {
            return Err(self.err(n, "<if> 缺少 when"));
        }

        // for 的变量只在它的子元素里可见
        if let Some((var, Expr::Path(segs))) = &each {
            let list: Vec<String> = segs
                .iter()
                .map(|s| match s {
                    expr::Seg::Key(k) => k.clone(),
                    expr::Seg::Index(_) => "[]".into(),
                })
                .collect();
            let resolved = match self.vars.iter().rev().find(|(v, _)| Some(v) == list.first()) {
                Some((_, base)) => {
                    let mut f = base.clone();
                    f.push("[]".into());
                    f.extend(list[1..].iter().cloned());
                    f
                }
                None => list,
            };
            self.vars.push((var.clone(), resolved));
        }

        let mut states = Vec::new();
        let mut children = Vec::new();
        for c in n.children().filter(|c| c.is_element()) {
            if c.tag_name().name() == "state" {
                let when = c.attribute("when").ok_or_else(|| self.err(c, "<state> 缺少 when"))?;
                let when = expr::parse(when).map_err(|e| self.err(c, e))?;
                self.check_expr(c, &when)?;
                let mut sattrs = BTreeMap::new();
                let mut animate = None;
                let mut period = 1.1;
                for a in c.attributes() {
                    match a.name() {
                        "when" => {}
                        "animate" => {
                            animate = Some(match a.value() {
                                "blink" => Animation::Blink,
                                "breathe" => Animation::Breathe,
                                "pulse" => Animation::Pulse,
                                "shift" => Animation::Shift,
                                v => return Err(self.err(c, format!("animate=\"{v}\" 应该是 blink、breathe、pulse 或 shift"))),
                            });
                            self.animations += 1;
                            if self.animations > MAX_ANIMATIONS {
                                return Err(self.err(c, format!("动画超过 {MAX_ANIMATIONS} 个")));
                            }
                        }
                        "period" => {
                            let ms: f32 = a.value().parse().map_err(|_| self.err(c, "period 要写毫秒数"))?;
                            period = (ms / 1000.0).clamp(0.2, 10.0);
                        }
                        an => {
                            if !kind.attrs().contains(&an) && !Kind::common_attrs().contains(&an) {
                                return Err(self.err(c, format!("<{name}> 的 state 里不能写 {an}")));
                            }
                            sattrs.insert(an.to_string(), self.attr_text(c, an, a.value())?);
                        }
                    }
                }
                states.push(State {
                    when,
                    attrs: sattrs,
                    animate,
                    period,
                });
            } else {
                if !kind.is_container() {
                    return Err(self.err(c, format!("<{name}> 里不能放 <{}>", c.tag_name().name())));
                }
                children.push(self.node(c)?);
            }
        }
        if each.is_some() {
            self.vars.pop();
        }
        Ok(Node {
            kind,
            attrs,
            states,
            children,
            each,
            line: self.doc.text_pos_at(n.range().start).row,
        })
    }
}

/// 解析 `<module id="...">` 下面的模板。只能有一个根元素。
pub fn parse_module(file: &Path, doc: &roxmltree::Document, module: roxmltree::Node, tokens: &Tokens) -> Result<Template, ThemeError> {
    let mut p = Parser {
        file,
        doc,
        tokens,
        sample: fields::sample(),
        nodes: 0,
        animations: 0,
        vars: Vec::new(),
    };
    let mut elems = module.children().filter(|c| c.is_element());
    let first = elems.next().ok_or_else(|| p.err(module, "<module> 里没有内容"))?;
    if let Some(extra) = elems.next() {
        return Err(p.err(extra, "<module> 里只能有一个根元素，用 <column> 或 <stack> 包起来"));
    }
    let root = p.node(first)?;
    Ok(Template { root, nodes: p.nodes })
}

/// 单独解析一段模板文本（插件交来的界面描述）。
pub fn parse_str(text: &str, tokens: &Tokens) -> Result<Template, ThemeError> {
    let file = Path::new("module.xml");
    let doc = roxmltree::Document::parse(text).map_err(|e| ThemeError {
        file: file.to_path_buf(),
        line: e.pos().row,
        message: format!("XML 格式错误：{e}"),
    })?;
    let root = doc.root_element();
    if root.tag_name().name() == "module" {
        parse_module(file, &doc, root, tokens)
    } else {
        let mut p = Parser {
            file,
            doc: &doc,
            tokens,
            sample: fields::sample(),
            nodes: 0,
            animations: 0,
            vars: Vec::new(),
        };
        let node = p.node(root)?;
        Ok(Template { root: node, nodes: p.nodes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Tokens {
        Tokens::defaults(true)
    }

    #[test]
    fn parses_sample_template() {
        let t = parse_str(
            r#"<module id="monster">
  <column gap="6">
    <row gap="10">
      <tag text="{monster.name}"/>
      <text text="{monster.hp_ratio | percent}" font="number" size="34"/>
    </row>
    <bar value="{monster.hp_ratio}" width="820">
      <state when="monster.enraged" fill="accent" animate="pulse"/>
    </bar>
    <for each="part in monster.parts" gap="4">
      <row><text text="{part.name}"/><bar value="{part.break_progress}" width="80" height="3"/></row>
    </for>
    <arc value="{monster.stamina_ratio}" radius="60" from="180" to="360" stroke="text"/>
  </column>
</module>"#,
            &tokens(),
        )
        .unwrap();
        assert_eq!(t.root.kind, Kind::Column);
        assert_eq!(t.root.children.len(), 4);
        assert_eq!(t.root.children[1].states.len(), 1);
        assert!(t.nodes >= 8);
    }

    #[test]
    fn rejects_unknown_field_with_line() {
        let e = parse_str("<column>\n<text text=\"{monster.hp_ratoi}\"/>\n</column>", &tokens()).unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.message.contains("monster.hp_ratoi"));
    }

    #[test]
    fn loop_variable_scoping() {
        assert!(parse_str("<for each=\"p in monster.parts\"><text text=\"{p.name}\"/></for>", &tokens()).is_ok());
        let e = parse_str("<for each=\"p in monster.parts\"><text text=\"{p.nope}\"/></for>", &tokens()).unwrap_err();
        assert!(e.message.contains("p.nope"));
        assert!(parse_str("<column><text text=\"{p.name}\"/></column>", &tokens()).is_err());
    }

    #[test]
    fn rejects_bad_structure() {
        assert!(parse_str("<text><text/></text>", &tokens()).is_err());
        assert!(parse_str("<column><blink/></column>", &tokens()).is_err());
        assert!(parse_str("<bar foo=\"1\"/>", &tokens()).is_err());
        assert!(parse_str("<for><text/></for>", &tokens()).is_err());
        assert!(parse_str("<bar><state when=\"monster.enraged\" animate=\"spin\"/></bar>", &tokens()).is_err());
    }

    #[test]
    fn token_names_are_allowed_in_expressions() {
        assert!(parse_str("<rect width=\"10\" height=\"10\" fill=\"{accent}\"/>", &tokens()).is_ok());
    }

    #[test]
    fn node_limit() {
        let many: String = (0..MAX_NODES + 1).map(|_| "<spacer/>").collect();
        let e = parse_str(&format!("<column>{many}</column>"), &tokens()).unwrap_err();
        assert!(e.message.contains("节点"));
    }
}
