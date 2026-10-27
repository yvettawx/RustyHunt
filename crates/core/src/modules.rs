//! Metadata of in-game modules: ID, name, elements that can be toggled one by one, and default
//! positions per game. Built-in and plugin modules share one description (`ModuleInfo`); the
//! Modules page of the main window and the overlay display settings both read from here.

use serde::{Deserialize, Serialize};

use crate::model::Game;

/// Version of the module interface; bump it when the module description format changes.
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

/// Module position, computed on a 1080-high reference frame. On wide screens modules anchor
/// left, center or right; bottom-anchored modules grow upward as their content grows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub h: HAnchor,
    pub v: VAnchor,
    /// Distance to the anchored edge; when centered, the offset of the module center from the frame center
    pub dx: f32,
    pub dy: f32,
}

impl Placement {
    pub const fn top_left(x: f32, y: f32) -> Placement {
        Placement {
            h: HAnchor::Left,
            v: VAnchor::Top,
            dx: x,
            dy: y,
        }
    }

    pub const fn top_right(right: f32, y: f32) -> Placement {
        Placement {
            h: HAnchor::Right,
            v: VAnchor::Top,
            dx: right,
            dy: y,
        }
    }

    pub const fn top_center(offset: f32, y: f32) -> Placement {
        Placement {
            h: HAnchor::Center,
            v: VAnchor::Top,
            dx: offset,
            dy: y,
        }
    }

    pub const fn bottom_center(offset: f32, bottom: f32) -> Placement {
        Placement {
            h: HAnchor::Center,
            v: VAnchor::Bottom,
            dx: offset,
            dy: bottom,
        }
    }

    pub const fn bottom_right(right: f32, bottom: f32) -> Placement {
        Placement {
            h: HAnchor::Right,
            v: VAnchor::Bottom,
            dx: right,
            dy: bottom,
        }
    }

    /// Top-left position in the frame. `frame_w` is the reference frame width (height is fixed at 1080).
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

    /// Derives the anchor from a top-left position, by the region the module is in. Used when a drag ends in layout mode.
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

/// An element of a module that can be toggled on its own.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementInfo {
    pub key: String,
    /// UI copy key
    pub name: String,
    pub default_on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleInfo {
    pub id: String,
    /// A copy key for built-in modules, the plugin's name for plugin modules
    pub name: String,
    pub default_on: bool,
    pub elements: Vec<ElementInfo>,
    pub games: Vec<Game>,
    /// The plugin it comes from; empty for built-in modules
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
                ("m-abs", true),
                ("m-en", true),
                ("m-crown", true),
                ("m-record", true),
                ("m-notes", true),
                ("m-state", true),
                ("m-weak", true),
                ("m-ghost", true),
                ("m-cap", true),
                ("m-st", true),
                ("m-parts", true),
                ("m-b2", true),
                ("m-pips", true),
                ("m-pval", false),
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
        module(AREA, true, &[("a-record", true), ("a-notes", true)]),
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
        module(BASE, false, &[("h-argosy", true), ("h-tail", true), ("h-farm", true), ("h-steam", true)]),
        module(PERF, false, &[("f-frame", true), ("f-read", true)]),
    ]
}

/// Default positions per game, designed at 1920x1080 and clear of the native HUD.
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
        // Plugin modules: under the quest clock, an area built-in modules don't use
        _ => Placement::top_center(0.0, 110.0),
    }
}

/// Modules whose width can be dragged in layout mode: (default, min, max) before scaling.
pub fn width_range(id: &str, monster_side: bool) -> Option<(f32, f32, f32)> {
    match id {
        MONSTER if monster_side => Some((380.0, 300.0, 640.0)),
        MONSTER => Some((820.0, 480.0, 1400.0)),
        BUFFS => Some((560.0, 200.0, 960.0)),
        DAMAGE => Some((270.0, 200.0, 520.0)),
        AREA => Some((230.0, 180.0, 420.0)),
        WEAPON => Some((300.0, 220.0, 480.0)),
        BASE => Some((220.0, 180.0, 420.0)),
        _ => None,
    }
}

/// A module's width: the one set in layout mode, kept within range, or its default.
pub fn width(id: &str, monster_side: bool, set: Option<f32>) -> f32 {
    let (default, min, max) = width_range(id, monster_side).unwrap_or((300.0, 100.0, 2000.0));
    set.map_or(default, |w| w.clamp(min, max))
}

/// Player module: pixels per HP point (measured on the native health bar) and the native bar's area.
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
