//! Demo data: drives the overlay when the game isn't connected, for arranging the layout and previewing themes.
//! Monsters, parts and ailments come from real World data; values change randomly like an ordinary hunt.

use rh_core::model::*;

use crate::data::data;

/// A small repeatable random number generator, so no extra dependency is needed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.next() * (b - a)
    }

    fn chance(&mut self, p: f32) -> bool {
        self.next() < p
    }

    fn pick(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

pub struct Demo {
    rng: Rng,
    snap: Snapshot,
    t: f64,
    hp_red: f32,
    quest_left: f32,
    next_enrage: f32,
    tired_left: f32,
    flinch: Vec<(f32, f32)>,
    /// A fixed state instead of the random hunt, see `Demo::scene`
    scene: Option<(Snapshot, Snapshot)>,
}

const TARGET: i32 = 96; // Raging Brachydios
const OTHERS: [i32; 2] = [39, 76]; // Bazelgeuse, Brachydios

impl Default for Demo {
    fn default() -> Self {
        Demo::new()
    }
}

impl Demo {
    pub fn new() -> Demo {
        let mut d = Demo {
            rng: Rng(0x9E37_79B9_7F4A_7C15),
            snap: Snapshot::default(),
            t: 0.0,
            hp_red: 12.0,
            quest_left: 38.0 * 60.0 + 50.0,
            next_enrage: 6.0,
            tired_left: 0.0,
            flinch: Vec::new(),
            scene: None,
        };
        d.reset();
        d
    }

    fn monster(&mut self, id: i32, uid: u64, hp_ratio: f32) -> Monster {
        let def = data().monster(id).clone();
        let hp_max = 42_800.0;
        let mut parts = Vec::new();
        for pd in &def.parts {
            let max = self.rng.range(900.0, 2000.0);
            let breaks_max = pd.thresholds.len() as i32;
            let kind = if pd.severable {
                PartKind::Severable
            } else if breaks_max > 0 {
                PartKind::Breakable
            } else {
                PartKind::Flinch
            };
            parts.push(Part {
                id: pd.id,
                key: pd.key.clone(),
                kind,
                flinch: max * self.rng.range(0.2, 1.0),
                flinch_max: max,
                durability: max * 3.0 * self.rng.range(0.3, 1.0),
                durability_max: max * 3.0,
                breaks: 0,
                breaks_max,
                tenderized: 0.0,
                tenderized_max: 90.0,
            });
        }
        self.flinch = parts.iter().map(|p| (p.flinch, p.flinch_max)).collect();
        let ailments = [
            "AILMENT_POISON",
            "AILMENT_PARALYSIS",
            "AILMENT_SLEEP",
            "AILMENT_BLAST",
            "AILMENT_STUN",
            "AILMENT_EXHAUST",
            "AILMENT_MOUNT",
            "AILMENT_SHOCKTRAP",
        ]
        .iter()
        .zip([180.0, 150.0, 150.0, 120.0, 220.0, 200.0, 260.0, 100.0])
        .map(|(k, max)| Ailment {
            key: k.to_string(),
            buildup: 0.0,
            buildup_max: max,
            timer: 0.0,
            timer_max: 10.0,
            count: 0,
        })
        .collect();
        Monster {
            uid,
            id,
            key: data().monster_key(id),
            hp: hp_max * hp_ratio,
            hp_max,
            stamina: 1800.0,
            stamina_max: 2000.0,
            capture_threshold: def.capture / 100.0,
            crown: if uid == 1 { Crown::Gold } else { Crown::None },
            size: if uid == 1 { 1.24 } else { 1.02 },
            weaknesses: def.weaknesses.clone(),
            parts,
            ailments,
            target: if uid == 1 { Target::Own } else { Target::Other },
            large: true,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        let mut monsters = vec![self.monster(TARGET, 1, 0.63)];
        for (i, id) in OTHERS.iter().enumerate() {
            let flinch = std::mem::take(&mut self.flinch);
            let m = self.monster(*id, 2 + i as u64, if i == 0 { 0.81 } else { 1.0 });
            self.flinch = flinch;
            monsters.push(m);
        }
        let player = Player {
            name: "mcwz".into(),
            hr: 999,
            mr: 384,
            hp: 128.0,
            hp_max: 150.0,
            hp_recoverable: 140.0,
            hp_healing_to: 128.0,
            hp_cap: 200.0,
            stamina: 120.0,
            stamina_max: 150.0,
            stamina_cap: 200.0,
            weapon: Weapon::Longsword,
            sharpness: Some(Sharpness {
                level: SharpnessLevel::White,
                current: 310,
                max: 340,
                thresholds: vec![50, 90, 130, 190, 260, 320, 340],
            }),
            weapon_state: WeaponState::Longsword {
                gauge: 62.0,
                level: 2,
                level_timer: 48.0,
                level_timer_max: 140.0,
            },
            abnormalities: vec![
                abn("Consumables_6CC-8", "ABNORMALITY_DEMONDRUG", AbnormalityKind::Consumable, 1.0, 1.0, true),
                abn("Consumables_6D0-8", "ABNORMALITY_ARMORSKIN", AbnormalityKind::Consumable, 1.0, 1.0, true),
                abn("Songs_3C", "ABNORMALITY_ATTACK_UP", AbnormalityKind::Song, 94.0, 120.0, false),
                abn("Consumables_6C4", "ABNORMALITY_DEMON_POWDER", AbnormalityKind::Consumable, 46.0, 90.0, false),
                abn("Foods_E4", "ABNORMALITY_FOOD_TIMER", AbnormalityKind::Food, 1500.0, 1800.0, false),
            ],
            tools: vec![
                Tool {
                    key: "TEMPORAL_MANTLE".into(),
                    timer: 0.0,
                    timer_max: 90.0,
                    cooldown: 0.0,
                    cooldown_max: 300.0,
                },
                Tool {
                    key: "ROCKSTEADY_MANTLE".into(),
                    timer: 0.0,
                    timer_max: 90.0,
                    cooldown: 0.0,
                    cooldown_max: 300.0,
                },
            ],
        };
        let party = [("mcwz", 0, true), ("Aster", 1, false), ("Kiri", 2, false), ("Momo", 3, false)]
            .iter()
            .map(|(n, slot, me)| PartyMember {
                name: n.to_string(),
                slot: *slot,
                weapon: Weapon::from_world_id(*slot as u8 * 3),
                damage: 0.0,
                is_self: *me,
                mr: 300,
                palico: false,
            })
            .collect();
        // Already some time into the fight: pre-accumulate damage at each member's damage rate
        let elapsed = 50.0 * 60.0 - self.quest_left;
        let mut party: Vec<PartyMember> = party;
        for (p, rate) in party.iter_mut().zip([72.0f32, 62.0, 48.0, 42.0]) {
            p.damage = rate * elapsed * self.rng.range(0.9, 1.1);
        }
        self.snap = Snapshot {
            game: Game::World,
            stage_id: 108,
            stage_key: "hoarfrost_reach".into(),
            in_hunt_zone: true,
            quest: Some(Quest {
                id: 66_831,
                stars: 4,
                rank: Rank::Master,
                status: QuestStatus::InProgress,
                time_left: self.quest_left,
                time_max: 50.0 * 60.0,
                deaths: 0,
                deaths_max: 3,
            }),
            world_time: Some((6, 40)),
            player,
            party,
            monsters,
            online: true,
            base: Some(Base {
                argosy_days: 3,
                argosy_in_town: true,
                tailraiders_quests: 2,
                tailraiders_deployed: true,
                harvest_items: 50,
                harvest_capacity: 50,
                fertilizers: vec![],
                steam_natural: 520,
                steam_natural_max: 700,
                steam_stored: 1200,
            }),
            ..Default::default()
        };
    }

    /// Advances `dt` seconds and returns the new snapshot.
    pub fn step(&mut self, dt: f32) -> Snapshot {
        self.t += dt as f64;
        if let Some((before, after)) = &self.scene {
            let mut s = if self.t < SCENE_SWITCH { before.clone() } else { after.clone() };
            s.time = self.t;
            return s;
        }
        let t = self.t as f32;
        let mut s = self.snap.clone();
        s.time = self.t;
        self.quest_left = (self.quest_left - dt).max(0.0);
        if let Some(q) = s.quest.as_mut() {
            q.time_left = self.quest_left;
            s.elapsed = q.time_max - q.time_left;
        }
        if let Some((h, m)) = s.world_time {
            let mins = (h as f32 * 60.0 + m as f32 + dt * 2.0) % 1440.0;
            s.world_time = Some(((mins / 60.0) as u8, (mins % 60.0) as u8));
        }

        // Party damage: different hand speeds by rank
        let rates = [1.2f32, 2.6, 3.2, 3.0];
        let avgs = [60.0f32, 24.0, 15.0, 14.0];
        let mut dealt = 0.0;
        for (i, member) in s.party.iter_mut().enumerate() {
            if self.rng.chance(rates[i] * dt) {
                let d = avgs[i] * self.rng.range(0.7, 1.3);
                member.damage += d;
                dealt += d;
            }
        }
        let me = s.party.iter().find(|p| p.is_self).map_or(0.0, |p| p.damage);
        s.damage_split = Some(DamageSplit {
            total: me,
            physical: Some(me * 0.71),
            elemental: Some(me * 0.29),
            critical: me * 0.38,
            hits: (me / 60.0) as u32,
            critical_hits: (me / 60.0 * 0.38) as u32,
        });

        let n_parts = s.monsters[0].parts.len().max(1);
        let target = &mut s.monsters[0];
        if dealt > 0.0 {
            target.hp = (target.hp - dealt).max(0.0);
            let pi = self.rng.pick(n_parts);
            if let Some(p) = target.parts.get_mut(pi) {
                p.flinch -= dealt * 0.6;
                if p.flinch <= 0.0 {
                    p.flinch = p.flinch_max;
                }
                if p.breaks < p.breaks_max {
                    p.durability -= dealt * 0.9;
                    if p.durability <= 0.0 {
                        p.breaks += 1;
                        p.durability = if p.breaks < p.breaks_max { p.durability_max } else { 0.0 };
                    }
                }
            }
            let ai = self.rng.pick(target.ailments.len());
            let a = &mut target.ailments[ai];
            if a.timer <= 0.0 {
                a.buildup += self.rng.range(2.0, 7.0);
                if a.buildup >= a.buildup_max {
                    a.buildup = 0.0;
                    a.buildup_max = (a.buildup_max * 1.35).round();
                    a.count += 1;
                    a.timer = a.timer_max;
                }
            }
        }
        for a in &mut target.ailments {
            a.timer = (a.timer - dt).max(0.0);
        }
        if target.enraged {
            target.enrage_timer = (target.enrage_timer - dt).max(0.0);
            if target.enrage_timer <= 0.0 {
                target.enraged = false;
                self.next_enrage = t + self.rng.range(15.0, 25.0);
            }
        } else if t >= self.next_enrage {
            target.enraged = true;
            target.enrage_timer = 24.0;
            target.enrage_timer_max = 24.0;
        }
        if self.tired_left > 0.0 {
            self.tired_left = (self.tired_left - dt).max(0.0);
            if self.tired_left <= 0.0 {
                target.stamina = target.stamina_max;
                target.tired = false;
            }
        } else {
            target.stamina -= if target.enraged { 24.0 } else { 10.0 } * dt;
            if target.stamina <= 0.0 {
                target.stamina = 0.0;
                target.tired = true;
                self.tired_left = 16.0;
            }
        }
        if target.hp <= 0.0 {
            target.hp = target.hp_max;
            for p in &mut target.parts {
                p.breaks = 0;
                p.durability = p.durability_max;
            }
        }

        // Self: takes hits now and then, red HP recovers slowly, drinks a potion when low
        let p = &mut s.player;
        if self.rng.chance(0.12 * dt) {
            let d = self.rng.range(18.0, 34.0);
            p.hp = (p.hp - d).max(1.0);
            self.hp_red = (self.hp_red + d * 0.6).min(p.hp_max - p.hp);
        }
        if self.hp_red > 0.0 {
            let r = (1.6 * dt).min(self.hp_red);
            p.hp = (p.hp + r).min(p.hp_max);
            self.hp_red -= r;
        }
        if p.hp < 40.0 && self.rng.chance(0.2 * dt) {
            p.hp_healing_to = (p.hp + 60.0).min(p.hp_max);
        }
        if p.hp_healing_to > p.hp {
            p.hp = (p.hp + 25.0 * dt).min(p.hp_healing_to);
        } else {
            p.hp_healing_to = p.hp;
        }
        p.hp_recoverable = (p.hp + self.hp_red).min(p.hp_max);
        p.stamina = (p.stamina + if self.rng.chance(0.3) { -22.0 } else { 30.0 } * dt).clamp(0.0, p.stamina_max);
        if let Some(sh) = p.sharpness.as_mut()
            && self.rng.chance(rates[0] * dt)
        {
            sh.current = (sh.current - 1).max(0);
            if sh.current <= 260 {
                sh.current = 330;
            }
            sh.level = crate::reader::current_sharpness(&sh.thresholds, sh.current).unwrap_or(sh.level);
        }
        for a in &mut p.abnormalities {
            if !a.infinite {
                a.timer -= dt;
                if a.timer <= 0.0 {
                    a.timer = a.timer_max;
                }
            }
        }
        if let WeaponState::Longsword {
            gauge,
            level,
            level_timer,
            level_timer_max,
        } = &mut p.weapon_state
        {
            *gauge = (*gauge + if self.rng.chance(0.2) { 6.0 } else { -0.4 }).clamp(0.0, 100.0);
            *level_timer = (*level_timer - dt).max(0.0);
            if *level_timer <= 0.0 && *level > 0 {
                *level -= 1;
                *level_timer = *level_timer_max;
            }
        }

        self.snap = s.clone();
        s
    }
}

/// Seconds into a scene when it switches from its first state to its second, so a screenshot taken
/// a little later catches the change (a break, an ailment proc, enrage) while it is still animating.
pub const SCENE_SWITCH: f64 = 2.4;

/// Names accepted by `Demo::scene`.
pub const SCENES: &[&str] = &[
    "hunt",
    "notarget",
    "enraged",
    "ailments",
    "breaks",
    "capture",
    "multi",
    "lowhp",
    "frostcraft",
    "debuff-poison",
    "debuff-venom",
    "debuff-fire",
    "debuff-thunder",
    "debuff-water",
    "debuff-ice",
    "debuff-dragon",
    "debuff-bleed",
    "debuff-effluvia",
    "debuff-blast",
];

fn set_ailment(m: &mut Monster, key: &str, count: i32, buildup: f32, active: f32) {
    if let Some(a) = m.ailments.iter_mut().find(|a| a.key == key) {
        a.count = count;
        a.buildup = buildup * a.buildup_max;
        a.timer = active * a.timer_max;
    }
}

impl Demo {
    /// A fixed state for screenshots: the demo hunt run for a while, then edited to show one situation.
    pub fn scene(name: &str) -> Option<Demo> {
        let mut d = Demo::new();
        let mut base = d.step(0.1);
        for _ in 0..300 {
            base = d.step(0.1);
        }
        let m = &mut base.monsters[0];
        m.enraged = false;
        m.tired = false;
        m.stamina = m.stamina_max * 0.7;
        m.hp = m.hp_max * 0.58;
        for a in &mut m.ailments {
            a.timer = 0.0;
            a.count = 0;
            a.buildup = a.buildup_max * 0.2;
        }
        base.player.hp = 118.0;
        base.player.hp_recoverable = 132.0;
        base.player.hp_healing_to = base.player.hp;
        let mut before = base.clone();
        let mut after = base;
        let both = |before: &mut Snapshot, after: &mut Snapshot, f: &dyn Fn(&mut Snapshot)| {
            f(before);
            f(after);
        };
        match name {
            "hunt" => {}
            "notarget" => both(&mut before, &mut after, &|s| {
                for m in &mut s.monsters {
                    m.target = Target::None;
                    m.manual_target = Target::None;
                }
            }),
            "enraged" => {
                let m = &mut after.monsters[0];
                m.enraged = true;
                m.enrage_timer = 24.0;
                m.enrage_timer_max = 24.0;
            }
            "ailments" => {
                both(&mut before, &mut after, &|s| {
                    let m = &mut s.monsters[0];
                    set_ailment(m, "AILMENT_POISON", 2, 0.0, 0.7);
                    set_ailment(m, "AILMENT_PARALYSIS", 1, 0.45, 0.0);
                    set_ailment(m, "AILMENT_BLAST", 3, 0.1, 0.0);
                    set_ailment(m, "AILMENT_STUN", 1, 0.3, 0.0);
                    set_ailment(m, "AILMENT_EXHAUST", 0, 0.55, 0.0);
                    set_ailment(m, "AILMENT_MOUNT", 0, 0.0, 0.0);
                    set_ailment(m, "AILMENT_SHOCKTRAP", 1, 0.0, 0.0);
                });
                set_ailment(&mut before.monsters[0], "AILMENT_SLEEP", 0, 0.92, 0.0);
                set_ailment(&mut after.monsters[0], "AILMENT_SLEEP", 1, 0.0, 1.0);
            }
            "breaks" => {
                both(&mut before, &mut after, &|s| {
                    for p in &mut s.monsters[0].parts {
                        p.breaks = 0;
                        p.durability = p.durability_max * 0.6;
                    }
                    let parts = &mut s.monsters[0].parts;
                    if let Some(p) = parts.iter_mut().find(|p| p.kind == PartKind::Severable) {
                        p.breaks = p.breaks_max;
                        p.durability = 0.0;
                    }
                    if let Some(p) = parts.iter_mut().filter(|p| p.kind == PartKind::Breakable).nth(1) {
                        p.breaks = 1;
                        p.durability = p.durability_max * 0.35;
                    }
                    if let Some(p) = parts.iter_mut().filter(|p| p.kind == PartKind::Breakable).nth(2) {
                        p.durability = p.durability_max * 0.05;
                    }
                });
                if let Some(i) = before.monsters[0].parts.iter().position(|p| p.kind == PartKind::Breakable) {
                    before.monsters[0].parts[i].durability = before.monsters[0].parts[i].durability_max * 0.03;
                    let p = &mut after.monsters[0].parts[i];
                    p.breaks += 1;
                    p.durability = if p.breaks < p.breaks_max { p.durability_max } else { 0.0 };
                }
            }
            "capture" => {
                before.monsters[0].hp = before.monsters[0].hp_max * 0.24;
                let m = &mut after.monsters[0];
                m.hp = m.hp_max * 0.14;
                m.tired = true;
                m.stamina = 0.0;
            }
            "multi" => both(&mut before, &mut after, &|s| {
                for (i, m) in s.monsters.iter_mut().enumerate() {
                    m.target = if i == 1 { Target::Own } else { Target::Other };
                }
                s.monsters[1].enraged = true;
                s.monsters[1].enrage_timer = 15.0;
                s.monsters[1].enrage_timer_max = 24.0;
            }),
            "lowhp" => both(&mut before, &mut after, &|s| {
                s.player.hp = 31.0;
                s.player.hp_recoverable = 58.0;
                s.player.hp_healing_to = 31.0;
            }),
            "frostcraft" => both(&mut before, &mut after, &|s| {
                let a = abn("Skills_788", "ABNORMALITY_FROSTCRAFT", AbnormalityKind::Skill, 42.0, 60.0, false);
                s.player.abnormalities.push(a);
            }),
            other => {
                let key = other.strip_prefix("debuff-")?.to_uppercase();
                if !SCENES.contains(&other) {
                    return None;
                }
                both(&mut before, &mut after, &|s| {
                    let a = abn("Debuffs_0", &format!("ABNORMALITY_{key}"), AbnormalityKind::Debuff, 18.0, 30.0, false);
                    s.player.abnormalities.push(a);
                });
            }
        }
        d.t = 0.0;
        d.scene = Some((before, after));
        Some(d)
    }
}

fn abn(id: &str, key: &str, kind: AbnormalityKind, timer: f32, max: f32, infinite: bool) -> Abnormality {
    Abnormality {
        id: id.into(),
        key: key.into(),
        kind,
        timer,
        timer_max: max,
        infinite,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_runs_and_changes() {
        let mut d = Demo::new();
        let a = d.step(0.1);
        let mut b = a.clone();
        for _ in 0..600 {
            b = d.step(0.1);
        }
        assert!(b.in_quest());
        assert_ne!(b.monsters[0].hp, a.monsters[0].hp);
        assert!(b.party.iter().map(|p| p.damage).sum::<f32>() > 0.0);
        assert!(!b.monsters[0].parts.is_empty());
    }

    #[test]
    fn scenes_build() {
        for name in SCENES {
            let mut d = Demo::scene(name).unwrap_or_else(|| panic!("{name}"));
            let a = d.step(0.1);
            assert!(!a.monsters.is_empty(), "{name}");
        }
        assert!(Demo::scene("nope").is_none());
        let mut d = Demo::scene("breaks").unwrap();
        let a = d.step(0.1);
        let b = d.step(SCENE_SWITCH as f32);
        let broke = |s: &Snapshot| s.monsters[0].parts.iter().map(|p| p.breaks).sum::<i32>();
        assert_eq!(broke(&b), broke(&a) + 1);
    }
}
