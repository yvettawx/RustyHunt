//! 每一帧从游戏内存读出 [`Snapshot`]。偏移和逻辑照上游
//! `HunterPie.Integrations/Datasources/MonsterHunterWorld`。
//!
//! 每一部分单独读：某一部分读失败时沿用上一帧的值，只写日志。

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use address_map::AddressMap;
use platform::{Memory, ReadResult};
use rh_core::model::*;

use crate::data::{MonsterDef, data};

/// 地址表里缺了某一项。
#[derive(Debug, Clone)]
pub struct MissingKey(pub String);

const MAX_QUEST_TIMERS: [u32; 5] = [54000, 72000, 108000, 126000, 180000];
const PEACE_ZONES: [i32; 9] = [301, 302, 303, 305, 306, 501, 502, 503, 506];
const SAVE_SLOT_SIZE: u64 = 0x26CC00;
const PART_STRUCT_SIZE: usize = 0x70;
const AILMENT_STRUCT_SIZE: usize = 0x88;

fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

fn read_buf<M: Memory>(mem: &M, address: u64, len: usize) -> ReadResult<Vec<u8>> {
    let mut b = vec![0u8; len];
    mem.read_bytes(address, &mut b)?;
    Ok(b)
}

/// 上游 `ApproximateHigh`：取第一个不小于 `v` 的候选值。
fn approximate_high(v: u32, values: &[u32]) -> u32 {
    values.iter().copied().find(|&c| c >= v).unwrap_or(v)
}

/// 每只怪物在读取之间保留的状态。
#[derive(Default)]
struct MonsterCache {
    /// 部位结构的地址，按定义里的顺序；0 表示还没找到
    part_addresses: Vec<u64>,
    /// 异常结构的地址和键
    ailments: Option<Vec<(u64, String)>>,
    list_index: i32,
}

#[derive(Default)]
struct SharpnessCache {
    minimum: Option<Vec<i32>>,
    weapon_id: i32,
    thresholds: Option<Vec<i32>>,
}

pub struct WorldReader<M: Memory> {
    mem: M,
    map: AddressMap,
    base: u64,
    monsters: HashMap<u64, MonsterCache>,
    sharpness: SharpnessCache,
    /// 每个增益见过的最大剩余时间，用作条的满格
    abnormality_max: HashMap<String, f32>,
    skills: Vec<u8>,
    save_address: u64,
    roam_start: Option<Instant>,
    sa_charge_max: f32,
    sa_awake_max: f32,
    /// 单人离线时用怪物掉血估算自己的伤害
    hp_seen: HashMap<u64, f32>,
    solo_damage: f32,
    pub warnings: Vec<String>,
}

impl<M: Memory> WorldReader<M> {
    pub fn new(mem: M, map: AddressMap, base: u64) -> WorldReader<M> {
        WorldReader {
            mem,
            map,
            base,
            monsters: HashMap::new(),
            sharpness: SharpnessCache::default(),
            abnormality_max: HashMap::new(),
            skills: vec![0; 226],
            save_address: 0,
            roam_start: None,
            sa_charge_max: 0.0,
            sa_awake_max: 0.0,
            hp_seen: HashMap::new(),
            solo_damage: 0.0,
            warnings: Vec::new(),
        }
    }

    pub fn memory(&self) -> &M {
        &self.mem
    }

