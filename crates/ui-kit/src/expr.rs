//! 模板里的简单表达式：字段路径、数字和字符串、比较、逻辑、四则运算、
//! 过滤器（`| percent`、`| time` 等）。不支持循环和自定义函数。

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Value),
    Path(Vec<Seg>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Box<Expr>, Op, Box<Expr>),
    Filter(Box<Expr>, String, Option<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Key(String),
    Index(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

pub const FILTERS: &[&str] = &[
    "percent", "int", "fixed", "thousands", "time", "upper", "lower", "default", "abs", "clamp01",
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Sym(&'static str),
}

/// 前一个符号是 `.` 时数字是下标（`parts.0.name`），不能吃掉后面的点。
fn after_dot(out: &[Tok]) -> bool {
    matches!(out.last(), Some(Tok::Sym(".")))
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    const SYMS: [&str; 19] = [
        "||", "&&", "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "%", "!", "(", ")", "[", "]", "|",
    ];
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < cs.len()
                && (cs[i].is_ascii_digit()
                    || (cs[i] == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && !after_dot(&out)))
            {
                i += 1;
            }
            let t: String = cs[start..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("数字 {t} 无效"))?));
        } else if c == '\'' || c == '"' {
            let q = c;
            i += 1;
            let start = i;
            while i < cs.len() && cs[i] != q {
                i += 1;
            }
            if i >= cs.len() {
                return Err("字符串没有结束引号".into());
            }
            out.push(Tok::Str(cs[start..i].iter().collect()));
            i += 1;
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '-') {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            // XML 属性里写 && 和 < 要转义，所以给逻辑和比较各配一个单词写法
            let alias = match word.as_str() {
                _ if after_dot(&out) => None,
                "and" => Some("&&"),
                "or" => Some("||"),
                "not" => Some("!"),
                "lt" => Some("<"),
                "le" => Some("<="),
                "gt" => Some(">"),
                "ge" => Some(">="),
                _ => None,
            };
            out.push(match alias {
                Some(sym) => Tok::Sym(sym),
                None => Tok::Ident(word),
            });
        } else if c == '.' {
            out.push(Tok::Sym("."));
            i += 1;
        } else if c == ':' {
            out.push(Tok::Sym(":"));
            i += 1;
        } else {
            let rest: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            let sym = SYMS.iter().find(|s| rest.starts_with(**s)).ok_or_else(|| format!("不认识的符号 {c}"))?;
            out.push(Tok::Sym(sym));
            i += sym.chars().count();
        }
    }
    Ok(out)
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn eat(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Sym(x)) if *x == s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn pipeline(&mut self) -> Result<Expr, String> {
        let mut e = self.or()?;
        while self.eat("|") {
            let Some(Tok::Ident(name)) = self.peek().cloned() else {
                return Err("| 后面要写过滤器名".into());
            };
            self.i += 1;
            if !FILTERS.contains(&name.as_str()) {
                return Err(format!("没有过滤器 {name}"));
            }
            let arg = if self.eat(":") {
                Some(match self.peek().cloned() {
                    Some(Tok::Num(n)) => {
                        self.i += 1;
                        num(n)
                    }
                    Some(Tok::Str(s)) => {
                        self.i += 1;
                        Value::String(s)
                    }
                    _ => return Err(format!("过滤器 {name} 的参数要是数字或字符串")),
                })
            } else {
                None
            };
            e = Expr::Filter(Box::new(e), name, arg);
        }
        Ok(e)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut e = self.and()?;
        while self.eat("||") {
            e = Expr::Bin(Box::new(e), Op::Or, Box::new(self.and()?));
        }
        Ok(e)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut e = self.cmp()?;
        while self.eat("&&") {
            e = Expr::Bin(Box::new(e), Op::And, Box::new(self.cmp()?));
        }
        Ok(e)
    }

    fn cmp(&mut self) -> Result<Expr, String> {
        let e = self.add()?;
        for (s, op) in [("==", Op::Eq), ("!=", Op::Ne), ("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)] {
            if self.eat(s) {
                return Ok(Expr::Bin(Box::new(e), op, Box::new(self.add()?)));
            }
        }
        Ok(e)
    }

    fn add(&mut self) -> Result<Expr, String> {
        let mut e = self.mul()?;
        loop {
            if self.eat("+") {
                e = Expr::Bin(Box::new(e), Op::Add, Box::new(self.mul()?));
            } else if self.eat("-") {
                e = Expr::Bin(Box::new(e), Op::Sub, Box::new(self.mul()?));
            } else {
                return Ok(e);
            }
        }
    }

    fn mul(&mut self) -> Result<Expr, String> {
        let mut e = self.unary()?;
        loop {
            let op = if self.eat("*") {
                Op::Mul
            } else if self.eat("/") {
                Op::Div
            } else if self.eat("%") {
                Op::Rem
            } else {
                return Ok(e);
            };
            e = Expr::Bin(Box::new(e), op, Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Expr::Lit(num(n)))
            }
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(Expr::Lit(Value::String(s)))
            }
            Some(Tok::Ident(id)) => {
                self.i += 1;
                match id.as_str() {
                    "true" => return Ok(Expr::Lit(Value::Bool(true))),
                    "false" => return Ok(Expr::Lit(Value::Bool(false))),
                    "null" => return Ok(Expr::Lit(Value::Null)),
                    _ => {}
                }
                let mut segs = vec![Seg::Key(id)];
                loop {
                    if self.eat(".") {
                        match self.peek().cloned() {
                            Some(Tok::Ident(k)) => {
                                self.i += 1;
                                segs.push(Seg::Key(k));
                            }
                            Some(Tok::Num(n)) if n.fract() == 0.0 => {
                                self.i += 1;
                                segs.push(Seg::Index(Box::new(Expr::Lit(num(n)))));
                            }
                            _ => return Err(". 后面要写字段名".into()),
                        }
                    } else if self.eat("[") {
                        let e = self.or()?;
                        if !self.eat("]") {
                            return Err("缺少 ]".into());
                        }
                        segs.push(Seg::Index(Box::new(e)));
                    } else {
                        break;
                    }
                }
                Ok(Expr::Path(segs))
            }
            Some(Tok::Sym("(")) => {
                self.i += 1;
                let e = self.or()?;
                if !self.eat(")") {
                    return Err("缺少 )".into());
                }
                Ok(e)
            }
            Some(t) => Err(format!("这里不能出现 {t:?}")),
            None => Err("表达式不完整".into()),
        }
    }
}

