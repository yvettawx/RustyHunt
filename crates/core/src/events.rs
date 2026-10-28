//! Compares two consecutive snapshots to produce events. The event list is in `docs/events.md`; plugins can subscribe.

use serde::{Deserialize, Serialize};

use crate::model::{QuestStatus, Snapshot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    QuestStart { quest_id: i32, stars: i32 },
    QuestEnd { quest_id: i32, status: QuestStatus, elapsed: f32 },
    MonsterSpawn { uid: u64, key: String },
    MonsterDespawn { uid: u64, key: String },
    MonsterDeath { uid: u64, key: String },
    MonsterCapture { uid: u64, key: String },
    PartBreak { uid: u64, part: String, count: i32 },
    PartSever { uid: u64, part: String },
    AilmentTrigger { uid: u64, ailment: String, count: i32 },
    Enrage { uid: u64 },
    EnrageEnd { uid: u64 },
    Tired { uid: u64 },
    Capturable { uid: u64 },
    PlayerDown { deaths: i32, deaths_max: i32 },
    StageChange { stage_id: i32 },
    WeaponChange { weapon: String },
    AbnormalityStart { id: String, key: String },
    AbnormalityEnd { id: String, key: String },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::QuestStart { .. } => "quest_start",
            Event::QuestEnd { .. } => "quest_end",
            Event::MonsterSpawn { .. } => "monster_spawn",
            Event::MonsterDespawn { .. } => "monster_despawn",
            Event::MonsterDeath { .. } => "monster_death",
            Event::MonsterCapture { .. } => "monster_capture",
            Event::PartBreak { .. } => "part_break",
            Event::PartSever { .. } => "part_sever",
            Event::AilmentTrigger { .. } => "ailment_trigger",
            Event::Enrage { .. } => "enrage",
            Event::EnrageEnd { .. } => "enrage_end",
            Event::Tired { .. } => "tired",
            Event::Capturable { .. } => "capturable",
            Event::PlayerDown { .. } => "player_down",
            Event::StageChange { .. } => "stage_change",
            Event::WeaponChange { .. } => "weapon_change",
            Event::AbnormalityStart { .. } => "abnormality_start",
            Event::AbnormalityEnd { .. } => "abnormality_end",
        }
    }

    pub const NAMES: [&'static str; 18] = [
        "quest_start",
        "quest_end",
        "monster_spawn",
        "monster_despawn",
        "monster_death",
        "monster_capture",
        "part_break",
        "part_sever",
        "ailment_trigger",
        "enrage",
        "enrage_end",
        "tired",
        "capturable",
        "player_down",
        "stage_change",
        "weapon_change",
        "abnormality_start",
        "abnormality_end",
    ];
}

/// Version of the event list; bump the minor version when adding events.
pub const EVENTS_VERSION: &str = "1.0";