    pub fn map(&self) -> &AddressMap {
        &self.map
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    /// 地址表里的地址加上模块基址。
    pub fn abs(&self, name: &str) -> u64 {
        self.base + self.map.address(name).unwrap_or(0)
    }

    fn offs(&self, name: &str) -> &[i32] {
        self.map.offsets(name).unwrap_or(&[])
    }

    /// 地址表缺少读取所需的项时返回缺的名字。
    pub fn check_map(&self) -> Result<(), MissingKey> {
        const ADDRS: &[&str] = &[
            "ZONE_OFFSET",
            "LEVEL_OFFSET",
            "MONSTER_LIST_ADDRESS",
            "QUEST_DATA_ADDRESS",
            "EQUIPMENT_ADDRESS",
            "WEAPON_ADDRESS",
            "ABNORMALITY_ADDRESS",
        ];
        for a in ADDRS {
            if self.map.address(a).is_none() {
                return Err(MissingKey(a.to_string()));
            }
        }
        Ok(())
    }

    fn warn(&mut self, what: &str, e: impl std::fmt::Display) {
        self.warnings.push(format!("{what}：{e}"));
    }

    /// 读一帧。`prev` 是上一帧，读失败的部分沿用它。
    pub fn read(&mut self, prev: &Snapshot, time: f64) -> Snapshot {
        self.warnings.clear();
        let mut s = Snapshot {
            game: Game::World,
            time,
            ..prev.clone()
        };
        s.damage_split = None;

        match self.read_stage() {
            Ok(id) => {
                s.stage_id = id;
                s.in_town = PEACE_ZONES.contains(&id);
                s.in_hunt_zone = id != 0 && !s.in_town;
                s.stage_key = stage_key(id).to_string();
            }
            Err(e) => self.warn("地点", e),
        }

        if let Err(e) = self.read_skills() {
            self.warn("技能", e);
        }
        match self.read_basic(&mut s) {
            Ok(()) => {}
            Err(e) => self.warn("猎人", e),
        }
        match self.read_hud(&mut s.player) {
            Ok(()) => {}
            Err(e) => self.warn("生命", e),
        }
        match self.read_weapon(&mut s) {
            Ok(()) => {}
            Err(e) => self.warn("武器", e),
        }
        match self.read_quest(&mut s) {
            Ok(()) => {}
            Err(e) => self.warn("任务", e),
        }
        match self.read_monsters(&prev.monsters) {
            Ok(m) => s.monsters = m,
            Err(e) => self.warn("怪物", e),
        }
        match self.read_party(&mut s) {
            Ok(()) => {}
            Err(e) => self.warn("队伍", e),
        }
        match self.read_abnormalities(&s) {
            Ok(a) => s.player.abnormalities = a,
            Err(e) => self.warn("增益", e),
        }
        match self.read_tools() {
            Ok(t) => s.player.tools = t,
            Err(e) => self.warn("专用装备", e),
        }
        match self.read_misc(&mut s) {
            Ok(()) => {}
            Err(e) => self.warn("菜单", e),
        }
        match self.read_base() {
            Ok(b) => s.base = b,
            Err(e) => self.warn("据点", e),
        }
        s
    }

    fn read_stage(&self) -> ReadResult<i32> {
        let a = self.mem.follow(self.abs("ZONE_OFFSET"), self.offs("ZoneOffsets"))?;
        if a == 0 {
            return Ok(0);
        }
        self.mem.read::<i32>(a)
    }

    fn read_skills(&mut self) -> ReadResult<()> {
        let p = self.mem.follow(self.abs("ABNORMALITY_ADDRESS"), self.offs("GEAR_SKILL_OFFSETS"))?;
        if p == 0 {
            return Ok(());
        }
        // 每个技能 24 字节，第 8 字节是装备提供的等级
        let b = read_buf(&self.mem, p, 226 * 24)?;
        for (i, lvl) in self.skills.iter_mut().enumerate() {
            *lvl = b[i * 24 + 8];
        }
        Ok(())
    }

    fn skill(&self, id: usize) -> i32 {
        self.skills.get(id).copied().unwrap_or(0) as i32
    }

    /// 力量延长的倍率。
    fn prolonger(&self, weapon: Weapon) -> f32 {
        let lvl = self.skill(0x35).min(3);
        if lvl <= 0 {
            return 1.0;
        }
        let base = 1.0 + 2f32.powi(lvl - 1) / 10.0;
        match weapon {
            Weapon::SwitchAxe | Weapon::DualBlades => base + 2.0 * lvl as f32 / 10.0,
            _ => base,
        }
    }

    fn read_basic(&mut self, s: &mut Snapshot) -> ReadResult<()> {
        if s.stage_id == 0 {
            self.save_address = 0;
            return Ok(());
        }
        let first = self.mem.follow(self.abs("LEVEL_OFFSET"), self.offs("LevelOffsets"))?;
        if first == 0 {
            return Ok(());
        }
        let slot = self.mem.read::<u32>(first + 0x44)? as u64;
        let header = self.mem.read_ptr(first)? + SAVE_SLOT_SIZE * slot;
        if header == self.save_address && !s.player.name.is_empty() {
            return Ok(());
        }
        s.player.name = self.mem.read_string(header + 0x50, 32)?;
        s.player.hr = self.mem.read::<i16>(header + 0x90)? as i32;
        s.player.mr = self.mem.read::<i16>(header + 0xD4)? as i32;
        self.save_address = header;
        Ok(())
    }

    fn read_hud(&mut self, p: &mut Player) -> ReadResult<()> {
        let a = self.mem.follow(self.abs("EQUIPMENT_ADDRESS"), self.offs("PLAYER_BASIC_INFORMATION_OFFSETS"))?;
        if a == 0 {
            return Ok(());
        }
        let b = read_buf(&self.mem, a, 0x138)?;
        let heal_array = u64_at(&b, 0x30);
        p.hp_max = f32_at(&b, 0x60);
        p.hp = f32_at(&b, 0x64);
        p.stamina = f32_at(&b, 0x12C);
        p.stamina_max = f32_at(&b, 0x130);
        let recoverable = self.mem.read::<f32>(a + 0x2DD4)?;
        p.hp_recoverable = recoverable.max(p.hp);

        // 回复中的四段：第一阶段的上限要乘 2.5
        let mut heal = 0.0;
        let mut heal_max = 0.0;
        if heal_array != 0
            && let Ok(h) = read_buf(&self.mem, heal_array + 0xEBB0, 4 * 64)
        {
            for i in 0..4 {
                let o = i * 64;
                let stage = i32_at(&h, o + 44);
                if stage == 0 {
                    continue;
                }
                let m = f32_at(&h, o + 24);
                heal += f32_at(&h, o + 16);
                heal_max += if stage == 1 { m * 2.5 } else { m };
            }
        }
        p.hp_healing_to = (p.hp + heal_max - heal).clamp(p.hp, p.hp_max.max(p.hp));

        let vitality: f32 = [0.0, 15.0, 30.0, 50.0][self.skill(0x15).clamp(0, 3) as usize];
        p.hp_cap = (150.0 + vitality).max(p.hp_max);
        let cap_up = self.skill(0xA3) >= 2 || self.skill(0xB6) >= 2 || self.skill(0x84) >= 4;
        p.stamina_cap = (if cap_up { 200.0 } else { 150.0f32 }).max(p.stamina_max);
        Ok(())
    }

    fn read_weapon(&mut self, s: &mut Snapshot) -> ReadResult<()> {
        if self.save_address == 0 {
            return Ok(());
        }
        let a = self.mem.follow(self.abs("WEAPON_ADDRESS"), self.offs("WEAPON_OFFSETS"))?;
        if a == 0 {
            return Ok(());
        }
        let weapon = Weapon::from_world_id(self.mem.read::<u8>(a)?);
        if weapon != Weapon::None {
            s.player.weapon = weapon;
        }
        let weapon = s.player.weapon;
        s.player.sharpness = if weapon.has_sharpness() {
            match self.read_sharpness() {
                Ok(v) => v.or(s.player.sharpness.take()),
                Err(e) => {
                    self.warn("斩味", e);
                    s.player.sharpness.take()
                }
            }
        } else {
            None
        };
        match self.read_weapon_state(weapon) {
            Ok(w) => s.player.weapon_state = w,
            Err(e) => self.warn("武器专用槽", e),
        }
        Ok(())
    }

    fn read_sharpness(&mut self) -> ReadResult<Option<Sharpness>> {
        if self.sharpness.minimum.is_none() {
            self.sharpness.minimum = Some(self.mem.read_vec::<i32>(self.abs("MINIMUM_SHARPNESSES_ADDRESS"), 8)?);
        }
        let data_ptr = self.mem.follow(self.abs("WEAPON_DATA_ADDRESS"), self.offs("WEAPON_DATA_OFFSETS"))?;
        if data_ptr == 0 {
            return Ok(None);
        }
        let weapon_id = self.mem.deref::<i32>(self.abs("WEAPON_ADDRESS"), self.offs("WEAPON_ID_OFFSETS"))?;
        let sp = self.mem.follow(self.abs("WEAPON_ADDRESS"), self.offs("WEAPON_SHARPNESS_OFFSETS"))?;
        if sp == 0 {
            return Ok(None);
        }
        let max_level = self.mem.read::<i32>(sp + 0x1D10)?;
        let current = self.mem.read::<i32>(sp + 0x20F8)?;
        let level = self.mem.read::<i32>(sp + 0x20FC)?;
        if !(0..=6).contains(&level) {
            return Ok(None);
        }
        if self.sharpness.thresholds.is_none() || self.sharpness.weapon_id != weapon_id {
            let arr = self.mem.follow(data_ptr, &[weapon_id.wrapping_mul(8), 0x0C])?;
            if arr == 0 {
                return Ok(None);
            }
            let t = self.mem.read_vec::<i16>(arr, 7)?;
            self.sharpness.thresholds = Some(t.into_iter().map(i32::from).collect());
            self.sharpness.weapon_id = weapon_id;
        }
        let thresholds = self.sharpness.thresholds.clone().unwrap_or_default();
        let Some(level) = current_sharpness(&thresholds, current) else {
            return Ok(None);
        };
        let minimum = self.sharpness.minimum.clone().unwrap_or_default();
        let max = maximum_sharpness(&thresholds, &minimum, level.index(), max_level, self.skill(54));
        Ok(Some(Sharpness {
            level,
            current,
            max,
            thresholds,
        }))
    }

    fn read_weapon_state(&mut self, weapon: Weapon) -> ReadResult<WeaponState> {
        let needs = matches!(
            weapon,
            Weapon::Longsword | Weapon::DualBlades | Weapon::InsectGlaive | Weapon::ChargeBlade | Weapon::SwitchAxe
        );
        if !needs {
            return Ok(WeaponState::None);
        }
        let p = self.mem.follow(self.abs("WEAPON_MECHANICS_ADDRESS"), self.offs("WEAPON_MECHANICS_OFFSETS"))?;
        if p == 0 {
            return Ok(WeaponState::None);
        }
        let k = self.prolonger(weapon);
        let b = read_buf(&self.mem, p + 0x2350, 0x130)?;
        let at = |o: usize| o - 0x2350;
        Ok(match weapon {
            Weapon::Longsword => {
                let level = i32_at(&b, at(0x2370));
                if !(0..=3).contains(&level) {
                    return Ok(WeaponState::None);
                }
                let max = [0.0, 200.0, 140.0, 70.0][level as usize] * k;
                WeaponState::Longsword {
                    gauge: f32_at(&b, at(0x2368)) * 100.0,
                    level,
                    level_timer: f32_at(&b, at(0x2374)) * max,
                    level_timer_max: max,
                }
            }
            Weapon::DualBlades => WeaponState::DualBlades {
                gauge: f32_at(&b, at(0x236C)) * 100.0,
                demon_mode: b[at(0x2368)] != 0,
                arch_demon: b[at(0x2369)] != 0,
            },
            Weapon::ChargeBlade => {
                let axe = self.mem.read::<f32>(p + 0x2470)?;
                WeaponState::ChargeBlade {
                    phials: i32_at(&b, at(0x2374)),
                    phials_max: 5 + i32::from(self.skill(69) > 0),
                    charge: f32_at(&b, at(0x2370)),
                    shield: f32_at(&b, at(0x2378)) * k,
                    sword: f32_at(&b, at(0x237C)) * k,
                    axe: axe * k,
                }
            }
            Weapon::SwitchAxe => {
                let abn = self.mem.follow(self.abs("ABNORMALITY_ADDRESS"), self.offs("ABNORMALITY_OFFSETS"))?;
                let slam = if abn != 0 && self.mem.read::<u8>(abn + 0x6E5)? != 0 {
                    self.mem.read::<f32>(abn + 0x6E8)? * k
                } else {
                    0.0
                };
                let charge_timer = f32_at(&b, at(0x2364)) * k;
                self.sa_charge_max = self.sa_charge_max.max(charge_timer);
                self.sa_awake_max = self.sa_awake_max.max(slam);
                WeaponState::SwitchAxe {
                    gauge: f32_at(&b, at(0x2350)),
                    charge: f32_at(&b, at(0x2360)),
                    awakened_timer: charge_timer,
                    awakened_timer_max: self.sa_charge_max,
                }
            }
            Weapon::InsectGlaive => {
                let kinsect = u64_at(&b, at(0x2350));
                let extra = self
                    .mem
                    .follow(self.abs("WEAPON_MECHANICS_ADDRESS"), self.offs("WEAPON_EXTRA_MECHANICS_DATA_OFFSETS"))?;
                let stamina_max = if extra != 0 { self.mem.read::<f32>(extra + 0x12B4)? } else { 0.0 };
                let stamina = if kinsect != 0 { self.mem.read::<f32>(kinsect + 0xA8C)? } else { 0.0 };
                WeaponState::InsectGlaive {
                    red: (f32_at(&b, at(0x2368)) * k).max(0.0),
                    white: (f32_at(&b, at(0x236C)) * k).max(0.0),
                    orange: (f32_at(&b, at(0x2370)) * k).max(0.0),
                    kinsect_stamina: stamina,
                    kinsect_stamina_max: stamina_max,
                }
            }
            _ => WeaponState::None,
        })
    }

    fn quest_struct(&self) -> ReadResult<(i32, i32, i32)> {
        let p = self.mem.follow(self.abs("QUEST_DATA_ADDRESS"), self.offs("QUEST_DATA_OFFSETS"))?;
        if p == 0 {
            return Ok((0, 0, 0));
        }
        let b = read_buf(&self.mem, p + 0x4C, 12)?;
        Ok((i32_at(&b, 0), i32_at(&b, 4), i32_at(&b, 8)))
    }

    fn read_quest(&mut self, s: &mut Snapshot) -> ReadResult<()> {
        let (id, stars, state) = self.quest_struct()?;
        let status = match state {
            2 => QuestStatus::InProgress,
            3 | 4 => QuestStatus::Success,
            5 => QuestStatus::Fail,
            6 | 7 => QuestStatus::Quit,
            _ => QuestStatus::None,
        };

        let timer_ptr = self.mem.follow(self.abs("QUEST_DATA_ADDRESS"), self.offs("QUEST_TIMER_OFFSETS"))?;
        let (time_left, time_max) = if timer_ptr != 0 {
            let left = self.mem.read::<u64>(timer_ptr)? as f32 / 60.0;
            let max_raw = self.mem.read::<u32>(timer_ptr + 0x10)?;
            (left, approximate_high(max_raw, &MAX_QUEST_TIMERS) as f32 / 60.0)
        } else {
            (0.0, 0.0)
        };

        if id > 0 && status != QuestStatus::None {
            let extra = self.mem.follow(self.abs("QUEST_DATA_ADDRESS"), self.offs("QUEST_EXTRA_DATA_OFFSETS"))?;
            let (deaths_max, deaths) = if extra != 0 {
                (self.mem.read::<i32>(extra)?, self.mem.read::<i32>(extra + 4)?)
            } else {
                (0, 0)
            };
            s.quest = Some(Quest {
                id,
                stars: stars % 10,
                rank: match stars {
                    1..=5 => Rank::Low,
                    6..=9 => Rank::High,
                    _ => Rank::Master,
                },
                status,
                time_left,
                time_max,
                deaths,
                deaths_max,
            });
        } else {
            s.quest = None;
        }

        // 任务中用任务计时；自由探索时从进入狩猎区域开始计时
        if s.in_quest() {
            self.roam_start = None;
            s.elapsed = (time_max - time_left).max(0.0);
        } else if s.in_hunt_zone {
            let start = *self.roam_start.get_or_insert_with(Instant::now);
            s.elapsed = start.elapsed().as_secs_f32();
        } else {
            self.roam_start = None;
            if s.quest.is_none() {
                s.elapsed = 0.0;
            }
        }
        Ok(())
    }

    fn read_monsters(&mut self, prev: &[Monster]) -> ReadResult<Vec<Monster>> {
        let list = self.mem.follow(self.abs("MONSTER_LIST_ADDRESS"), self.offs("MONSTER_LIST_OFFSETS"))?;
        if list == 0 {
            self.monsters.clear();
            return Ok(Vec::new());
        }
        let components = self.mem.read_vec::<u64>(list, 128)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for c in components.into_iter().filter(|&c| c != 0) {
            let Ok(addr) = self.mem.read_ptr(c + 0x138) else { continue };
            if addr == 0 || !seen.insert(addr) {
                continue;
            }
            let Ok(em_ptr) = self.mem.read_ptr(addr + 0x2A0) else { continue };
            let Ok(em) = self.mem.read_string(em_ptr + 0x0C, 64) else { continue };
            if !em.starts_with("em\\em") || em.starts_with("em\\ems") {
                continue;
            }
            let id = self.mem.read::<i32>(addr + 0x12280)?;
            let old = prev.iter().find(|m| m.uid == addr);
            match self.read_monster(addr, id, old) {
                Ok(m) => out.push(m),
                Err(e) => {
                    // 读失败沿用上一帧这只怪物
                    if let Some(o) = old {
                        out.push(o.clone());
                    }
                    self.warn("怪物", e);
                }
            }
        }
        self.monsters.retain(|a, _| seen.contains(a));
        Ok(out)
    }

    fn read_monster(&mut self, addr: u64, id: i32, prev: Option<&Monster>) -> ReadResult<Monster> {
        let def = data().monster(id).clone();
        let mut m = prev.cloned().unwrap_or_else(|| Monster {
            uid: addr,
            id,
            key: data().monster_key(id),
            capture_threshold: def.capture / 100.0,
            weaknesses: def.weaknesses.clone(),
            large: true,
            ..Default::default()
        });

        let hp_ptr = self.mem.read_ptr(addr + 0x7670)?;
        let hp = self.mem.read_vec::<f32>(hp_ptr + 0x60, 2)?;
        m.hp_max = hp[0];
        m.hp = hp[1];

        let st = self.mem.read_vec::<f32>(addr + 0x1C0F0, 2)?;
        m.stamina = st[0];
        m.stamina_max = st[1];
        m.tired = m.stamina_max > 0.0 && m.stamina <= 0.0;

        let mut modifier = self.mem.read::<f32>(addr + 0x7730)?;
        let multiplier = self.mem.read::<f32>(addr + 0x184)?;
        if modifier <= 0.0 || modifier >= 2.0 {
            modifier = 1.0;
        }
        m.size = (multiplier / modifier * 100.0).round() / 100.0;
        m.crown = if m.size >= def.gold {
            Crown::Gold
        } else if m.size >= def.silver {
            Crown::Silver
        } else if m.size <= def.mini {
            Crown::Mini
        } else {
            Crown::None
        };

        let enrage = read_buf(&self.mem, addr + 0x1BE30, 72)?;
        let duration = f32_at(&enrage, 36);
        let max_duration = f32_at(&enrage, 40);
        m.enraged = duration > 0.0;
        m.enrage_timer = if duration > 0.0 { (max_duration - duration).max(0.0) } else { 0.0 };
        m.enrage_timer_max = max_duration;

        if let Ok(captured) = self.read_captured(addr) {
            m.captured = captured;
        }

        let cache = self.monsters.entry(addr).or_default();
        cache.list_index = self.mem.read::<i32>(addr + 0x1228C).unwrap_or(-1);

        match self.read_targets(addr) {
            Ok((t, manual)) => {
                m.target = t;
                m.manual_target = manual;
            }
            Err(e) => self.warn("目标", e),
        }
        match self.read_parts(addr, id, &def) {
            Ok(parts) => m.parts = parts,
            Err(e) => self.warn("部位", e),
        }
        match self.read_tenderize(addr, &def, &mut m.parts) {
            Ok(()) => {}
            Err(e) => self.warn("弱点特效", e),
        }
        match self.read_ailments(addr) {
            Ok(a) => m.ailments = a,
            Err(e) => self.warn("怪物异常", e),
        }
        Ok(m)
    }

    fn read_captured(&self, addr: u64) -> ReadResult<bool> {
        let action = addr + 0x61C8;
        let action_id = self.mem.read::<i32>(action + 0xB0)?;
        let p = self
            .mem
            .follow_ptr(action, &[2 * 8 + 0x68, action_id.wrapping_mul(8), 0, 0x20])?;
        if p == 0 {
            return Ok(false);
        }
        let off = self.mem.read::<u32>(p + 3)? as u64;
        let r = self.mem.read_ptr(p + off + 7 + 8)?;
        let name = self.mem.read_string(r, 64)?;
        Ok(name.contains("Capture"))
    }

    fn read_targets(&self, addr: u64) -> ReadResult<(Target, Target)> {
        let my_index = self.monsters.get(&addr).map_or(-1, |c| c.list_index);
        let lock = self.mem.follow(self.abs("LOCKON_ADDRESS"), self.offs("LOCKEDON_MONSTER_INDEX_OFFSETS"))?;
        let target = if lock == 0 {
            Target::None
        } else {
            let idx = self.mem.read::<i32>(lock + 0x950)?;
            if idx == my_index {
                Target::Own
            } else if idx < 0 {
                Target::None
            } else {
                Target::Other
            }
        };

        let quest_target = self
            .mem
            .deref::<u64>(self.abs("MONSTER_QUEST_TARGET_ADDRESS"), self.offs("MONSTER_QUEST_TARGET_OFFSETS"))?;
        let sel = self
            .mem
            .follow(self.abs("MONSTER_MANUAL_TARGET_ADDRESS"), self.offs("MONSTER_MANUAL_TARGET_OFFSETS"))?;
        let (insects, selected, radar) = if sel != 0 {
            let b = read_buf(&self.mem, sel + 0x128, 0x40)?;
            (u64_at(&b, 0), u64_at(&b, 0x20), u64_at(&b, 0x38))
        } else {
            (0, 0, 0)
        };
        let by_quest = quest_target != 0;
        let pinned = insects != 0 && radar != 0;
        let is_selected = if by_quest { quest_target == addr } else { selected == addr };
        let manual = if pinned || by_quest {
            if is_selected { Target::Own } else { Target::Other }
        } else {
            Target::None
        };
        Ok((target, manual))
    }

    fn read_parts(&mut self, addr: u64, id: i32, def: &MonsterDef) -> ReadResult<Vec<Part>> {
        let parts_ptr = self.mem.read_ptr(addr + 0x1D058)?;
        if parts_ptr == 0 {
            return Ok(Vec::new());
        }
        let normal_base = parts_ptr + 0x40;
        let mut sever_addr = parts_ptr + 0x1FC8;
        let mut normal_index = 0u64;

        let mut cached = self
            .monsters
            .get(&addr)
            .map(|c| c.part_addresses.clone())
            .unwrap_or_default();
        cached.resize(def.parts.len(), 0);

        let mut out = Vec::with_capacity(def.parts.len());
        for (i, pd) in def.parts.iter().enumerate() {
            let mut raw = None;
            if cached[i] != 0 {
                raw = Some(read_buf(&self.mem, cached[i], PART_STRUCT_SIZE)?);
            } else if pd.severable {
                // 切断部位在另一块数组里，按部位编号找（上游的循环没有上限，这里限制 64 步）
                for _ in 0..64 {
                    if self.mem.read::<i32>(sever_addr)? <= 0xA0 {
                        sever_addr += 0x8;
                    }
                    let b = read_buf(&self.mem, sever_addr, PART_STRUCT_SIZE)?;
                    if u32_at(&b, 108) == pd.id && f32_at(&b, 12) > 0.0 {
                        cached[i] = sever_addr;
                        // 跳过紧跟着的重复项
                        for _ in 0..16 {
                            sever_addr += 0x78;
                            let next = read_buf(&self.mem, sever_addr, PART_STRUCT_SIZE)?;
                            if !same_part(&b, &next) {
                                break;
                            }
                        }
                        raw = Some(b);
                        break;
                    }
                    sever_addr += 0x78;
                }
            } else {
                let a = normal_base + normal_index * 0x1F8;
                normal_index += 1;
                cached[i] = a;
                raw = Some(read_buf(&self.mem, a, PART_STRUCT_SIZE)?);
            }

            let Some(mut b) = raw else { continue };
            // 煌黑龙的属性爆发次数在另一处
            if id == 87 && u32_at(&b, 108) == 3 {
                let counter = self.mem.read::<i32>(addr + 0x20920)?;
                b[24..28].copy_from_slice(&counter.to_le_bytes());
            }
            out.push(part_from_raw(pd, &b));
        }

        self.monsters.entry(addr).or_default().part_addresses = cached;
        Ok(out)
    }

    fn read_tenderize(&self, addr: u64, def: &MonsterDef, parts: &mut [Part]) -> ReadResult<()> {
        let b = read_buf(&self.mem, addr + 0x1C458, 10 * 64)?;
        for p in parts.iter_mut() {
            p.tenderized = 0.0;
        }
        for i in 0..10 {
            let o = i * 64;
            let part_id = u32_at(&b, o + 48);
            if part_id == u32::MAX {
                continue;
            }
            for (pd, p) in def.parts.iter().zip(parts.iter_mut()) {
                if pd.tenderize_ids.contains(&part_id) {
                    p.tenderized = f32_at(&b, o + 8);
                    p.tenderized_max = f32_at(&b, o + 12);
                }
            }
        }
        Ok(())
    }

    fn read_ailments(&mut self, addr: u64) -> ReadResult<Vec<Ailment>> {
        let known = self.monsters.get(&addr).and_then(|c| c.ailments.clone());
        let list = match known {
            Some(l) => l,
            None => {
                let mut list = Vec::new();
                let mut element = addr + 0x1BC40;
                let mut ptr = self.mem.read_ptr(element)?;
                let mut keys = HashSet::new();
                for _ in 0..64 {
                    if ptr <= 1 {
                        break;
                    }
                    let current = ptr;
                    let b = read_buf(&self.mem, current + 0x148, AILMENT_STRUCT_SIZE)?;
                    element += 8;
                    ptr = self.mem.read_ptr(element)?;
                    if u64_at(&b, 0) != addr {
                        break;
                    }
                    let Some(key) = data().ailment(i32_at(&b, 16)) else { continue };
                    if keys.insert(key.to_string()) {
                        list.push((current, key.to_string()));
                    }
                }
                self.monsters.entry(addr).or_default().ailments = Some(list.clone());
                list
            }
        };
        let mut out = Vec::with_capacity(list.len());
        for (a, key) in list {
            let b = read_buf(&self.mem, a + 0x148, AILMENT_STRUCT_SIZE)?;
            out.push(Ailment {
                key,
                buildup: f32_at(&b, 48),
                buildup_max: f32_at(&b, 64),
                timer: f32_at(&b, 112),
                timer_max: f32_at(&b, 20),
                count: i32_at(&b, 120),
            });
        }
        Ok(out)
    }

    fn read_party(&mut self, s: &mut Snapshot) -> ReadResult<()> {
        let size = self.mem.deref::<i32>(self.abs("SESSION_OFFSET"), self.offs("SESSION_PARTY_OFFSETS"))?;
        s.online = size > 0;

        // 单人离线：游戏不记录伤害，用大型怪物掉的血估算
        if size == 0 {
            if s.in_quest() || s.in_hunt_zone {
                for m in &s.monsters {
                    if let Some(&last) = self.hp_seen.get(&m.uid)
                        && last > m.hp
                    {
                        self.solo_damage += last - m.hp;
                    }
                    self.hp_seen.insert(m.uid, m.hp);
                }
            } else {
                self.hp_seen.clear();
                self.solo_damage = 0.0;
            }
            s.party = vec![PartyMember {
                name: s.player.name.clone(),
                slot: 0,
                weapon: s.player.weapon,
                damage: self.solo_damage,
                is_self: true,
                mr: s.player.mr,
                palico: false,
            }];
            return Ok(());
        }
        self.hp_seen.clear();
        self.solo_damage = 0.0;

        if !s.in_quest() {
            return Ok(());
        }
        let info = self.mem.follow(self.abs("PARTY_ADDRESS"), self.offs("PARTY_OFFSETS"))?;
        let damage = self.mem.follow(self.abs("DAMAGE_ADDRESS"), self.offs("DAMAGE_OFFSETS"))?;
        if info == 0 {
            return Ok(());
        }
        let members = read_buf(&self.mem, info, 4 * 88)?;
        let mut party = Vec::new();
        for i in 0..4 {
            let a = u64_at(&members, i * 88);
            if a == 0 {
                continue;
            }
            let name = self.mem.read_string(a + 0x49, 32)?;
            if name.is_empty() {
                continue;
            }
            let mr = self.mem.read::<i16>(a + 0x72)? as i32;
            let is_self = name == s.player.name;
            let weapon = if is_self {
                s.player.weapon
            } else {
                Weapon::from_world_id(self.mem.read::<u8>(a + 0x7C)?)
            };
            let dmg = if damage != 0 {
                self.mem.read::<i32>(damage + i as u64 * 0x2A0)? as f32
            } else {
                0.0
            };
            party.push(PartyMember {
                name,
                slot: i as i32,
                weapon,
                damage: dmg,
                is_self,
                mr,
                palico: false,
            });
        }
        s.party = party;
        Ok(())
    }

    fn read_abnormalities(&mut self, s: &Snapshot) -> ReadResult<Vec<Abnormality>> {
        let base = self.mem.follow(self.abs("ABNORMALITY_ADDRESS"), self.offs("ABNORMALITY_OFFSETS"))?;
        if !s.in_hunt_zone || base == 0 {
            self.abnormality_max.clear();
            return Ok(Vec::new());
        }
        let gear = self.mem.follow(self.abs("ABNORMALITY_ADDRESS"), self.offs("ABNORMALITY_GEAR_OFFSETS"))?;
        let canteen = self.mem.follow(self.abs("CANTEEN_ADDRESS"), self.offs("ABNORMALITY_CANTEEN_OFFSETS"))?;
        // 狩猎笛和随从的效果是一段连续的 float
        let songs = self.mem.read_vec::<f32>(base + 0x38, 75)?;

        let mut out = Vec::new();
        for def in &data().abnormalities {
            let timer = match def.kind {
                AbnormalityKind::Song | AbnormalityKind::Palico => {
                    let idx = ((def.offset - 0x38) / 4) as usize;
                    songs.get(idx).copied().unwrap_or(0.0)
                }
                AbnormalityKind::Food => {
                    if canteen == 0 {
                        0.0
                    } else {
                        self.mem.read::<f32>(canteen + def.offset as u64)?
                    }
                }
                _ if def.gear => {
                    if gear == 0 {
                        0.0
                    } else {
                        self.mem.read::<f32>(gear + def.offset as u64)?
                    }
                }
                _ => {
                    let sub = if def.depends_on != 0 {
                        self.mem.read::<i32>(base + def.depends_on as u64)?
                    } else {
                        0
                    };
                    if sub != def.with_value {
                        0.0
                    } else if def.infinite {
                        // 鬼人药这类按标志位判断，没有倒计时
                        1.0
                    } else {
                        self.mem.read::<f32>(base + def.offset as u64)?
                    }
                }
            };
            if timer <= 0.0 || !timer.is_finite() {
                self.abnormality_max.remove(&def.id);
                continue;
            }
            let max = self.abnormality_max.entry(def.id.clone()).or_insert(0.0);
            *max = max.max(timer);
            out.push(Abnormality {
                id: def.id.clone(),
                key: def.key.clone(),
                kind: def.kind,
                timer,
                timer_max: *max,
                infinite: def.infinite,
            });
        }
        Ok(out)
    }

    fn read_tools(&self) -> ReadResult<Vec<Tool>> {
        let w = self.mem.follow(self.abs("WEAPON_ADDRESS"), self.offs("WEAPON_OFFSETS"))?;
        let e = self.mem.follow(self.abs("EQUIPMENT_ADDRESS"), self.offs("EQUIPMENT_OFFSETS"))?;
        if w == 0 || e == 0 {
            return Ok(Vec::new());
        }
        let ids = self.mem.read_vec::<i32>(w + 0x34, 2)?;
        let cooldowns = self.mem.read_vec::<f32>(e + 0x99C, 40)?;
        let timers = self.mem.read_vec::<f32>(e + 0xA8C, 40)?;
        Ok(ids
            .into_iter()
            .map(|id| {
                let i = usize::try_from(id).ok().filter(|&i| i < 20);
                let get = |v: &[f32], k: usize| v.get(k).copied().unwrap_or(0.0);
                match i {
                    Some(i) => Tool {
                        key: tool_key(i).to_string(),
                        timer: get(&timers, i),
                        timer_max: get(&timers, i + 20),
                        cooldown: get(&cooldowns, i),
                        cooldown_max: get(&cooldowns, i + 20),
                    },
                    None => Tool::default(),
                }
            })
            .collect())
    }

    fn read_misc(&self, s: &mut Snapshot) -> ReadResult<()> {
        s.hud_open = self.mem.deref::<i32>(self.abs("HUD_MENU_ADDRESS"), self.offs("HUD_MENU_OPEN_OFFSETS"))? == 1;
        let w = self.mem.follow(self.abs("WORLD_DATA_ADDRESS"), self.offs("WORLD_DATA_OFFSETS"))?;
        if w != 0 {
            let t = self.mem.read::<f32>(w + 0x38)?;
            if t.is_finite() {
                let t = t.abs();
                s.world_time = Some(((t as u32 % 24) as u8, ((60.0 * (t % 1.0)) as u32 % 60) as u8));
            }
        }
        Ok(())
    }

    fn read_base(&self) -> ReadResult<Option<Base>> {
        if self.save_address == 0 {
            return Ok(None);
        }
        let save = self.save_address;
        let harvest = read_buf(&self.mem, save + 0x103068, 0x50 + 50 * 16)?;
        let fertilizers = (0..4)
            .map(|i| Fertilizer {
                key: fertilizer_key(i32_at(&harvest, i * 16 + 8)).to_string(),
                count: i32_at(&harvest, i * 16 + 12),
            })
            .collect();
        let items = (0..50).filter(|i| i32_at(&harvest, 0x50 + i * 16 + 12) > 0).count() as i32;
        let steam = read_buf(&self.mem, save + 0x102FDC, 8)?;
        let argosy = self.mem.read::<u8>(save + 0x1034C0)?;
        let tail = self.mem.read::<u8>(save + 0x1034DC)?;
        let in_town = argosy < 250;
        Ok(Some(Base {
            argosy_days: if in_town { argosy as i32 } else { 256 - argosy as i32 },
            argosy_in_town: in_town,
            tailraiders_quests: if tail != u8::MAX { 5 - tail as i32 } else { 0 },
            tailraiders_deployed: tail != u8::MAX,
            harvest_items: items,
            harvest_capacity: 50,
            fertilizers,
            steam_natural: i32_at(&steam, 0),
            steam_natural_max: 700,
            steam_stored: i32_at(&steam, 4),
        }))
    }
}

fn same_part(a: &[u8], b: &[u8]) -> bool {
    [12usize, 16, 24, 32, 36, 108].iter().all(|&o| a[o..o + 4] == b[o..o + 4])
}

fn part_from_raw(pd: &crate::data::PartDef, b: &[u8]) -> Part {
    let max = f32_at(b, 12);
    let hp = f32_at(b, 16);
    let count = i32_at(b, 24);
    let mut p = Part {
        id: pd.id,
        key: pd.key.clone(),
        breaks_max: pd.thresholds.len() as i32,
        ..Default::default()
    };
    if pd.severable {
        p.kind = PartKind::Severable;
        p.durability = hp;
        p.durability_max = max;
        p.breaks = count.min(p.breaks_max);
    } else if !pd.thresholds.is_empty() {
        p.kind = PartKind::Breakable;
        p.flinch = hp;
        p.flinch_max = max;
        // 世界的破坏是"累计硬直几次"：下一档阈值减去已有次数，换算成耐久
        p.breaks = pd.thresholds.iter().filter(|&&t| count >= t).count() as i32;
        match pd.thresholds.iter().find(|&&t| t > count) {
            Some(&t) => {
                p.durability_max = t as f32 * max;
                p.durability = (t - count - 1).max(0) as f32 * max + hp;
            }
            None => {
                p.durability_max = max;
                p.durability = 0.0;
            }
        }
    } else {
        p.kind = PartKind::Flinch;
        p.flinch = hp;
        p.flinch_max = max;
    }
    p
}

/// 上游 `GetCurrentSharpness`：当前值落在哪一段颜色里。
pub fn current_sharpness(thresholds: &[i32], current: i32) -> Option<SharpnessLevel> {
    for i in (1..thresholds.len()).rev() {
        let end = thresholds[i];
        let start = thresholds[i - 1];
        if end == 0 {
            continue;
        }
        if current > start && current <= end {
            return SharpnessLevel::from_index(i as i32);
        }
    }
    Some(SharpnessLevel::Red)
}

/// 上游 `MaximumSharpness`：武器在匠的加成下最多能到多少。
pub fn maximum_sharpness(thresholds: &[i32], minimum: &[i32], level: usize, max_level: i32, handicraft: i32) -> i32 {
    let bonus = 10 * handicraft.clamp(0, 5);
    let upper = thresholds.get(level).copied().unwrap_or(0);
    let actual = usize::try_from(max_level).ok().and_then(|i| minimum.get(i)).copied().unwrap_or(0);
    let last = actual < upper;
    upper.min(actual) + if last { bonus } else { 0 }
}

fn tool_key(i: usize) -> &'static str {
    const KEYS: [&str; 22] = [
        "GHILLIE_MANTLE",
        "TEMPORAL_MANTLE",
        "HEALTH_BOOSTER",
        "ROCKSTEADY_MANTLE",
        "CHALLENGER_MANTLE",
        "VITALITY_MANTLE",
        "FIREPROOF_MANTLE",
        "WATERPROOF_MANTLE",
        "ICEPROOF_MANTLE",
        "THUNDERPROOF_MANTLE",
        "DRAGONPROOF_MANTLE",
        "CLEANSER_BOOSTER",
        "GLIDER_MANTLE",
        "EVASION_MANTLE",
        "IMPACT_MANTLE",
        "APOTHECARY_MANTLE",
        "IMMUNITY_MANTLE",
        "AFFINITY_BOOSTER",
        "BANDIT_MANTLE",
        "ASSASSINS_HOOD",
        "MENDING_MANTLE",
        "CORRUPTED_MANTLE",
    ];
    KEYS.get(i).copied().unwrap_or("")
}