fn num(n: f64) -> Value {
    serde_json::Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
}

pub fn parse(s: &str) -> Result<Expr, String> {
    let mut p = P { t: lex(s)?, i: 0 };
    let e = p.pipeline()?;
    if p.i != p.t.len() {
        return Err(format!("表达式 {s} 后面多了内容"));
    }
    Ok(e)
}

/// 变量作用域：模板里的 `for` 会加一层。
pub trait Scope {
    fn get(&self, name: &str) -> Option<&Value>;
}

impl Scope for Value {
    fn get(&self, name: &str) -> Option<&Value> {
        self.as_object()?.get(name)
    }
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

pub fn as_f64(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::Bool(b) => *b as i32 as f64,
        Value::String(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

pub fn to_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(0.0);
            if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x:.2}") }
        }
        other => other.to_string(),
    }
}

fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn eval(e: &Expr, scope: &dyn Scope) -> Value {
    match e {
        Expr::Lit(v) => v.clone(),
        Expr::Path(segs) => {
            let Some(Seg::Key(first)) = segs.first() else { return Value::Null };
            let mut cur = match scope.get(first) {
                Some(v) => v,
                None => return Value::Null,
            };
            for s in &segs[1..] {
                let next = match s {
                    Seg::Key(k) => cur.get(k.as_str()),
                    Seg::Index(i) => {
                        let idx = eval(i, scope);
                        match &idx {
                            Value::String(k) => cur.get(k.as_str()),
                            other => cur.get(as_f64(other) as usize),
                        }
                    }
                };
                match next {
                    Some(v) => cur = v,
                    None => return Value::Null,
                }
            }
            cur.clone()
        }
        Expr::Not(x) => Value::Bool(!truthy(&eval(x, scope))),
        Expr::Neg(x) => num(-as_f64(&eval(x, scope))),
        Expr::Bin(a, op, b) => {
            let va = eval(a, scope);
            match op {
                Op::Or => {
                    return if truthy(&va) { va } else { eval(b, scope) };
                }
                Op::And => {
                    return if truthy(&va) { eval(b, scope) } else { va };
                }
                _ => {}
            }
            let vb = eval(b, scope);
            let (x, y) = (as_f64(&va), as_f64(&vb));
            match op {
                Op::Eq => Value::Bool(eq(&va, &vb)),
                Op::Ne => Value::Bool(!eq(&va, &vb)),
                Op::Lt => Value::Bool(x < y),
                Op::Le => Value::Bool(x <= y),
                Op::Gt => Value::Bool(x > y),
                Op::Ge => Value::Bool(x >= y),
                Op::Add => {
                    if va.is_string() || vb.is_string() {
                        Value::String(to_text(&va) + &to_text(&vb))
                    } else {
                        num(x + y)
                    }
                }
                Op::Sub => num(x - y),
                Op::Mul => num(x * y),
                Op::Div => num(if y == 0.0 { 0.0 } else { x / y }),
                Op::Rem => num(if y == 0.0 { 0.0 } else { x % y }),
                Op::Or | Op::And => unreachable!(),
            }
        }
        Expr::Filter(x, name, arg) => {
            let v = eval(x, scope);
            let f = as_f64(&v);
            match name.as_str() {
                "percent" => Value::String(format!("{}%", (f * 100.0).round() as i64)),
                "int" => num(f.round()),
                "fixed" => {
                    let d = arg.as_ref().map(as_f64).unwrap_or(1.0).clamp(0.0, 6.0) as usize;
                    Value::String(format!("{f:.d$}"))
                }
                "thousands" => Value::String(thousands(f.round() as i64)),
                "time" => {
                    let s = f.max(0.0) as i64;
                    Value::String(if s >= 3600 {
                        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
                    } else {
                        format!("{}:{:02}", s / 60, s % 60)
                    })
                }
                "upper" => Value::String(to_text(&v).to_uppercase()),
                "lower" => Value::String(to_text(&v).to_lowercase()),
                "default" => {
                    if truthy(&v) {
                        v
                    } else {
                        arg.clone().unwrap_or(Value::Null)
                    }
                }
                "abs" => num(f.abs()),
                "clamp01" => num(f.clamp(0.0, 1.0)),
                _ => v,
            }
        }
    }
}

