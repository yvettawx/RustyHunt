//! 局内模块的元信息：编号、名字、可以单独开关的元素、各作的默认位置。
//! 内置模块和插件模块用同一套描述（`ModuleInfo`），主界面"模块"页和
//! overlay 的显示设置都从这里取。

use serde::{Deserialize, Serialize};

use crate::model::Game;

/// 模块接口的版本，改了模块描述的格式时升版本。
pub const MODULE_API_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum HAnchor {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VAnchor {
    #[default]
    Top,
    Bottom,
}

/// 模块的位置，按 1080 高的参考画面计算。宽屏时左、中、右锚定，
/// 底部锚定的模块内容变多时向上长。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub h: HAnchor,
    pub v: VAnchor,
    /// 到锚定边的距离；居中时是模块中心相对画面中心的偏移
    pub dx: f32,
    pub dy: f32,
}

impl Placement {
    pub const fn top_left(x: f32, y: f32) -> Placement {
        Placement { h: HAnchor::Left, v: VAnchor::Top, dx: x, dy: y }
    }

    pub const fn top_right(right: f32, y: f32) -> Placement {
        Placement { h: HAnchor::Right, v: VAnchor::Top, dx: right, dy: y }
    }

    pub const fn top_center(offset: f32, y: f32) -> Placement {
        Placement { h: HAnchor::Center, v: VAnchor::Top, dx: offset, dy: y }
    }

    pub const fn bottom_center(offset: f32, bottom: f32) -> Placement {
        Placement { h: HAnchor::Center, v: VAnchor::Bottom, dx: offset, dy: bottom }
    }

    pub const fn bottom_right(right: f32, bottom: f32) -> Placement {
        Placement { h: HAnchor::Right, v: VAnchor::Bottom, dx: right, dy: bottom }
    }

    /// 左上角在画面里的坐标。`frame_w` 是参考画面的宽（高固定 1080）。
    pub fn to_top_left(&self, frame_w: f32, size: (f32, f32)) -> (f32, f32) {
        let x = match self.h {
            HAnchor::Left => self.dx,
            HAnchor::Right => frame_w - self.dx - size.0,
            HAnchor::Center => frame_w / 2.0 + self.dx - size.0 / 2.0,
        };
        let y = match self.v {
            VAnchor::Top => self.dy,
            VAnchor::Bottom => 1080.0 - self.dy - size.1,
        };
        (x, y)
    }

    /// 由左上角坐标反推：按模块所在的区域选锚点。布局模式拖动结束时用。
    pub fn from_top_left(x: f32, y: f32, frame_w: f32, size: (f32, f32)) -> Placement {
        let cx = x + size.0 / 2.0;
        let h = if cx < frame_w / 3.0 {
            HAnchor::Left
        } else if cx > frame_w * 2.0 / 3.0 {
            HAnchor::Right
        } else {
            HAnchor::Center
        };
        let v = if y + size.1 / 2.0 > 540.0 { VAnchor::Bottom } else { VAnchor::Top };
        let dx = match h {
            HAnchor::Left => x,
            HAnchor::Right => frame_w - x - size.0,
            HAnchor::Center => cx - frame_w / 2.0,
        };
        let dy = match v {
            VAnchor::Top => y,
            VAnchor::Bottom => 1080.0 - y - size.1,
        };
        Placement { h, v, dx, dy }
    }
}

/// 模块里一个可以单独开关的元素。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementInfo {
    pub key: String,
    /// 界面文案键
    pub name: String,
    pub default_on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleInfo {
    pub id: String,
    /// 内置模块是文案键，插件模块是插件给的名字
    pub name: String,
    pub default_on: bool,
    pub elements: Vec<ElementInfo>,
    pub games: Vec<Game>,
    /// 来自哪个插件；内置模块为空
    pub plugin: Option<String>,
}

fn el(key: &str, default_on: bool) -> ElementInfo {
    ElementInfo {
        key: key.to_string(),
        name: format!("element.{key}"),
        default_on,
    }
}

fn module(id: &str, default_on: bool, elements: &[(&str, bool)]) -> ModuleInfo {
    ModuleInfo {
        id: id.to_string(),
        name: format!("module.{id}"),
        default_on,
        elements: elements.iter().map(|(k, on)| el(k, *on)).collect(),
        games: Game::ALL.to_vec(),
        plugin: None,
    }
}

pub const MONSTER: &str = "monster";
pub const BUFFS: &str = "buffs";
pub const DAMAGE: &str = "damage";
pub const AREA: &str = "area";
pub const PLAYER: &str = "player";
pub const CLOCK: &str = "clock";
pub const WEAPON: &str = "weapon";
pub const BASE: &str = "base";
pub const PERF: &str = "perf";

