//! 狩猎记录：每场狩猎在任务结束时写进本地的 `records.db`。
//! SQLite 开 WAL，写入中途断电也不会写坏；插件写的自定义字段单独一张表。

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rh_core::Event;
use rh_core::model::{Crown, QuestStatus, Rank, Snapshot};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::scanner::Sink;
use crate::state::Shared;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS hunts (
    id INTEGER PRIMARY KEY,
    game TEXT NOT NULL,
    quest_id INTEGER NOT NULL,
    stars INTEGER NOT NULL,
    rank TEXT NOT NULL,
    monster TEXT NOT NULL,
    monsters TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    duration REAL NOT NULL,
    result TEXT NOT NULL,
    crown TEXT NOT NULL DEFAULT 'none',
    size REAL NOT NULL DEFAULT 0,
    map TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS hunts_by_game ON hunts(game, started_at);
CREATE INDEX IF NOT EXISTS hunts_by_monster ON hunts(monster);
CREATE TABLE IF NOT EXISTS hunt_players (
    hunt_id INTEGER NOT NULL REFERENCES hunts(id) ON DELETE CASCADE,
    slot INTEGER NOT NULL,
    name TEXT NOT NULL,
    weapon TEXT NOT NULL,
    damage REAL NOT NULL,
    is_self INTEGER NOT NULL,
    crit REAL
);
CREATE INDEX IF NOT EXISTS players_by_hunt ON hunt_players(hunt_id);
CREATE TABLE IF NOT EXISTS hunt_curve (
    hunt_id INTEGER NOT NULL REFERENCES hunts(id) ON DELETE CASCADE,
    t REAL NOT NULL,
    slot INTEGER NOT NULL,
    dps REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS curve_by_hunt ON hunt_curve(hunt_id);
CREATE TABLE IF NOT EXISTS hunt_events (
    hunt_id INTEGER NOT NULL REFERENCES hunts(id) ON DELETE CASCADE,
    t REAL NOT NULL,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    accent INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS events_by_hunt ON hunt_events(hunt_id);
CREATE TABLE IF NOT EXISTS plugin_fields (
    hunt_id INTEGER NOT NULL REFERENCES hunts(id) ON DELETE CASCADE,
    plugin TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL
);
";

pub fn db_path() -> PathBuf {
    rh_core::paths::records().join("records.db")
}

pub fn open() -> rusqlite::Result<Connection> {
    let c = Connection::open(db_path())?;
    c.execute_batch(SCHEMA)?;
    c.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(c)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub slot: i32,
    pub name: String,
    pub weapon: String,
    pub damage: f32,
    pub is_self: bool,
    pub crit: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventRow {
    pub t: f32,
    pub kind: String,
    /// 本地化键或者文本，例如 PART_HEAD、AILMENT_POISON
    pub label: String,
    pub accent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hunt {
    pub id: i64,
    pub game: String,
    pub quest_id: i32,
    pub stars: i32,
    pub rank: String,
    pub monster: String,
    pub monsters: Vec<String>,
    pub started_at: i64,
    pub duration: f32,
    /// slay、capture、fail、quit
    pub result: String,
    pub crown: String,
    pub size: f32,
    pub map: String,
    pub players: Vec<Player>,
    /// (秒, 栏位, DPS)
    pub curve: Vec<(f32, i32, f32)>,
    pub events: Vec<EventRow>,
    pub fields: BTreeMap<String, String>,
}

impl Hunt {
    pub fn self_dps(&self) -> f32 {
        self.players.iter().find(|p| p.is_self).map_or(0.0, |p| p.damage / self.duration.max(1.0))
    }
}

/// 列表页用的摘要：不带曲线和事件。
pub fn list(conn: &Connection, game: Option<&str>, limit: usize) -> rusqlite::Result<Vec<Hunt>> {
    let mut stmt = conn.prepare(
        "SELECT id, game, quest_id, stars, rank, monster, monsters, started_at, duration, result, crown, size, map
         FROM hunts WHERE (?1 IS NULL OR game = ?1) ORDER BY started_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![game, limit as i64], |r| {
        Ok(Hunt {
            id: r.get(0)?,
            game: r.get(1)?,
            quest_id: r.get(2)?,
            stars: r.get(3)?,
            rank: r.get(4)?,
            monster: r.get(5)?,
            monsters: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
            started_at: r.get(7)?,
            duration: r.get(8)?,
            result: r.get(9)?,
            crown: r.get(10)?,
            size: r.get(11)?,
            map: r.get(12)?,
            players: Vec::new(),
            curve: Vec::new(),
            events: Vec::new(),
            fields: BTreeMap::new(),
        })
    })?;
    let mut out: Vec<Hunt> = rows.collect::<Result<_, _>>()?;
    // 摘要里也带上玩家，列表要显示自己的 DPS
    let mut pstmt = conn.prepare("SELECT slot, name, weapon, damage, is_self, crit FROM hunt_players WHERE hunt_id = ?1 ORDER BY damage DESC")?;
    for h in &mut out {
        h.players = pstmt
            .query_map([h.id], |r| {
                Ok(Player {
                    slot: r.get(0)?,
                    name: r.get(1)?,
                    weapon: r.get(2)?,
                    damage: r.get(3)?,
                    is_self: r.get::<_, i32>(4)? != 0,
                    crit: r.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;
    }
    Ok(out)
}

pub fn detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<Hunt>> {
    let Some(mut h) = list(conn, None, 100_000)?.into_iter().find(|h| h.id == id) else {
        return Ok(None);
    };
    let mut c = conn.prepare("SELECT t, slot, dps FROM hunt_curve WHERE hunt_id = ?1 ORDER BY t")?;
    h.curve = c.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut e = conn.prepare("SELECT t, kind, label, accent FROM hunt_events WHERE hunt_id = ?1 ORDER BY t")?;
    h.events = e
        .query_map([id], |r| {
            Ok(EventRow {
                t: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                accent: r.get::<_, i32>(3)? != 0,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut f = conn.prepare("SELECT plugin, key, value FROM plugin_fields WHERE hunt_id = ?1")?;
    h.fields = f
        .query_map([id], |r| Ok((format!("{}.{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?), r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    Ok(Some(h))
}

pub fn insert(conn: &mut Connection, h: &Hunt) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO hunts (game, quest_id, stars, rank, monster, monsters, started_at, duration, result, crown, size, map)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            h.game,
            h.quest_id,
            h.stars,
            h.rank,
            h.monster,
            serde_json::to_string(&h.monsters).unwrap_or_default(),
            h.started_at,
            h.duration,
            h.result,
            h.crown,
            h.size,
            h.map
        ],
    )?;
    let id = tx.last_insert_rowid();
    for p in &h.players {
        tx.execute(
            "INSERT INTO hunt_players (hunt_id, slot, name, weapon, damage, is_self, crit) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, p.slot, p.name, p.weapon, p.damage, p.is_self as i32, p.crit],
        )?;
    }
    for (t, slot, dps) in &h.curve {
        tx.execute(
            "INSERT INTO hunt_curve (hunt_id, t, slot, dps) VALUES (?1, ?2, ?3, ?4)",
            params![id, t, slot, dps],
        )?;
    }
    for e in &h.events {
        tx.execute(
            "INSERT INTO hunt_events (hunt_id, t, kind, label, accent) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, e.t, e.kind, e.label, e.accent as i32],
        )?;
    }
    for (k, v) in &h.fields {
        let (plugin, key) = k.split_once('.').unwrap_or(("", k));
        tx.execute(
            "INSERT INTO plugin_fields (hunt_id, plugin, key, value) VALUES (?1, ?2, ?3, ?4)",
            params![id, plugin, key, v],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM hunts WHERE id = ?1", [id])?;
    Ok(())
}

/// 插件往当前这场狩猎写的字段，任务结束时一起保存。
pub static PENDING_FIELDS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

struct Running {
    started_at: i64,
    quest_id: i32,
    stars: i32,
    rank: Rank,
    map: String,
    events: Vec<EventRow>,
    curve: Vec<(f32, i32, f32)>,
    last_sample: f32,
    /// 每只怪物开始时的血量，用来判断主目标
    hp_start: HashMap<u64, f32>,
    last: Option<Arc<Snapshot>>,
}

pub struct Recorder {
    conn: Option<Connection>,
    running: Option<Running>,
}

impl Recorder {
    pub fn new() -> Recorder {
        let conn = match open() {
            Ok(c) => Some(c),
            Err(e) => {
                log::error!("打开 records.db 失败：{e}");
                None
            }
        };
        Recorder { conn, running: None }
    }

    fn finish(&mut self, shared: &Shared, status: QuestStatus, elapsed: f32) {
        let Some(r) = self.running.take() else { return };
        let Some(last) = r.last.clone() else { return };
        let fields = std::mem::take(&mut *PENDING_FIELDS.lock().unwrap());
        // 主目标：这一场掉血最多的大型怪物
        let main = last
            .large_monsters()
            .max_by(|a, b| {
                let da = r.hp_start.get(&a.uid).copied().unwrap_or(a.hp_max) - a.hp;
                let db = r.hp_start.get(&b.uid).copied().unwrap_or(b.hp_max) - b.hp;
                da.total_cmp(&db)
            })
            .cloned();
        let Some(main) = main else {
            log::info!("这场任务没有大型怪物，不记录");
            return;
        };
        let result = match status {
            QuestStatus::Success if main.captured => "capture",
            QuestStatus::Success => "slay",
            QuestStatus::Fail => "fail",
            _ => "quit",
        };
        let crit = last.damage_split.as_ref().map(|d| d.crit_rate());
        let hunt = Hunt {
            id: 0,
            game: last.game.key().into(),
            quest_id: r.quest_id,
            stars: r.stars,
            rank: match r.rank {
                Rank::Low => "low",
                Rank::High => "high",
                Rank::Master => "master",
            }
            .into(),
            monster: main.key.clone(),
            monsters: last.large_monsters().map(|m| m.key.clone()).collect(),
            started_at: r.started_at,
            duration: elapsed.max(1.0),
            result: result.into(),
            crown: match main.crown {
                Crown::Gold => "gold",
                Crown::Silver => "silver",
                Crown::Mini => "mini",
                Crown::None => "none",
            }
            .into(),
            size: main.size,
            map: r.map,
            players: last
                .party
                .iter()
                .map(|p| Player {
                    slot: p.slot,
                    name: p.name.clone(),
                    weapon: p.weapon.key().into(),
                    damage: p.damage,
                    is_self: p.is_self,
                    crit: if p.is_self { crit } else { None },
                })
                .collect(),
            curve: r.curve,
            events: r.events,
            fields,
        };
        if let Some(conn) = self.conn.as_mut() {
            match insert(conn, &hunt) {
                Ok(id) => log::info!("任务结束 {}，已保存记录 #{id}", crate::overlay::mmss(elapsed)),
                Err(e) => log::error!("保存狩猎记录失败：{e}"),
            }
        }
        if matches!(result, "slay" | "capture") {
            crate::bestiary::on_hunt_finished(shared, &main.key, main.crown, main.size);
        }
        shared.repaint();
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Recorder::new()
    }
}

impl Sink for Recorder {
    fn frame(&mut self, shared: &Shared, snap: &Arc<Snapshot>, events: &[Event]) {
        // 演示数据不记录
        if matches!(shared.live().connection, crate::state::Connection::Demo) {
            return;
        }
        for e in events {
            match e {
                Event::QuestStart { quest_id, stars } => {
                    let q = snap.quest.clone().unwrap_or_default();
                    self.running = Some(Running {
                        started_at: chrono::Local::now().timestamp(),
                        quest_id: *quest_id,
                        stars: *stars,
                        rank: q.rank,
                        map: snap.stage_key.clone(),
                        events: Vec::new(),
                        curve: Vec::new(),
                        last_sample: -5.0,
                        hp_start: snap.large_monsters().map(|m| (m.uid, m.hp)).collect(),
                        last: None,
                    });
                    PENDING_FIELDS.lock().unwrap().clear();
                    log::info!("进入任务 {quest_id}");
                }
                Event::QuestEnd { status, elapsed, .. } => {
                    let status = *status;
                    let elapsed = *elapsed;
                    self.finish(shared, status, elapsed);
                }
                _ => {}
            }
        }
        let Some(r) = self.running.as_mut() else { return };
        let t = snap.elapsed;
        for m in snap.large_monsters() {
            r.hp_start.entry(m.uid).or_insert(m.hp_max);
        }
        for e in events {
            let row = match e {
                Event::PartBreak { part, .. } => Some(("break", part.clone(), true)),
                Event::PartSever { part, .. } => Some(("sever", part.clone(), true)),
                Event::AilmentTrigger { ailment, .. } => Some(("ailment", ailment.clone(), false)),
                Event::Enrage { .. } => Some(("enrage", String::new(), false)),
                Event::Tired { .. } => Some(("tired", String::new(), false)),
                Event::MonsterCapture { key, .. } => Some(("capture", key.clone(), true)),
                Event::MonsterDeath { key, .. } => Some(("slay", key.clone(), true)),
                Event::PlayerDown { .. } => Some(("down", String::new(), false)),
                _ => None,
            };
            if let Some((kind, label, accent)) = row {
                r.events.push(EventRow {
                    t,
                    kind: kind.into(),
                    label,
                    accent,
                });
            }
        }
        if t - r.last_sample >= 5.0 {
            r.last_sample = t;
            for p in &snap.party {
                r.curve.push((t, p.slot, p.damage / t.max(1.0)));
            }
        }
        r.last = Some(snap.clone());
    }

    fn detached(&mut self, shared: &Shared) {
        // 游戏中途关掉：按放弃记录
        if let Some(r) = &self.running {
            let elapsed = r.last.as_ref().map_or(0.0, |s| s.elapsed);
            self.finish(shared, QuestStatus::Quit, elapsed);
        }
    }
}

/// 导出 CSV：每场一行，玩家展开成列。
pub fn to_csv(hunts: &[Hunt]) -> String {
    let i = rh_core::i18n::current();
    let mut out = String::from("\u{feff}id,date,game,monster,rank,stars,duration_s,result,crown,size,player,weapon,damage,dps,share\n");
    for h in hunts {
        let total: f32 = h.players.iter().map(|p| p.damage).sum::<f32>().max(1.0);
        let date = chrono::DateTime::from_timestamp(h.started_at, 0)
            .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        for p in &h.players {
            out.push_str(&format!(
                "{},{},{},{},{},{},{:.0},{},{},{:.2},\"{}\",{},{:.0},{:.1},{:.3}\n",
                h.id,
                date,
                h.game,
                i.monster(&h.monster),
                h.rank,
                h.stars,
                h.duration,
                h.result,
                h.crown,
                h.size,
                p.name.replace('"', "\"\""),
                p.weapon,
                p.damage,
                p.damage / h.duration.max(1.0),
                p.damage / total
            ));
        }
    }
    out
}

pub fn to_json(hunts: &[Hunt]) -> String {
    serde_json::to_string_pretty(hunts).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Hunt {
        Hunt {
            id: 0,
            game: "world".into(),
            quest_id: 501,
            stars: 4,
            rank: "master".into(),
            monster: "raging_brachydios".into(),
            monsters: vec!["raging_brachydios".into()],
            started_at: 1_700_000_000,
            duration: 680.0,
            result: "slay".into(),
            crown: "gold".into(),
            size: 1.23,
            map: String::new(),
            players: vec![Player {
                slot: 0,
                name: "mcwz".into(),
                weapon: "longsword".into(),
                damage: 28000.0,
                is_self: true,
                crit: Some(0.38),
            }],
            curve: vec![(5.0, 0, 300.0), (10.0, 0, 320.0)],
            events: vec![EventRow {
                t: 120.0,
                kind: "break".into(),
                label: "PART_HEAD".into(),
                accent: true,
            }],
            fields: [("speedrun.split".to_string(), "1:23".to_string())].into_iter().collect(),
        }
    }

    #[test]
    fn insert_and_read_back() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(SCHEMA).unwrap();
        let id = insert(&mut c, &sample()).unwrap();
        let all = list(&c, Some("world"), 10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].players.len(), 1);
        let d = detail(&c, id).unwrap().unwrap();
        assert_eq!(d.curve.len(), 2);
        assert_eq!(d.events[0].label, "PART_HEAD");
        assert_eq!(d.fields.get("speedrun.split").map(String::as_str), Some("1:23"));
        assert!((d.self_dps() - 28000.0 / 680.0).abs() < 0.01);
        delete(&c, id).unwrap();
        assert!(list(&c, None, 10).unwrap().is_empty());
    }

    #[test]
    fn csv_has_header_and_rows() {
        let csv = to_csv(&[sample()]);
        assert!(csv.lines().count() == 2);
        assert!(csv.contains("mcwz"));
    }
}