fn eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), _) | (_, Value::Number(_)) => (as_f64(a) - as_f64(b)).abs() < 1e-9,
        _ => a == b,
    }
}

/// 表达式里引用到的字段路径，例如 `monster.parts`。下标部分不展开。
pub fn paths(e: &Expr, out: &mut Vec<Vec<String>>) {
    match e {
        Expr::Lit(_) => {}
        Expr::Path(segs) => {
            let mut p = Vec::new();
            for s in segs {
                match s {
                    Seg::Key(k) => p.push(k.clone()),
                    Seg::Index(i) => {
                        paths(i, out);
                        p.push("[]".into());
                    }
                }
            }
            out.push(p);
        }
        Expr::Not(x) | Expr::Neg(x) | Expr::Filter(x, _, _) => paths(x, out),
        Expr::Bin(a, _, b) => {
            paths(a, out);
            paths(b, out);
        }
    }
}

/// 属性里的文字：可以整体是 `{表达式}`，也可以是夹着表达式的字符串，`{{` 表示 `{`。
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Lit(String),
    Expr(Expr),
    Mixed(Vec<Piece>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Lit(String),
    Expr(Expr),
}

pub fn parse_text(s: &str) -> Result<Text, String> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                let mut inner = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    inner.push(c);
                }
                if !closed {
                    return Err(format!("{s} 里的 {{ 没有对应的 }}"));
                }
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                }
                pieces.push(Piece::Expr(parse(&inner)?));
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        pieces.push(Piece::Lit(lit));
    }
    Ok(match pieces.len() {
        0 => Text::Lit(String::new()),
        1 => match pieces.pop().unwrap() {
            Piece::Lit(l) => Text::Lit(l),
            Piece::Expr(e) => Text::Expr(e),
        },
        _ => Text::Mixed(pieces),
    })
}