fn fertilizer_key(id: i32) -> &'static str {
    match id {
        1 => "FERTILIZER_PLANT_S_STRING",
        2 => "FERTILIZER_PLANT_L_STRING",
        3 => "FERTILIZER_FUNGI_S_STRING",
        4 => "FERTILIZER_FUNGI_L_STRING",
        5 => "FERTILIZER_HONEY_S_STRING",
        6 => "FERTILIZER_HONEY_L_STRING",
        7 => "FERTILIZER_GROWTH_S_STRING",
        8 => "FERTILIZER_GROWTH_L_STRING",
        _ => "",
    }
}

/// 地点编号对应术语表里地图的键；城镇、房间等不在术语表里。
pub fn stage_key(id: i32) -> &'static str {
    match id {
        101 | 406 => "ancient_forest",
        102 => "wildspire_waste",
        103 => "coral_highlands",
        104 => "rotten_vale",
        105 => "elders_recess",
        108 => "hoarfrost_reach",
        109 => "guiding_lands",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharpness_level_lookup() {
        let t = [50, 90, 130, 190, 260, 320, 0];
        assert_eq!(current_sharpness(&t, 300), Some(SharpnessLevel::White));
        assert_eq!(current_sharpness(&t, 100), Some(SharpnessLevel::Yellow));
        assert_eq!(current_sharpness(&t, 10), Some(SharpnessLevel::Red));
    }

    #[test]
    fn maximum_sharpness_adds_handicraft_on_last_level() {
        let t = [50, 90, 130, 190, 260, 320, 0];
        let min = [10, 20, 30, 40, 50, 60, 300, 400];
        // 最大等级 6 对应的实际上限 300 小于白段的上限 320，加上匠 5 级的 50
        assert_eq!(maximum_sharpness(&t, &min, 5, 6, 5), 350);
        assert_eq!(maximum_sharpness(&t, &min, 5, 7, 5), 320);
    }

    #[test]
    fn breakable_part_counts_flinches() {
        let pd = crate::data::PartDef {
            id: 2,
            key: "PART_HEAD".into(),
            severable: false,
            thresholds: vec![5],
            tenderize_ids: vec![],
        };
        let mut b = vec![0u8; PART_STRUCT_SIZE];
        b[12..16].copy_from_slice(&100f32.to_le_bytes());
        b[16..20].copy_from_slice(&40f32.to_le_bytes());
        b[24..28].copy_from_slice(&2i32.to_le_bytes());
        let p = part_from_raw(&pd, &b);
        assert_eq!(p.kind, PartKind::Breakable);
        assert_eq!(p.durability_max, 500.0);
        assert_eq!(p.durability, 240.0);
        assert_eq!(p.breaks, 0);
        b[24..28].copy_from_slice(&5i32.to_le_bytes());
        let p = part_from_raw(&pd, &b);
        assert!(p.broken());
    }

    #[test]
    fn quest_timer_rounds_up() {
        assert_eq!(approximate_high(53000, &MAX_QUEST_TIMERS), 54000);
        assert_eq!(approximate_high(180000, &MAX_QUEST_TIMERS), 180000);
        assert_eq!(approximate_high(200000, &MAX_QUEST_TIMERS), 200000);
    }
}