pub fn diff(prev: &Snapshot, cur: &Snapshot) -> Vec<Event> {
    let mut out = Vec::new();

    if prev.stage_id != cur.stage_id {
        out.push(Event::StageChange { stage_id: cur.stage_id });
    }

    let was = prev.quest.as_ref().filter(|q| q.status == QuestStatus::InProgress);
    let now = cur.quest.as_ref().filter(|q| q.status == QuestStatus::InProgress);
    match (was, now) {
        (None, Some(q)) => out.push(Event::QuestStart {
            quest_id: q.id,
            stars: q.stars,
        }),
        (Some(q), None) => out.push(Event::QuestEnd {
            quest_id: q.id,
            status: cur.quest.as_ref().filter(|n| n.id == q.id).map(|n| n.status).unwrap_or(QuestStatus::Quit),
            elapsed: prev.elapsed,
        }),
        (Some(a), Some(b)) if a.id != b.id => {
            out.push(Event::QuestEnd {
                quest_id: a.id,
                status: QuestStatus::Quit,
                elapsed: prev.elapsed,
            });
            out.push(Event::QuestStart {
                quest_id: b.id,
                stars: b.stars,
            });
        }
        _ => {}
    }

    if let (Some(a), Some(b)) = (&prev.quest, &cur.quest)
        && a.id == b.id
        && b.deaths > a.deaths
    {
        out.push(Event::PlayerDown {
            deaths: b.deaths,
            deaths_max: b.deaths_max,
        });
    }

    if prev.player.weapon != cur.player.weapon {
        out.push(Event::WeaponChange {
            weapon: cur.player.weapon.key().to_string(),
        });
    }

    for a in &cur.player.abnormalities {
        if !prev.player.abnormalities.iter().any(|p| p.id == a.id) {
            out.push(Event::AbnormalityStart {
                id: a.id.clone(),
                key: a.key.clone(),
            });
        }
    }
    for a in &prev.player.abnormalities {
        if !cur.player.abnormalities.iter().any(|c| c.id == a.id) {
            out.push(Event::AbnormalityEnd {
                id: a.id.clone(),
                key: a.key.clone(),
            });
        }
    }

    for m in cur.monsters.iter().filter(|m| m.large) {
        let Some(p) = prev.monsters.iter().find(|p| p.uid == m.uid) else {
            out.push(Event::MonsterSpawn {
                uid: m.uid,
                key: m.key.clone(),
            });
            continue;
        };
        let uid = m.uid;

        if p.hp > 0.0 && m.hp <= 0.0 && !m.captured {
            out.push(Event::MonsterDeath { uid, key: m.key.clone() });
        }
        if !p.captured && m.captured {
            out.push(Event::MonsterCapture { uid, key: m.key.clone() });
        }
        if !p.enraged && m.enraged {
            out.push(Event::Enrage { uid });
        }
        if p.enraged && !m.enraged {
            out.push(Event::EnrageEnd { uid });
        }
        if !p.tired && m.tired {
            out.push(Event::Tired { uid });
        }
        if !p.capturable() && m.capturable() && m.alive() {
            out.push(Event::Capturable { uid });
        }

        for part in &m.parts {
            let Some(old) = p.parts.iter().find(|x| x.id == part.id && x.key == part.key) else {
                continue;
            };
            if part.breaks > old.breaks && part.breaks_max > 0 {
                // Cutting a tail off; severable parts with several breaks (horns) break
                if part.kind == crate::model::PartKind::Severable && part.breaks_max <= 1 {
                    out.push(Event::PartSever { uid, part: part.key.clone() });
                } else {
                    out.push(Event::PartBreak {
                        uid,
                        part: part.key.clone(),
                        count: part.breaks,
                    });
                }
            }
        }

        for ail in &m.ailments {
            let Some(old) = p.ailments.iter().find(|x| x.key == ail.key) else {
                continue;
            };
            if ail.count > old.count {
                out.push(Event::AilmentTrigger {
                    uid,
                    ailment: ail.key.clone(),
                    count: ail.count,
                });
            }
        }
    }

    for p in prev.monsters.iter().filter(|m| m.large) {
        if !cur.monsters.iter().any(|m| m.uid == p.uid) {
            out.push(Event::MonsterDespawn {
                uid: p.uid,
                key: p.key.clone(),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn monster() -> Monster {
        Monster {
            uid: 1,
            key: "rathalos".into(),
            hp: 100.0,
            hp_max: 100.0,
            large: true,
            capture_threshold: 0.2,
            parts: vec![Part {
                id: 1,
                key: "PART_HEAD".into(),
                kind: PartKind::Breakable,
                breaks_max: 1,
                ..Default::default()
            }],
            ailments: vec![Ailment {
                key: "AILMENT_POISON".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn detects_spawn_break_ailment_enrage() {
        let a = Snapshot::default();
        let mut b = Snapshot {
            monsters: vec![monster()],
            ..Default::default()
        };
        assert_eq!(
            diff(&a, &b),
            vec![Event::MonsterSpawn {
                uid: 1,
                key: "rathalos".into()
            }]
        );

        let mut c = b.clone();
        c.monsters[0].parts[0].breaks = 1;
        c.monsters[0].ailments[0].count = 1;
        c.monsters[0].enraged = true;
        c.monsters[0].hp = 10.0;
        let ev = diff(&b, &c);
        assert!(ev.contains(&Event::PartBreak {
            uid: 1,
            part: "PART_HEAD".into(),
            count: 1
        }));
        assert!(ev.contains(&Event::AilmentTrigger {
            uid: 1,
            ailment: "AILMENT_POISON".into(),
            count: 1
        }));
        assert!(ev.contains(&Event::Enrage { uid: 1 }));
        assert!(ev.contains(&Event::Capturable { uid: 1 }));

        b.monsters.clear();
        assert!(diff(&c, &b).contains(&Event::MonsterDespawn {
            uid: 1,
            key: "rathalos".into()
        }));
    }

    #[test]
    fn detects_quest_start_end_and_deaths() {
        let a = Snapshot::default();
        let q = Quest {
            id: 501,
            stars: 12,
            status: QuestStatus::InProgress,
            deaths_max: 3,
            ..Default::default()
        };
        let b = Snapshot {
            quest: Some(q.clone()),
            elapsed: 60.0,
            ..Default::default()
        };
        assert_eq!(diff(&a, &b), vec![Event::QuestStart { quest_id: 501, stars: 12 }]);

        let mut c = b.clone();
        c.quest.as_mut().unwrap().deaths = 1;
        assert_eq!(diff(&b, &c), vec![Event::PlayerDown { deaths: 1, deaths_max: 3 }]);

        let mut d = c.clone();
        d.quest.as_mut().unwrap().status = QuestStatus::Success;
        assert_eq!(
            diff(&c, &d),
            vec![Event::QuestEnd {
                quest_id: 501,
                status: QuestStatus::Success,
                elapsed: 60.0
            }]
        );
    }
}