impl Text {
    pub fn eval(&self, scope: &dyn Scope) -> Value {
        match self {
            Text::Lit(s) => Value::String(s.clone()),
            Text::Expr(e) => eval(e, scope),
            Text::Mixed(ps) => Value::String(
                ps.iter()
                    .map(|p| match p {
                        Piece::Lit(s) => s.clone(),
                        Piece::Expr(e) => to_text(&eval(e, scope)),
                    })
                    .collect(),
            ),
        }
    }

    pub fn exprs(&self) -> Vec<&Expr> {
        match self {
            Text::Lit(_) => vec![],
            Text::Expr(e) => vec![e],
            Text::Mixed(ps) => ps
                .iter()
                .filter_map(|p| match p {
                    Piece::Expr(e) => Some(e),
                    Piece::Lit(_) => None,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(s: &str, v: &Value) -> Value {
        eval(&parse(s).unwrap(), v)
    }

    #[test]
    fn paths_and_arithmetic() {
        let v = json!({"monster": {"hp": 50, "hp_max": 200, "parts": [{"name": "头部"}, {"name": "尾巴"}]}, "i": 1});
        assert_eq!(as_f64(&ev("monster.hp / monster.hp_max", &v)), 0.25);
        assert_eq!(ev("monster.parts[i].name", &v), json!("尾巴"));
        assert_eq!(ev("monster.parts.0.name", &v), json!("头部"));
        assert_eq!(ev("monster.missing.x", &v), Value::Null);
        assert_eq!(as_f64(&ev("-(1 + 2) * 3 % 4", &v)), -1.0);
    }

    #[test]
    fn comparison_and_logic() {
        let v = json!({"m": {"enraged": true, "hp_ratio": 0.15, "name": "火龙"}});
        assert_eq!(ev("m.enraged && m.hp_ratio < 0.2", &v), json!(true));
        assert_eq!(ev("!m.enraged || m.name == '火龙'", &v), json!(true));
        assert_eq!(ev("m.hp_ratio >= 0.5", &v), json!(false));
    }

    #[test]
    fn word_operators() {
        let v = json!({"a": 3, "b": true, "c": false, "m": {"and": 1}});
        assert_eq!(ev("a gt 2 and b", &v), json!(true));
        assert_eq!(ev("not b or c", &v), json!(false));
        assert_eq!(ev("a le 3", &v), json!(true));
        assert_eq!(ev("m.and", &v), json!(1));
    }

    #[test]
    fn filters() {
        let v = json!({"r": 0.634, "t": 754.0, "d": 1234567.4, "z": 0});
        assert_eq!(ev("r | percent", &v), json!("63%"));
        assert_eq!(ev("t | time", &v), json!("12:34"));
        assert_eq!(ev("d | thousands", &v), json!("1,234,567"));
        assert_eq!(ev("r | fixed:2", &v), json!("0.63"));
        assert_eq!(ev("z | default:'—'", &v), json!("—"));
        assert!(parse("r | nope").is_err());
    }

    #[test]
    fn text_interpolation() {
        let v = json!({"a": 3, "b": 10});
        assert_eq!(parse_text("{a} / {b} 击").unwrap().eval(&v), json!("3 / 10 击"));
        assert_eq!(parse_text("{a}").unwrap().eval(&v), json!(3));
        assert_eq!(parse_text("{{lit}}").unwrap().eval(&v), json!("{lit}"));
        assert!(parse_text("{a").is_err());
    }

    #[test]
    fn collects_paths() {
        let mut p = Vec::new();
        paths(&parse("monster.parts[i].name | upper").unwrap(), &mut p);
        assert!(p.contains(&vec!["monster".into(), "parts".into(), "[]".into(), "name".into()]));
        assert!(p.contains(&vec!["i".into()]));
    }

    #[test]
    fn errors() {
        assert!(parse("a +").is_err());
        assert!(parse("(a").is_err());
        assert!(parse("a b").is_err());
        assert!(parse("'abc").is_err());
    }
}
