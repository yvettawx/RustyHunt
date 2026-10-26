//! 三作统一的游戏数据快照。读取线程每一帧产出一份 [`Snapshot`]，
//! overlay、记录、插件都只读它。字段表见 `docs/fields.md`。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    #[default]
    World,
    Rise,
    Wilds,
}

impl Game {
    pub const ALL: [Game; 3] = [Game::World, Game::Rise, Game::Wilds];

    pub fn key(self) -> &'static str {
        match self {
            Game::World => "world",
            Game::Rise => "rise",
            Game::Wilds => "wilds",
        }
    }

    pub fn exe(self) -> &'static str {
        match self {
            Game::World => "MonsterHunterWorld.exe",
            Game::Rise => "MonsterHunterRise.exe",
            Game::Wilds => "MonsterHunterWilds.exe",
        }
    }

    pub fn steam_app_id(self) -> u32 {
        match self {
            Game::World => 582010,
            Game::Rise => 1446780,
            Game::Wilds => 2246340,
        }
    }

    /// 1.0 只支持世界
    pub fn supported(self) -> bool {
        matches!(self, Game::World)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Weapon {
    #[default]
    Greatsword,
    SwordAndShield,
    DualBlades,
    Longsword,
    Hammer,
    HuntingHorn,
    Lance,
    Gunlance,
    SwitchAxe,
    ChargeBlade,
    InsectGlaive,
    Bow,
    HeavyBowgun,
    LightBowgun,
    None,
}

impl Weapon {
    pub fn from_world_id(id: u8) -> Weapon {
        match id {
            0 => Weapon::Greatsword,
            1 => Weapon::SwordAndShield,
            2 => Weapon::DualBlades,
            3 => Weapon::Longsword,
            4 => Weapon::Hammer,
            5 => Weapon::HuntingHorn,
            6 => Weapon::Lance,
            7 => Weapon::Gunlance,
            8 => Weapon::SwitchAxe,
            9 => Weapon::ChargeBlade,
            10 => Weapon::InsectGlaive,
            11 => Weapon::Bow,
            12 => Weapon::HeavyBowgun,
            13 => Weapon::LightBowgun,
            _ => Weapon::None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Weapon::Greatsword => "greatsword",
            Weapon::SwordAndShield => "sword_and_shield",
            Weapon::DualBlades => "dual_blades",
            Weapon::Longsword => "longsword",
            Weapon::Hammer => "hammer",
            Weapon::HuntingHorn => "hunting_horn",
            Weapon::Lance => "lance",
            Weapon::Gunlance => "gunlance",
            Weapon::SwitchAxe => "switch_axe",
            Weapon::ChargeBlade => "charge_blade",
            Weapon::InsectGlaive => "insect_glaive",
            Weapon::Bow => "bow",
            Weapon::HeavyBowgun => "heavy_bowgun",
            Weapon::LightBowgun => "light_bowgun",
            Weapon::None => "none",
        }
    }

    pub fn has_sharpness(self) -> bool {
        !matches!(self, Weapon::Bow | Weapon::HeavyBowgun | Weapon::LightBowgun | Weapon::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Crown {
    #[default]
    None,
    Mini,
    Silver,
    Gold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Element {
    Fire,
    Water,
    Ice,
    Thunder,
    Dragon,
}

impl Element {
    pub fn key(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Water => "water",
            Element::Ice => "ice",
            Element::Thunder => "thunder",
            Element::Dragon => "dragon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    None,
    /// 这只怪物是目标
    Own,
    /// 目标是另一只怪物
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    #[default]
    Flinch,
    Breakable,
    Severable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Part {
    pub id: u32,
    /// 部位名的本地化键，例如 `PART_HEAD`
    pub key: String,
    pub kind: PartKind,
    /// 硬直：当前值和上限
    pub flinch: f32,
    pub flinch_max: f32,
    /// 耐久：到下一次破坏还差多少。切断部位用切断耐久
    pub durability: f32,
    pub durability_max: f32,
    /// 已经破坏（或切断）的次数
    pub breaks: i32,
    /// 能破坏的总次数，0 表示不能破坏
    pub breaks_max: i32,
    pub tenderized: f32,
    pub tenderized_max: f32,
}

impl Part {
    pub fn broken(&self) -> bool {
        self.breaks_max > 0 && self.breaks >= self.breaks_max
    }

    pub fn flinch_ratio(&self) -> f32 {
        ratio(self.flinch, self.flinch_max)
    }

    pub fn durability_ratio(&self) -> f32 {
        if self.broken() { 0.0 } else { ratio(self.durability, self.durability_max) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Ailment {
    /// 本地化键，例如 `AILMENT_POISON`
    pub key: String,
    pub buildup: f32,
    pub buildup_max: f32,
    /// 正在生效时的剩余时间
    pub timer: f32,
    pub timer_max: f32,
    /// 已经触发的次数
    pub count: i32,
}

impl Ailment {
    pub fn active(&self) -> bool {
        self.timer > 0.0
    }

    pub fn buildup_ratio(&self) -> f32 {
        ratio(self.buildup, self.buildup_max)
    }

    pub fn timer_ratio(&self) -> f32 {
        ratio(self.timer, self.timer_max)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Monster {
    /// 在游戏内存里的地址，一场任务里唯一
    pub uid: u64,
    /// 游戏内部编号
    pub id: i32,
    /// 术语表 `locales/terms/*.toml` 里的键，例如 `raging_brachydios`
    pub key: String,
    pub hp: f32,
    pub hp_max: f32,
    pub stamina: f32,
    pub stamina_max: f32,
    pub enraged: bool,
    pub enrage_timer: f32,
    pub enrage_timer_max: f32,
    pub tired: bool,
    /// 血量比例低于它时可以捕获，0 表示不能捕获
    pub capture_threshold: f32,
    pub crown: Crown,
    /// 体型倍率，例如 1.23
    pub size: f32,
    pub weaknesses: Vec<Element>,
    pub parts: Vec<Part>,
    pub ailments: Vec<Ailment>,
    pub target: Target,
    pub manual_target: Target,
    pub captured: bool,
    /// 是否是大型怪物
    pub large: bool,
}

impl Monster {
    pub fn hp_ratio(&self) -> f32 {
        ratio(self.hp, self.hp_max)
    }

    pub fn stamina_ratio(&self) -> f32 {
        ratio(self.stamina, self.stamina_max)
    }

    pub fn capturable(&self) -> bool {
        self.capture_threshold > 0.0 && self.hp > 0.0 && self.hp_ratio() <= self.capture_threshold
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0 && !self.captured
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SharpnessLevel {
    #[default]
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    White,
    Purple,
}

impl SharpnessLevel {
    pub fn from_index(i: i32) -> Option<SharpnessLevel> {
        use SharpnessLevel::*;
        [Red, Orange, Yellow, Green, Blue, White, Purple].get(usize::try_from(i).ok()?).copied()
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Sharpness {
    pub level: SharpnessLevel,
    /// 当前斩味值（击数），从 0 开始累计
    pub current: i32,
    /// 能达到的最大值（含匠）
    pub max: i32,
    /// 每一段颜色的上限，7 段
    pub thresholds: Vec<i32>,
}

impl Sharpness {
    /// 当前颜色还能打多少下
    pub fn hits_left(&self) -> i32 {
        let level = self.level.index();
        let start = if level == 0 { 0 } else { self.thresholds.get(level - 1).copied().unwrap_or(0) };
        (self.current - start).max(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AbnormalityKind {
    #[default]
    Consumable,
    Song,
    Palico,
    Skill,
    Gear,
    Food,
    Debuff,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Abnormality {
    /// 唯一编号，例如 `Consumables_6C4`
    pub id: String,
    /// 本地化键，例如 `ABNORMALITY_DEMON_POWDER`
    pub key: String,
    pub kind: AbnormalityKind,
    pub timer: f32,
    pub timer_max: f32,
    pub infinite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Tool {
    /// 本地化键，例如 `TOOL_GHILLIE_MANTLE`；空表示没有装备
    pub key: String,
    pub timer: f32,
    pub timer_max: f32,
    pub cooldown: f32,
    pub cooldown_max: f32,
}

/// 武器专用槽。只有世界支持的五种武器有内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WeaponState {
    #[default]
    None,
    Longsword {
        /// 气刃槽 0..100
        gauge: f32,
        /// 练气等级 0 无，1 白，2 黄，3 红
        level: i32,
        level_timer: f32,
        level_timer_max: f32,
    },
    DualBlades {
        gauge: f32,
        demon_mode: bool,
        arch_demon: bool,
    },
    InsectGlaive {
        red: f32,
        white: f32,
        orange: f32,
        kinsect_stamina: f32,
        kinsect_stamina_max: f32,
    },
    ChargeBlade {
        phials: i32,
        phials_max: i32,
        /// 剑能量 0..100
        charge: f32,
        shield: f32,
        sword: f32,
        axe: f32,
    },
    SwitchAxe {
        /// 剑瓶 0..100
        gauge: f32,
        /// 觉醒充能 0..100
        charge: f32,
        awakened_timer: f32,
        awakened_timer_max: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Player {
    pub name: String,
    pub hr: i32,
    pub mr: i32,
    pub hp: f32,
    pub hp_max: f32,
    /// 可回复的红血上限（生命条会回到这里）
    pub hp_recoverable: f32,
    /// 正在回复到的目标值
    pub hp_healing_to: f32,
    pub hp_cap: f32,
    pub stamina: f32,
    pub stamina_max: f32,
    pub stamina_cap: f32,
    pub weapon: Weapon,
    pub sharpness: Option<Sharpness>,
    pub weapon_state: WeaponState,
    pub abnormalities: Vec<Abnormality>,
    pub tools: Vec<Tool>,
}

impl Player {
    pub fn debuffs(&self) -> impl Iterator<Item = &Abnormality> {
        self.abnormalities.iter().filter(|a| a.kind == AbnormalityKind::Debuff)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PartyMember {
    pub name: String,
    pub slot: i32,
    pub weapon: Weapon,
    pub damage: f32,
    pub is_self: bool,
    pub mr: i32,
    /// 随从不是猎人
    pub palico: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuestStatus {
    #[default]
    None,
    InProgress,
    Success,
    Fail,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Rank {
    #[default]
    Low,
    High,
    Master,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Quest {
    pub id: i32,
    pub stars: i32,
    pub rank: Rank,
    pub status: QuestStatus,
    pub time_left: f32,
    pub time_max: f32,
    pub deaths: i32,
    pub deaths_max: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Fertilizer {
    pub key: String,
    pub count: i32,
}

/// 据点活动（世界）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Base {
    pub argosy_days: i32,
    pub argosy_in_town: bool,
    pub tailraiders_quests: i32,
    pub tailraiders_deployed: bool,
    pub harvest_items: i32,
    pub harvest_capacity: i32,
    pub fertilizers: Vec<Fertilizer>,
    pub steam_natural: i32,
    pub steam_natural_max: i32,
    pub steam_stored: i32,
}

/// 伤害 hook 送来的本机玩家伤害拆分。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DamageSplit {
    pub total: f32,
    pub physical: Option<f32>,
    pub elemental: Option<f32>,
    pub critical: f32,
    pub hits: u32,
    pub critical_hits: u32,
}

impl DamageSplit {
    pub fn crit_rate(&self) -> f32 {
        if self.hits == 0 { 0.0 } else { self.critical_hits as f32 / self.hits as f32 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Snapshot {
    pub game: Game,
    /// 读取线程启动后的秒数
    pub time: f64,
    pub stage_id: i32,
    /// 术语表里地图的键，城镇等没有条目时为空
    pub stage_key: String,
    pub in_hunt_zone: bool,
    pub in_town: bool,
    /// 打开了游戏菜单或地图
    pub hud_open: bool,
    /// 游戏内时间（时、分）
    pub world_time: Option<(u8, u8)>,
    pub quest: Option<Quest>,
    /// 这一场狩猎已经过去的秒数
    pub elapsed: f32,
    pub player: Player,
    pub party: Vec<PartyMember>,
    pub monsters: Vec<Monster>,
    pub base: Option<Base>,
    pub damage_split: Option<DamageSplit>,
    /// 多人任务
    pub online: bool,
}

impl Snapshot {
    pub fn in_quest(&self) -> bool {
        self.quest.as_ref().is_some_and(|q| q.status == QuestStatus::InProgress)
    }

    pub fn large_monsters(&self) -> impl Iterator<Item = &Monster> {
        self.monsters.iter().filter(|m| m.large)
    }

    pub fn monster(&self, uid: u64) -> Option<&Monster> {
        self.monsters.iter().find(|m| m.uid == uid)
    }

    pub fn self_member(&self) -> Option<&PartyMember> {
        self.party.iter().find(|p| p.is_self)
    }
}

pub fn ratio(v: f32, max: f32) -> f32 {
    if max > 0.0 && v.is_finite() { (v / max).clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_ratios() {
        let p = Part {
            durability: 50.0,
            durability_max: 200.0,
            breaks: 1,
            breaks_max: 2,
            ..Default::default()
        };
        assert_eq!(p.durability_ratio(), 0.25);
        assert!(!p.broken());
        let p = Part { breaks: 2, ..p };
        assert!(p.broken());
        assert_eq!(p.durability_ratio(), 0.0);
    }

    #[test]
    fn capturable_needs_threshold() {
        let m = Monster {
            hp: 10.0,
            hp_max: 100.0,
            capture_threshold: 0.2,
            ..Default::default()
        };
        assert!(m.capturable());
        let m = Monster { capture_threshold: 0.0, ..m };
        assert!(!m.capturable());
    }

    #[test]
    fn sharpness_hits_left() {
        let s = Sharpness {
            level: SharpnessLevel::White,
            current: 330,
            max: 400,
            thresholds: vec![50, 90, 130, 190, 260, 320, 0],
        };
        assert_eq!(s.hits_left(), 70);
    }

    #[test]
    fn ratio_handles_zero_and_nan() {
        assert_eq!(ratio(1.0, 0.0), 0.0);
        assert_eq!(ratio(f32::NAN, 1.0), 0.0);
        assert_eq!(ratio(3.0, 2.0), 1.0);
    }
}
