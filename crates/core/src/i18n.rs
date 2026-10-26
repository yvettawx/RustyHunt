//! 界面文案和游戏术语。
//!
//! - 界面文案：`locales/ui/<语言>.toml`，键是 ASCII，值是文案。
//! - 游戏术语：`locales/terms/world.toml`（怪物、地图，来自游戏内文本）和
//!   `locales/game/<语言>.toml`（部位、异常、增益等）。
//!
//! 某种语言缺一条时退回简体中文，再退回键本身。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Lang {
    #[default]
    #[serde(rename = "zh-Hans")]
    ZhHans,
    #[serde(rename = "zh-Hant")]
    ZhHant,
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "en")]
    En,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::ZhHans, Lang::ZhHant, Lang::Ja, Lang::En];

    pub fn code(self) -> &'static str {
        match self {
            Lang::ZhHans => "zh-Hans",
            Lang::ZhHant => "zh-Hant",
            Lang::Ja => "ja",
            Lang::En => "en",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::ZhHans => "简体中文",
            Lang::ZhHant => "繁體中文",
            Lang::Ja => "日本語",
            Lang::En => "English",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(code))
    }
}

/// 一种语言的键值表。
#[derive(Debug, Default, Clone)]
pub struct Table(HashMap<String, String>);

impl Table {
    /// 把嵌套的 TOML 展平成 `a.b.c = "..."`。
    pub fn parse(text: &str) -> Result<Table, toml::de::Error> {
        let value: toml::Table = toml::from_str(text)?;
        let mut map = HashMap::new();
        flatten("", &toml::Value::Table(value), &mut map);
        Ok(Table(map))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    pub fn merge(&mut self, other: Table) {
        self.0.extend(other.0);
    }
}

fn flatten(prefix: &str, v: &toml::Value, out: &mut HashMap<String, String>) {
    match v {
        toml::Value::Table(t) => {
            for (k, v) in t {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, v, out);
            }
        }
        toml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

/// 一个游戏的术语表：怪物、地图、设施，按键查四种语言。
#[derive(Debug, Default, Clone)]
pub struct Terms {
    /// 键 -> (语言 -> 名字)
    entries: HashMap<String, HashMap<Lang, String>>,
    /// 怪物键 -> 是否大型
    large: HashMap<String, bool>,
}

impl Terms {
    pub fn parse(text: &str) -> Result<Terms, toml::de::Error> {
        let root: toml::Table = toml::from_str(text)?;
        let mut terms = Terms::default();
        for (section, entries) in &root {
            let Some(entries) = entries.as_table() else { continue };
            for (key, e) in entries {
                let Some(e) = e.as_table() else { continue };
                let full = format!("{section}.{key}");
                let mut names = HashMap::new();
                for lang in Lang::ALL {
                    if let Some(s) = e.get(lang.code()).and_then(|v| v.as_str()) {
                        names.insert(lang, s.to_string());
                    }
                }
                if section == "monster" {
                    terms.large.insert(key.clone(), e.get("size").and_then(|v| v.as_str()) == Some("large"));
                }
                terms.entries.insert(full, names);
            }
        }
        Ok(terms)
    }

    pub fn get(&self, full_key: &str, lang: Lang) -> Option<&str> {
        let names = self.entries.get(full_key)?;
        names.get(&lang).or_else(|| names.get(&Lang::En)).map(String::as_str)
    }

    pub fn is_large(&self, monster_key: &str) -> Option<bool> {
        self.large.get(monster_key).copied()
    }

    pub fn keys(&self, section: &str) -> Vec<String> {
        let head = format!("{section}.");
        let mut v: Vec<String> = self.entries.keys().filter_map(|k| k.strip_prefix(&head).map(str::to_string)).collect();
        v.sort();
        v
    }

    /// 按英文名找键，用来把游戏内部编号对到术语表。
    pub fn find_by_english(&self, section: &str, english: &str) -> Option<String> {
        let head = format!("{section}.");
        self.entries.iter().find_map(|(k, names)| {
            let key = k.strip_prefix(&head)?;
            names.get(&Lang::En).filter(|n| n.eq_ignore_ascii_case(english)).map(|_| key.to_string())
        })
    }
}

#[derive(Debug, Default)]
pub struct I18n {
    pub ui_lang: Lang,
    /// 游戏术语用的语言；None 表示跟随界面语言
    pub term_lang: Option<Lang>,
    ui: HashMap<Lang, Table>,
    game: HashMap<Lang, Table>,
    terms: Terms,
}

impl I18n {
    pub fn term_lang(&self) -> Lang {
        self.term_lang.unwrap_or(self.ui_lang)
    }

    pub fn load_ui(&mut self, lang: Lang, table: Table) {
        self.ui.entry(lang).or_default().merge(table);
    }

    /// 游戏字符串按 `[part]`、`[ailment]` 等分节存放，查的时候不带节名，
    /// 所以载入时同时按原键存一份。
    pub fn load_game(&mut self, lang: Lang, table: Table) {
        let bare: HashMap<String, String> = table
            .0
            .iter()
            .filter_map(|(k, v)| k.split_once('.').map(|(_, rest)| (rest.to_string(), v.clone())))
            .collect();
        let t = self.game.entry(lang).or_default();
        t.0.extend(bare);
        t.merge(table);
    }

    pub fn set_terms(&mut self, terms: Terms) {
        self.terms = terms;
    }

    pub fn terms(&self) -> &Terms {
        &self.terms
    }

    pub fn ui_coverage(&self, lang: Lang) -> f32 {
        let base = self.ui.get(&Lang::ZhHans).map_or(0, Table::len).max(1);
        let have = self
            .ui
            .get(&lang)
            .map_or(0, |t| self.ui[&Lang::ZhHans].keys().filter(|k| t.get(k).is_some()).count());
        have as f32 / base as f32
    }

    pub fn t(&self, key: &str) -> String {
        self.ui
            .get(&self.ui_lang)
            .and_then(|t| t.get(key))
            .or_else(|| self.ui.get(&Lang::ZhHans).and_then(|t| t.get(key)))
            .unwrap_or(key)
            .to_string()
    }

    /// 带参数的文案，`{name}` 换成对应的值。
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.t(key);
        for (k, v) in args {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// 部位、异常、增益等游戏字符串。
    pub fn game(&self, key: &str) -> String {
        let lang = self.term_lang();
        self.game
            .get(&lang)
            .and_then(|t| t.get(key))
            .or_else(|| self.game.get(&Lang::ZhHans).and_then(|t| t.get(key)))
            .or_else(|| self.game.get(&Lang::En).and_then(|t| t.get(key)))
            .unwrap_or(key)
            .to_string()
    }

    pub fn monster(&self, key: &str) -> String {
        self.terms
            .get(&format!("monster.{key}"), self.term_lang())
            .map(str::to_string)
            .unwrap_or_else(|| self.game(&format!("MONSTER_{key}")))
    }

    pub fn monster_in(&self, key: &str, lang: Lang) -> String {
        self.terms.get(&format!("monster.{key}"), lang).map(str::to_string).unwrap_or_default()
    }

    pub fn map(&self, key: &str) -> String {
        self.terms
            .get(&format!("map.{key}"), self.term_lang())
            .map(str::to_string)
            .unwrap_or_else(|| key.to_string())
    }

    pub fn facility(&self, key: &str) -> String {
        self.terms
            .get(&format!("facility.{key}"), self.term_lang())
            .map(str::to_string)
            .unwrap_or_else(|| self.t(&format!("facility.{key}")))
    }
}

static GLOBAL: RwLock<Option<Arc<I18n>>> = RwLock::new(None);

pub fn install(i18n: I18n) {
    *GLOBAL.write().unwrap() = Some(Arc::new(i18n));
}

pub fn current() -> Arc<I18n> {
    GLOBAL.read().unwrap().clone().unwrap_or_else(|| Arc::new(I18n::default()))
}

pub fn t(key: &str) -> String {
    current().t(key)
}

pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    current().tf(key, args)
}

/// 程序自带的四种语言和术语表。
pub fn load_bundled(dir: &Path, ui_lang: Lang, term_lang: Option<Lang>) -> I18n {
    let mut i = I18n {
        ui_lang,
        term_lang,
        ..Default::default()
    };
    for lang in Lang::ALL {
        if let Ok(text) = std::fs::read_to_string(dir.join("ui").join(format!("{}.toml", lang.code())))
            && let Ok(t) = Table::parse(&text)
        {
            i.load_ui(lang, t);
        }
        if let Ok(text) = std::fs::read_to_string(dir.join("game").join(format!("{}.toml", lang.code())))
            && let Ok(t) = Table::parse(&text)
        {
            i.load_game(lang, t);
        }
    }
    if let Ok(text) = std::fs::read_to_string(dir.join("terms").join("world.toml"))
        && let Ok(t) = Terms::parse(&text)
    {
        i.set_terms(t);
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_and_falls_back() {
        let mut i = I18n::default();
        i.load_ui(Lang::ZhHans, Table::parse("[nav]\nhome = \"总览\"\nlog = \"日志\"").unwrap());
        i.load_ui(Lang::En, Table::parse("[nav]\nhome = \"Overview\"").unwrap());
        i.ui_lang = Lang::En;
        assert_eq!(i.t("nav.home"), "Overview");
        assert_eq!(i.t("nav.log"), "日志");
        assert_eq!(i.t("nav.none"), "nav.none");
        assert_eq!(i.ui_coverage(Lang::En), 0.5);
    }

    #[test]
    fn formats_arguments() {
        let mut i = I18n::default();
        i.load_ui(Lang::ZhHans, Table::parse("hits = \"{n} 击\"").unwrap());
        assert_eq!(i.tf("hits", &[("n", "12")]), "12 击");
    }

    #[test]
    fn terms_by_language() {
        let text = r#"
[monster.rathalos]
size = "large"
"zh-Hans" = "火龙"
"en" = "Rathalos"

[map.ancient_forest]
"zh-Hans" = "古代树森林"
"en" = "Ancient Forest"
"#;
        let t = Terms::parse(text).unwrap();
        assert_eq!(t.get("monster.rathalos", Lang::ZhHans), Some("火龙"));
        assert_eq!(t.get("monster.rathalos", Lang::Ja), Some("Rathalos"));
        assert_eq!(t.is_large("rathalos"), Some(true));
        assert_eq!(t.find_by_english("monster", "rathalos"), Some("rathalos".into()));
        assert_eq!(t.keys("map"), vec!["ancient_forest".to_string()]);
    }

    #[test]
    fn bundled_world_terms_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
        let t = Terms::parse(&std::fs::read_to_string(dir.join("terms/world.toml")).unwrap()).unwrap();
        assert!(t.keys("monster").len() > 90);
        assert_eq!(t.get("monster.rathalos", Lang::ZhHans), Some("火龙"));
    }

    #[test]
    fn bundled_ui_languages_have_the_same_keys() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales/ui");
        let keys = |l: Lang| {
            let t = Table::parse(&std::fs::read_to_string(dir.join(format!("{}.toml", l.code()))).unwrap()).unwrap();
            let mut k: Vec<String> = t.keys().cloned().collect();
            k.sort();
            k
        };
        let base = keys(Lang::ZhHans);
        for l in [Lang::ZhHant, Lang::Ja, Lang::En] {
            let k = keys(l);
            let missing: Vec<&String> = base.iter().filter(|x| !k.contains(x)).collect();
            let extra: Vec<&String> = k.iter().filter(|x| !base.contains(x)).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{}: 缺 {missing:?}，多 {extra:?}", l.code());
        }
    }
}