pub fn builtin() -> Vec<ModuleInfo> {
    vec![
        module(
            MONSTER,
            true,
            &[
                ("m-pct", true),
                ("m-abs", true),
                ("m-en", true),
                ("m-crown", true),
                ("m-state", true),
                ("m-weak", true),
                ("m-dmg", true),
                ("m-ghost", true),
                ("m-cap", true),
                ("m-st", true),
                ("m-stl", true),
                ("m-parts", true),
                ("m-b2", true),
                ("m-pips", true),
                ("m-pval", true),
                ("m-ails", true),
                ("m-toast", true),
            ],
        ),
        module(BUFFS, true, &[("b-time", true), ("b-low", true), ("b-debuff", true)]),
        module(
            DAMAGE,
            true,
            &[
                ("d-team", true),
                ("d-rank", true),
                ("d-bar", true),
                ("d-others", true),
                ("d-blur", false),
                ("d-pal", true),
                ("d-split", true),
                ("d-curve", true),
            ],
        ),
        module(AREA, true, &[("a-bar", true), ("a-pct", true)]),
        module(
            PLAYER,
            true,
            &[
                ("p-num", true),
                ("p-st", true),
                ("p-ruler", true),
                ("p-ails", true),
                ("p-sharp", true),
                ("p-team", false),
            ],
        ),
        module(CLOCK, true, &[("c-quest", true), ("c-game", true)]),
        module(WEAPON, true, &[("w-gauge", true), ("w-num", true)]),
        module(
            BASE,
            false,
            &[("h-argosy", true), ("h-tail", true), ("h-farm", true), ("h-steam", true)],
        ),
        module(PERF, false, &[("f-frame", true), ("f-read", true)]),
    ]
}

/// 各作的默认位置，按 1920×1080 设计，避开原生 HUD。
pub fn default_placement(game: Game, id: &str, monster_side: bool) -> Placement {
    match (game, id) {
        (_, MONSTER) if monster_side => Placement::top_right(40.0, 230.0),
        (_, MONSTER) => Placement::bottom_center(-90.0, 90.0),
        (_, BUFFS) => Placement::top_left(60.0, 312.0),
        (_, DAMAGE) => Placement::top_left(60.0, 392.0),
        (_, AREA) => Placement::top_right(130.0, 214.0),
        (_, PLAYER) => Placement::top_left(176.0, 40.0),
        (_, CLOCK) => Placement::top_center(0.0, 24.0),
        (_, WEAPON) => Placement::top_left(60.0, 236.0),
        (_, BASE) => Placement::top_right(140.0, 330.0),
        (_, PERF) => Placement::bottom_right(20.0, 186.0),
        // 插件模块：放在任务时钟下面，那里内置模块不用
        _ => Placement::top_center(0.0, 110.0),
    }
}

/// 原生 HUD 的大致区域（布局模式里画出来，模块拖进去时提示）。
pub fn native_zones(game: Game) -> &'static [(f32, f32, f32, f32, &'static str)] {
    match game {
        Game::World => &[
            (40.0, 30.0, 620.0, 100.0, "zone.health"),
            (1400.0, 90.0, 500.0, 100.0, "zone.quest"),
            (90.0, 670.0, 340.0, 340.0, "zone.minimap"),
            (1280.0, 930.0, 620.0, 140.0, "zone.items"),
        ],
        Game::Rise => &[
            (20.0, 0.0, 1000.0, 180.0, "zone.health"),
            (150.0, 600.0, 310.0, 470.0, "zone.minimap"),
            (1560.0, 880.0, 360.0, 190.0, "zone.items"),
        ],
        Game::Wilds => &[
            (30.0, 15.0, 670.0, 185.0, "zone.health"),
            (40.0, 820.0, 340.0, 250.0, "zone.minimap"),
            (430.0, 915.0, 280.0, 110.0, "zone.items"),
        ],
    }
}

/// 自身模块：每点生命对应的像素（量的原生血条）和原生血条区域。
pub fn player_metrics(game: Game) -> (f32, [f32; 4]) {
    match game {
        Game::World => (3.65, [86.0, 30.0, 690.0, 116.0]),
        Game::Rise => (5.25, [8.0, 0.0, 1000.0, 176.0]),
        Game::Wilds => (3.1, [36.0, 20.0, 640.0, 184.0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_round_trips() {
        let size = (300.0, 120.0);
        for w in [1920.0, 2520.0] {
            for (x, y) in [(60.0, 40.0), (1500.0, 700.0), (900.0, 300.0)] {
                let p = Placement::from_top_left(x, y, 1920.0, size);
                let (x2, y2) = p.to_top_left(1920.0, size);
                assert!((x - x2).abs() < 0.01 && (y - y2).abs() < 0.01, "{w} {x} {y}");
            }
        }
    }

    #[test]
    fn right_anchor_follows_wide_screen() {
        let p = Placement::top_right(40.0, 100.0);
        assert_eq!(p.to_top_left(1920.0, (380.0, 10.0)).0, 1500.0);
        assert_eq!(p.to_top_left(2520.0, (380.0, 10.0)).0, 2100.0);
    }

    #[test]
    fn element_keys_unique() {
        let mut seen = std::collections::HashSet::new();
        for m in builtin() {
            for e in m.elements {
                assert!(seen.insert(e.key.clone()), "{}", e.key);
            }
        }
    }
}
