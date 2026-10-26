//! 设置，存成 `settings.toml`。每个游戏单独一套布局方案。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::model::Game;
use crate::modules::{self, ModuleInfo, Placement};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Full,
    #[default]
    Standard,
    Minimal,
}

impl Density {
    pub fn next(self) -> Density {
        match self {
            Density::Full => Density::Standard,
            Density::Standard => Density::Minimal,
            Density::Minimal => Density::Full,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Density::Full => "density.full",
            Density::Standard => "density.standard",
            Density::Minimal => "density.minimal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MonsterLayout {
    #[default]
    Bottom,
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetMode {
    #[default]
    Lock,
    LastHit,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backing {
    None,
    Solid,
    #[default]
    Blur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModuleConfig {
    pub enabled: bool,
    pub placement: Placement,
    pub scale: f32,
    pub opacity: f32,
    pub hide_in_menu: bool,
    pub quest_only: bool,
    /// 关掉的元素
    pub off: BTreeSet<String>,
}

impl Default for ModuleConfig {
    fn default() -> Self {
        ModuleConfig {
            enabled: true,
            placement: Placement::top_left(200.0, 200.0),
            scale: 1.0,
            opacity: 1.0,
            hide_in_menu: true,
            quest_only: true,
            off: BTreeSet::new(),
        }
    }
}

impl ModuleConfig {
    pub fn shows(&self, element: &str) -> bool {
        !self.off.contains(element)
    }

    pub fn toggle(&mut self, element: &str) {
        if !self.off.remove(element) {
            self.off.insert(element.to_string());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonsterOptions {
    pub layout: MonsterLayout,
    pub idle_fade_secs: f32,
    pub target_mode: TargetMode,
}

impl Default for MonsterOptions {
    fn default() -> Self {
        MonsterOptions {
            layout: MonsterLayout::Bottom,
            idle_fade_secs: 4.0,
            target_mode: TargetMode::Lock,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerOptions {
    /// 低血阈值，百分比
    pub low_hp: f32,
    pub backing: Backing,
    pub blur: f32,
    /// 底板不透明度 0..1
    pub backing_opacity: f32,
    pub backing_w: f32,
    pub backing_h: f32,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        PlayerOptions {
            low_hp: 30.0,
            backing: Backing::Blur,
            blur: 12.0,
            backing_opacity: 0.55,
            backing_w: 1.0,
            backing_h: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Tray {
    pub name: String,
    /// 增益编号；空表示显示全部
    pub ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuffOptions {
    pub trays: Vec<Tray>,
    /// 剩余秒数低于它时变色
    pub low_secs: f32,
}

impl Default for BuffOptions {
    fn default() -> Self {
        BuffOptions {
            trays: vec![Tray {
                name: "1".into(),
                ids: BTreeSet::new(),
            }],
            low_secs: 10.0,
        }
    }
}

/// 一个布局方案：每个模块的开关、位置、显示设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Profile {
    pub modules: BTreeMap<String, ModuleConfig>,
    pub monster: MonsterOptions,
    pub player: PlayerOptions,
    pub buffs: BuffOptions,
}

impl Profile {
    pub fn new_default(game: Game) -> Profile {
        let mut p = Profile::default();
        for m in modules::builtin() {
            p.ensure(game, &m);
        }
        p
    }

    /// 补上缺的模块（新版本加了模块，或者装了插件）。
    pub fn ensure(&mut self, game: Game, info: &ModuleInfo) {
        let side = self.monster.layout == MonsterLayout::Side;
        self.modules.entry(info.id.clone()).or_insert_with(|| ModuleConfig {
            enabled: info.default_on,
            placement: modules::default_placement(game, &info.id, side),
            off: info
                .elements
                .iter()
                .filter(|e| !e.default_on)
                .map(|e| e.key.clone())
                .collect(),
            ..Default::default()
        });
    }

    pub fn module(&self, id: &str) -> Option<&ModuleConfig> {
        self.modules.get(id)
    }

    pub fn module_mut(&mut self, id: &str) -> &mut ModuleConfig {
        self.modules.entry(id.to_string()).or_default()
    }

    pub fn reset_module(&mut self, game: Game, id: &str) {
        let side = self.monster.layout == MonsterLayout::Side;
        if let Some(m) = self.modules.get_mut(id) {
            m.placement = modules::default_placement(game, id, side);
            m.scale = 1.0;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameLayouts {
    pub active: String,
    pub profiles: BTreeMap<String, Profile>,
}

impl GameLayouts {
    pub fn new(game: Game) -> GameLayouts {
        let mut profiles = BTreeMap::new();
        profiles.insert(DEFAULT_PROFILE.to_string(), Profile::new_default(game));
        GameLayouts {
            active: DEFAULT_PROFILE.to_string(),
            profiles,
        }
    }

    pub fn active(&self) -> &Profile {
        self.profiles
            .get(&self.active)
            .or_else(|| self.profiles.values().next())
            .expect("至少有一个方案")
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        if !self.profiles.contains_key(&self.active) {
            self.active = self.profiles.keys().next().cloned().unwrap_or_else(|| DEFAULT_PROFILE.into());
        }
        self.profiles.entry(self.active.clone()).or_default()
    }
}

impl Default for GameLayouts {
    fn default() -> Self {
        GameLayouts::new(Game::World)
    }
}

pub const DEFAULT_PROFILE: &str = "默认";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle_overlay: String,
    pub layout_mode: String,
    pub density: String,
    pub show_all: String,
    pub cycle_target: String,
    /// 插件注册的动作：插件编号.动作编号 -> 热键
    pub plugin: BTreeMap<String, String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys {
            toggle_overlay: "Ctrl Shift H".into(),
            layout_mode: "Ctrl Shift L".into(),
            density: "Ctrl Shift D".into(),
            show_all: "Left Alt".into(),
            cycle_target: "Ctrl Tab".into(),
            plugin: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    pub toast: bool,
    pub sound: bool,
    pub in_game: bool,
    pub update: bool,
    pub map_mismatch: bool,
    pub crown: bool,
    pub disconnect: bool,
    /// 插件注册的通知渠道
    pub channels: BTreeMap<String, bool>,
}

impl Default for Notify {
    fn default() -> Self {
        Notify {
            toast: true,
            sound: false,
            in_game: true,
            update: true,
            map_mismatch: true,
            crown: true,
            disconnect: false,
            channels: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Injection {
    pub crc_patch: bool,
    pub damage_hook: bool,
    pub solo_only: bool,
}

impl Default for Injection {
    fn default() -> Self {
        Injection {
            crc_patch: false,
            damage_hook: false,
            solo_only: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Perf {
    pub read_hz: u32,
    pub fps_cap: u32,
    pub animations: bool,
}

impl Default for Perf {
    fn default() -> Self {
        Perf {
            read_hz: 20,
            fps_cap: 60,
            animations: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PluginState {
    pub enabled: bool,
    pub granted: BTreeSet<String>,
    pub settings: BTreeMap<String, toml::Value>,
    /// 安装时确认过的权限，权限变化时要重新确认
    pub confirmed: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Update {
    pub check_on_start: bool,
    /// GitHub 仓库，形如 owner/name
    pub repo: String,
    pub skipped: Option<String>,
}

impl Default for Update {
    fn default() -> Self {
        Update {
            check_on_start: true,
            repo: crate::DEFAULT_REPO.to_string(),
            skipped: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub first_run_done: bool,
    pub language: Lang,
    pub term_language: Option<Lang>,
    pub autostart: bool,
    pub close_to_tray: bool,
    pub auto_connect: bool,
    pub overlay_visible: bool,
    pub density: Density,
    pub perf: Perf,
    pub log_level: LogLevel,
    pub hotkeys: Hotkeys,
    pub notify: Notify,
    pub injection: Injection,
    pub theme: String,
    pub theme_follow_system: bool,
    pub save_account: Option<u32>,
    pub games: BTreeMap<String, GameLayouts>,
    pub game_paths: BTreeMap<String, String>,
    pub update: Update,
    pub plugins: BTreeMap<String, PluginState>,
    /// 没连上游戏时用模拟数据驱动 overlay
    pub demo: bool,
    pub window: Option<[f32; 4]>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut games = BTreeMap::new();
        games.insert(Game::World.key().to_string(), GameLayouts::new(Game::World));
        Settings {
            version: 1,
            first_run_done: false,
            language: Lang::ZhHans,
            term_language: None,
            autostart: false,
            close_to_tray: true,
            auto_connect: true,
            overlay_visible: true,
            density: Density::Standard,
            perf: Perf::default(),
            log_level: LogLevel::Info,
            hotkeys: Hotkeys::default(),
            notify: Notify::default(),
            injection: Injection::default(),
            theme: "light".into(),
            theme_follow_system: false,
            save_account: None,
            games,
            game_paths: BTreeMap::new(),
            update: Update::default(),
            plugins: BTreeMap::new(),
            demo: false,
            window: None,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        let mut s: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| match toml::from_str(&t) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::warn!("settings.toml 解析失败，使用默认设置：{e}");
                    None
                }
            })
            .unwrap_or_default();
        s.normalize();
        s
    }

    /// 写到临时文件再改名，写一半断电也不会坏。
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    pub fn normalize(&mut self) {
        for game in Game::ALL {
            let layouts = self
                .games
                .entry(game.key().to_string())
                .or_insert_with(|| GameLayouts::new(game));
            if layouts.profiles.is_empty() {
                *layouts = GameLayouts::new(game);
            }
            for p in layouts.profiles.values_mut() {
                for m in modules::builtin() {
                    p.ensure(game, &m);
                }
            }
        }
        self.perf.read_hz = self.perf.read_hz.clamp(5, 60);
        self.perf.fps_cap = self.perf.fps_cap.clamp(30, 144);
    }

    pub fn layouts(&self, game: Game) -> &GameLayouts {
        self.games.get(game.key()).expect("normalize 之后每个游戏都有布局")
    }

    pub fn layouts_mut(&mut self, game: Game) -> &mut GameLayouts {
        self.games
            .entry(game.key().to_string())
            .or_insert_with(|| GameLayouts::new(game))
    }

    pub fn profile(&self, game: Game) -> &Profile {
        self.layouts(game).active()
    }

    pub fn profile_mut(&mut self, game: Game) -> &mut Profile {
        self.layouts_mut(game).active_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_toml() {
        let mut s = Settings::default();
        s.normalize();
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn default_profile_has_all_modules() {
        let mut s = Settings::default();
        s.normalize();
        let p = s.profile(Game::World);
        assert!(p.module(modules::MONSTER).unwrap().enabled);
        assert!(!p.module(modules::BASE).unwrap().enabled);
        assert!(!p.module(modules::PLAYER).unwrap().shows("p-team"));
        assert!(p.module(modules::PLAYER).unwrap().shows("p-sharp"));
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = toml::from_str("language = \"en\"\n[perf]\nread_hz = 200").unwrap();
        let mut s = s;
        s.normalize();
        assert_eq!(s.language, Lang::En);
        assert_eq!(s.perf.read_hz, 60);
        assert_eq!(s.hotkeys.show_all, "Left Alt");
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("rh-settings-{}", std::process::id()));
        let path = dir.join("settings.toml");
        let mut s = Settings::default();
        s.demo = true;
        s.profile_mut(Game::World).module_mut("clock").toggle("c-game");
        s.save(&path).unwrap();
        let back = Settings::load(&path);
        assert!(back.demo);
        assert!(!back.profile(Game::World).module("clock").unwrap().shows("c-game"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
