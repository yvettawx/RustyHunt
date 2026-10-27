//! Fonts: numbers use the bundled Montserrat (Latin only, about 140 KB), Chinese uses the system's
//! Microsoft YaHei, and missing Japanese glyphs fall back to Yu Gothic. System fonts are memory-mapped,
//! so only the glyphs actually used end up in memory.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};

pub const NUM: &str = "num";
pub const NUM_HEAVY: &str = "num-heavy";
pub const NUM_BOLD: &str = "num-bold";
pub const TEXT: &str = "text";
pub const TEXT_BOLD: &str = "text-bold";
pub const MONO: &str = "mono";

static MONTSERRAT_800: &[u8] = include_bytes!("../../../assets/fonts/Montserrat-ExtraBold.ttf");
static MONTSERRAT_900: &[u8] = include_bytes!("../../../assets/fonts/Montserrat-Black.ttf");
static MONTSERRAT_700: &[u8] = include_bytes!("../../../assets/fonts/Montserrat-Bold.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    /// Montserrat 800
    Num,
    /// Montserrat 900
    NumHeavy,
    /// Montserrat 700
    NumBold,
    Text,
    TextBold,
    Mono,
}

impl Font {
    pub fn family(self) -> FontFamily {
        FontFamily::Name(
            match self {
                Font::Num => NUM,
                Font::NumHeavy => NUM_HEAVY,
                Font::NumBold => NUM_BOLD,
                Font::Text => TEXT,
                Font::TextBold => TEXT_BOLD,
                Font::Mono => MONO,
            }
            .into(),
        )
    }

    pub fn id(self, size: f32) -> FontId {
        FontId::new(size, self.family())
    }

    /// Which font a theme's family and weight map to.
    pub fn from_token(family: &str, weight: u16) -> Font {
        let num = family.to_ascii_lowercase().contains("montserrat") || family == "number";
        match (num, weight) {
            (true, w) if w >= 900 => Font::NumHeavy,
            (true, w) if w >= 800 => Font::Num,
            (true, _) => Font::NumBold,
            (false, w) if w >= 600 => Font::TextBold,
            _ => Font::Text,
        }
    }
}

struct Mapped(memmap2::Mmap);

impl AsRef<[u8]> for Mapped {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn map_font(path: &Path) -> Option<Arc<dyn AsRef<[u8]> + Send + Sync>> {
    let file = std::fs::File::open(path).ok()?;
    // Files in the system font directory aren't rewritten at runtime
    let map = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    Some(Arc::new(Mapped(map)))
}

fn windows_fonts() -> std::path::PathBuf {
    std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("Fonts")
}

/// Fonts bundled with a theme: name -> file contents.
pub type ExtraFonts = BTreeMap<String, Vec<u8>>;

pub fn definitions(extra: &ExtraFonts) -> FontDefinitions {
    let mut d = FontDefinitions::default();
    let mut add = |name: &str, data: FontData| {
        d.font_data.insert(name.to_string(), Arc::new(data));
    };
    add("montserrat-800", FontData::from_static(MONTSERRAT_800));
    add("montserrat-900", FontData::from_static(MONTSERRAT_900));
    add("montserrat-700", FontData::from_static(MONTSERRAT_700));

    let dir = windows_fonts();
    let mut cjk = Vec::new();
    let mut cjk_bold = Vec::new();
    for (name, file, bold) in [
        ("yahei", "msyh.ttc", false),
        ("yahei-bold", "msyhbd.ttc", true),
        ("yugothic", "YuGothM.ttc", false),
        ("yugothic-bold", "YuGothB.ttc", true),
        ("simsun", "simsun.ttc", false),
    ] {
        if let Some(blob) = map_font(&dir.join(file)) {
            add(name, FontData::from_blob(blob, 0));
            if bold {
                cjk_bold.push(name.to_string());
            } else {
                cjk.push(name.to_string());
            }
        }
    }
    let mut mono = Vec::new();
    if let Some(blob) = map_font(&dir.join("consola.ttf")) {
        add("consolas", FontData::from_blob(blob, 0));
        mono.push("consolas".to_string());
    }
    let mut themed = Vec::new();
    for (name, bytes) in extra {
        add(&format!("theme-{name}"), FontData::from_owned(bytes.clone()));
        themed.push(format!("theme-{name}"));
    }

    // Missing bold glyphs fall back to the regular face, then to egui's built-in font
    cjk_bold.extend(cjk.iter().cloned());
    let builtin: Vec<String> = d.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let chain = |first: &[&str], rest: &[String]| -> Vec<String> {
        let mut v: Vec<String> = themed.clone();
        v.extend(first.iter().map(|s| s.to_string()));
        v.extend(rest.iter().cloned());
        v.extend(builtin.iter().cloned());
        v
    };
    let text = chain(&[], &cjk);
    let text_bold = chain(&[], &cjk_bold);
    d.families.insert(Font::Text.family(), text.clone());
    d.families.insert(Font::TextBold.family(), text_bold.clone());
    d.families.insert(Font::Num.family(), chain(&["montserrat-800"], &cjk_bold));
    d.families.insert(Font::NumHeavy.family(), chain(&["montserrat-900"], &cjk_bold));
    d.families.insert(Font::NumBold.family(), chain(&["montserrat-700"], &cjk_bold));
    let mut m = mono.clone();
    m.extend(cjk.iter().cloned());
    m.extend(d.families.get(&FontFamily::Monospace).cloned().unwrap_or_default());
    d.families.insert(Font::Mono.family(), m.clone());
    d.families.insert(FontFamily::Proportional, text);
    d.families.insert(FontFamily::Monospace, m);
    d
}

pub fn install(ctx: &egui::Context, extra: &ExtraFonts) {
    ctx.set_fonts(definitions(extra));
}
