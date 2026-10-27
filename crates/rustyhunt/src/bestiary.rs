//! Hunter's Notes: read from the Steam save (read-only), with manual entry when the save can't be read.
//! When a hunt ends with a crown-size monster, it lights up in the notes automatically and a notification is sent.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use game_world::save::{Hunter, SaveFile};
use rh_core::model::{Crown, Game};
use serde::{Deserialize, Serialize};

use crate::notify::{Kind, Notification};
use crate::state::Shared;

const DATA: &str = include_str!("../../../data/world/bestiary.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Info {
    /// Position in the in-game research level list
    pub order: u16,
    pub species: String,
    /// Research level cap
    pub research: u8,
    #[serde(default)]
    pub maps: String,
    #[serde(default)]
    pub fixed: bool,
}

pub fn info() -> &'static BTreeMap<String, Info> {
    static I: OnceLock<BTreeMap<String, Info>> = OnceLock::new();
    I.get_or_init(|| toml::from_str(DATA).expect("内置图鉴数据格式正确"))
}

const CROWN_QUESTS: &str = include_str!("../../../data/world/crown_quests.toml");

/// A quest with a raised chance of a gold or mini crown for one of its monsters.
#[derive(Debug, Clone, Deserialize)]
pub struct CrownQuest {
    pub monster: String,
    /// `gold` or `mini`
    pub crown: String,
    /// Percent
    pub chance: u8,
    #[serde(rename = "zh-Hans")]
    zh_hans: String,
    #[serde(rename = "zh-Hant")]
    zh_hant: String,
    ja: String,
    en: String,
}

impl CrownQuest {
    pub fn name(&self, lang: rh_core::i18n::Lang) -> &str {
        use rh_core::i18n::Lang;
        match lang {
            Lang::ZhHans => &self.zh_hans,
            Lang::ZhHant => &self.zh_hant,
            Lang::Ja => &self.ja,
            Lang::En => &self.en,
        }
    }

    pub fn guaranteed(&self) -> bool {
        self.chance >= 100
    }

    pub fn gold(&self) -> bool {
        self.crown == "gold"
    }
}

#[derive(Deserialize)]
struct CrownQuests {
    quest: Vec<CrownQuest>,
}

pub fn crown_quests() -> &'static [CrownQuest] {
    static Q: OnceLock<Vec<CrownQuest>> = OnceLock::new();
    Q.get_or_init(|| toml::from_str::<CrownQuests>(CROWN_QUESTS).expect("bundled crown quest data is valid").quest)
}

/// Crown quests for one monster: guaranteed ones first, then by chance.
pub fn crown_quests_for(key: &str) -> Vec<&'static CrownQuest> {
    let mut v: Vec<&CrownQuest> = crown_quests().iter().filter(|q| q.monster == key).collect();
    v.sort_by_key(|q| std::cmp::Reverse(q.chance));
    v
}

/// One quest as shown in the notes table.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestLine<'a> {
    pub name: &'a str,
    pub gold: bool,
    pub mini: bool,
    /// Percent; the highest chance among the crowns on this line
    pub chance: u8,
}

/// Groups a monster's crown quests into lines. For each crown, only guaranteed quests are kept
/// when there are any; otherwise every quest with a raised chance is.
pub fn quest_lines<'a>(quests: &[&'a CrownQuest], lang: rh_core::i18n::Lang) -> Vec<QuestLine<'a>> {
    let mut lines: Vec<QuestLine> = Vec::new();
    for gold in [true, false] {
        let of: Vec<&&CrownQuest> = quests.iter().filter(|q| q.gold() == gold).collect();
        let sure = of.iter().any(|q| q.guaranteed());
        for q in of.into_iter().filter(|q| !sure || q.guaranteed()) {
            let name = q.name(lang);
            let i = match lines.iter().position(|l| l.name == name) {
                Some(i) => i,
                None => {
                    lines.push(QuestLine {
                        name,
                        gold: false,
                        mini: false,
                        chance: 0,
                    });
                    lines.len() - 1
                }
            };
            let l = &mut lines[i];
            if gold {
                l.gold = true;
            } else {
                l.mini = true;
            }
            l.chance = l.chance.max(q.chance);
        }
    }
    lines.sort_by(|a, b| b.chance.cmp(&a.chance).then((b.gold && b.mini).cmp(&(a.gold && a.mini))));
    lines
}

/// Map codes to the maps in the term table.
pub fn map_keys(code: &str) -> Vec<&'static str> {
    code.chars()
        .filter_map(|c| {
            Some(match c {
                'A' => "ancient_forest",
                'W' => "wildspire_waste",
                'C' => "coral_highlands",
                'R' => "rotten_vale",
                'E' => "elders_recess",
                'H' => "hoarfrost_reach",
                'G' => "guiding_lands",
                _ => return None,
            })
        })
        .collect()
}

/// Manually entered values, which override the save.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Manual {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub big: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Store {
    pub world: BTreeMap<String, Manual>,
}

fn store_file() -> PathBuf {
    rh_core::paths::records().join("bestiary.json")
}

impl Store {
    pub fn load() -> Store {
        std::fs::read_to_string(store_file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(store_file(), t);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub species: String,
    pub maps: Vec<&'static str>,
    pub fixed: bool,
    pub big: bool,
    pub small: bool,
    pub research: u8,
    pub research_cap: u8,
    pub nav: u8,
    pub slain: u16,
    pub captured: u16,
    pub largest: u16,
    pub smallest: u16,
}

impl Row {
    pub fn done(&self) -> bool {
        (self.fixed || (self.big && self.small)) && self.research >= self.research_cap && self.nav >= 3
    }
}

#[derive(Default)]
pub struct Book {
    pub save: Option<SaveFile>,
    pub path: Option<PathBuf>,
    /// When the game last wrote the save file (Unix seconds)
    pub saved_at: Option<i64>,
    /// When the file's write time was last checked
    checked: Option<std::time::Instant>,
    pub error: Option<String>,
    pub hunter: usize,
    pub reading: bool,
    pub store: Store,
    loaded: bool,
}

pub static BOOK: Mutex<Option<Book>> = Mutex::new(None);

pub fn with<R>(f: impl FnOnce(&mut Book) -> R) -> R {
    let mut g = BOOK.lock().unwrap();
    let b = g.get_or_insert_with(Book::default);
    if !b.loaded {
        b.loaded = true;
        b.store = Store::load();
    }
    f(b)
}

/// Save path: the Steam account chosen in settings; if none, the first account with a World save.
pub fn save_path(account: Option<u32>) -> Option<PathBuf> {
    let steam = platform::steam::steam_dir()?;
    let app = Game::World.steam_app_id();
    let file = |id: u32| platform::steam::remote_dir(&steam, id, app).join("SAVEDATA1000");
    if let Some(id) = account {
        return Some(file(id));
    }
    platform::steam::accounts(&steam).into_iter().map(|a| file(a.account_id)).find(|p| p.is_file())
}

/// In-game ID to crown thresholds, looked up through the term table key.
fn thresholds(key: &str) -> (f32, f32) {
    let d = game_world::data::data();
    d.monster_keys
        .iter()
        .find(|(_, k)| *k == key)
        .map(|(id, _)| {
            let m = d.monster(*id);
            (m.mini, m.gold)
        })
        .unwrap_or((0.9, 1.23))
}

pub fn rows(book: &Book) -> Vec<Row> {
    let hunter: Option<&Hunter> = book.save.as_ref().and_then(|s| s.hunters.get(book.hunter));
    let records: HashMap<&str, &game_world::save::MonsterRecord> = hunter
        .map(|h| h.monsters.iter().filter(|m| !m.key.is_empty()).map(|m| (m.key.as_str(), m)).collect())
        .unwrap_or_default();
    let mut keys: Vec<(&String, &Info)> = info().iter().collect();
    keys.sort_by_key(|(_, inf)| inf.order);
    keys.into_iter()
        .map(|(key, inf)| {
            let r = records.get(key.as_str());
            let (mini, gold) = thresholds(key);
            let largest = r.map_or(0, |r| r.largest);
            let smallest = r.map_or(0, |r| r.smallest);
            let big = largest > 0 && largest as f32 >= (gold * 100.0).round();
            let small = smallest > 0 && smallest as f32 <= (mini * 100.0).round();
            let m = book.store.world.get(key).cloned().unwrap_or_default();
            let saved_research = r.map_or(0, |r| r.research);
            let research = m.research.unwrap_or(saved_research);
            Row {
                key: key.clone(),
                species: inf.species.clone(),
                maps: map_keys(&inf.maps),
                fixed: inf.fixed,
                big: !inf.fixed && m.big.unwrap_or(big),
                small: !inf.fixed && m.small.unwrap_or(small),
                research,
                // When the save's value is above the table's cap, the save wins
                research_cap: inf.research.max(saved_research).min(9),
                nav: m.nav.unwrap_or_else(|| r.map_or(0, |r| r.nav())),
                slain: r.map_or(0, |r| r.slain),
                captured: r.map_or(0, |r| r.captured),
                largest,
                smallest,
            }
        })
        .collect()
}

fn modified(path: &std::path::Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64)
}

/// Rereads the save when the game has written it since the last read. Checks the file time at most every few seconds.
pub fn reread_if_saved(shared: &std::sync::Arc<Shared>) {
    let due = with(|b| {
        if b.reading || b.checked.is_some_and(|t| t.elapsed().as_secs() < 5) {
            return false;
        }
        b.checked = Some(std::time::Instant::now());
        let Some(path) = &b.path else { return false };
        modified(path).is_some_and(|t| b.saved_at.is_none_or(|s| t > s))
    });
    if due {
        read(shared.clone());
    }
}

/// For the overlay, which may run without the notes page ever being opened: reads the save once,
/// then again whenever the game writes it. A failed read isn't retried until the notes page asks.
pub fn keep_fresh(shared: &std::sync::Arc<Shared>) {
    let first = with(|b| b.save.is_none() && !b.reading && b.error.is_none() && b.path.is_none());
    if first {
        read(shared.clone());
    } else {
        reread_if_saved(shared);
    }
}

/// The notes rows by monster key, for the overlay.
pub fn by_key() -> HashMap<String, Row> {
    with(|b| {
        if b.save.is_none() {
            HashMap::new()
        } else {
            rows(b).into_iter().map(|r| (r.key.clone(), r)).collect()
        }
    })
}

/// Whether a monster of `size` (multiplier, e.g. 1.23) would beat the recorded largest or smallest.
/// Nothing for fixed-size monsters or ones without a record yet.
pub fn size_record(row: &Row, size: f32) -> Option<bool> {
    if row.fixed || row.largest == 0 || row.smallest == 0 {
        return None;
    }
    let pct = (size * 100.0).round() as u16;
    if pct > row.largest {
        Some(true)
    } else if pct < row.smallest {
        Some(false)
    } else {
        None
    }
}

/// Reads the save in the background, then compares how many crowns are lit before and after and sends a notification.
pub fn read(shared: std::sync::Arc<Shared>) {
    let account = shared.settings().save_account;
    let player = shared.live().snapshot.player.name.clone();
    with(|b| b.reading = true);
    std::thread::Builder::new()
        .name("save-read".into())
        .spawn(move || {
            let before: Vec<(bool, bool)> = with(|b| rows(b).iter().map(|r| (r.big, r.small)).collect());
            let Some(path) = save_path(account) else {
                with(|b| {
                    b.reading = false;
                    b.error = Some(rh_core::i18n::t("records.no_save"));
                });
                shared.repaint();
                return;
            };
            let saved_at = modified(&path);
            let result = SaveFile::read(&path, &rh_core::paths::cache());
            let ok = with(|b| {
                b.reading = false;
                b.path = Some(path.clone());
                match result {
                    Ok(Ok(save)) => {
                        // While connected to the game pick the current character, otherwise the one with the most play time
                        b.hunter = save
                            .hunters
                            .iter()
                            .position(|h| !player.is_empty() && h.name == player)
                            .or_else(|| save.hunters.iter().enumerate().max_by_key(|(_, h)| h.playtime).map(|(i, _)| i))
                            .unwrap_or(0);
                        log::info!("读取存档 {}：{} 个角色", path.display(), save.hunters.len());
                        b.save = Some(save);
                        b.saved_at = saved_at;
                        b.error = None;
                        true
                    }
                    Ok(Err(e)) => {
                        log::warn!("存档格式不对：{e}");
                        b.error = Some(e.to_string());
                        false
                    }
                    Err(e) => {
                        log::warn!("读取存档失败：{e}");
                        b.error = Some(e.to_string());
                        false
                    }
                }
            });
            if ok {
                let after: Vec<(bool, bool)> = with(|b| rows(b).iter().map(|r| (r.big, r.small)).collect());
                let gold = after.iter().zip(&before).filter(|(a, b)| a.0 && !b.0).count();
                let mini = after.iter().zip(&before).filter(|(a, b)| a.1 && !b.1).count();
                if !before.is_empty() && (gold > 0 || mini > 0) && before.iter().any(|x| x.0 || x.1) {
                    shared.notify(Notification::new(
                        Kind::SaveRead {
                            new_gold: gold,
                            new_mini: mini,
                        },
                        rh_core::i18n::t("notify.save_read"),
                        rh_core::i18n::tf("notify.save_read_body", &[("gold", &gold.to_string()), ("mini", &mini.to_string())]),
                    ));
                }
            }
            shared.repaint();
        })
        .ok();
}

/// Changes one entry manually.
pub fn set(key: &str, f: impl FnOnce(&mut Manual)) {
    with(|b| {
        let m = b.store.world.entry(key.to_string()).or_default();
        f(m);
        b.store.save();
    });
}

/// When a hunt ends: crowns not recorded yet are lit automatically and a notification is sent.
pub fn on_hunt_finished(shared: &Shared, key: &str, crown: Crown, _size: f32) {
    if !matches!(crown, Crown::Gold | Crown::Mini) || info().get(key).is_none_or(|i| i.fixed) {
        return;
    }
    let gold = crown == Crown::Gold;
    let already = with(|b| rows(b).into_iter().find(|r| r.key == key).is_some_and(|r| if gold { r.big } else { r.small }));
    if already {
        return;
    }
    set(key, |m| {
        if gold {
            m.big = Some(true);
        } else {
            m.small = Some(true);
        }
    });
    let name = rh_core::i18n::current().monster(key);
    log::info!("{name} 达到{}，已在图鉴里点亮", if gold { "大金" } else { "小金" });
    if shared.settings().notify.crown {
        shared.notify(Notification::new(
            Kind::Crown {
                monster: key.to_string(),
                gold,
            },
            rh_core::i18n::tf(if gold { "notify.crown_gold" } else { "notify.crown_mini" }, &[("monster", &name)]),
            String::new(),
        ));
    }
}

#[derive(Serialize, Deserialize)]
struct ExportRow {
    key: String,
    big: bool,
    small: bool,
    research: u8,
    nav: u8,
    slain: u16,
    captured: u16,
    largest: u16,
    smallest: u16,
}

pub fn export_json() -> String {
    let rows = with(|b| rows(b));
    let v: Vec<ExportRow> = rows
        .into_iter()
        .map(|r| ExportRow {
            key: r.key,
            big: r.big,
            small: r.small,
            research: r.research,
            nav: r.nav,
            slain: r.slain,
            captured: r.captured,
            largest: r.largest,
            smallest: r.smallest,
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "game": "world", "monsters": v })).unwrap_or_default()
}

/// Imports a backup as manual entries.
pub fn import_json(text: &str) -> Result<usize, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows: Vec<ExportRow> = serde_json::from_value(v.get("monsters").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
    let n = rows.len();
    with(|b| {
        for r in rows {
            b.store.world.insert(
                r.key,
                Manual {
                    big: Some(r.big),
                    small: Some(r.small),
                    research: Some(r.research),
                    nav: Some(r.nav),
                },
            );
        }
        b.store.save();
    });
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_covers_large_monsters() {
        assert!(info().len() >= 71);
        assert!(info()["zorah_magdaros"].fixed);
        assert_eq!(map_keys("AG"), vec!["ancient_forest", "guiding_lands"]);
        assert_eq!(info()["rathalos"].species, "TYPE_FLYING_WYVERN");
    }

    #[test]
    fn size_record_compares_with_the_notes() {
        let row = Row {
            key: "rathalos".into(),
            species: String::new(),
            maps: vec![],
            fixed: false,
            big: false,
            small: false,
            research: 0,
            research_cap: 9,
            nav: 0,
            slain: 3,
            captured: 0,
            largest: 112,
            smallest: 95,
        };
        assert_eq!(size_record(&row, 1.13), Some(true));
        assert_eq!(size_record(&row, 1.12), None, "equal isn't a new record");
        assert_eq!(size_record(&row, 0.94), Some(false));
        assert_eq!(size_record(&Row { largest: 0, ..row.clone() }, 1.3), None, "no record yet");
        assert_eq!(size_record(&Row { fixed: true, ..row }, 1.3), None);
    }

    #[test]
    fn crown_quests_load_and_group() {
        use rh_core::i18n::Lang;
        let all = crown_quests();
        assert!(all.iter().all(|q| info().contains_key(&q.monster)), "every quest monster is in the notes");
        assert!(all.iter().all(|q| q.crown == "gold" || q.crown == "mini"));

        // Two Great Jagras in the USJ quest are always tiny and one is always huge
        let lines = quest_lines(&crown_quests_for("great_jagras"), Lang::Ja);
        assert_eq!(lines[0].name, "ＵＳＪ・躍動せよ、金の星達！");
        assert!(lines[0].gold && lines[0].mini && lines[0].chance == 100);
        // Guaranteed quests hide the 12% ones for the same crown
        assert!(lines.iter().all(|l| l.chance == 100));

        // No guaranteed quest: the 12% music quests show, one line each with both crowns
        let lines = quest_lines(&crown_quests_for("barroth"), Lang::En);
        assert!(lines.iter().any(|l| l.gold && l.chance == 100));
        let lines = quest_lines(&crown_quests_for("radobaan"), Lang::En);
        assert!(!lines.is_empty() && lines.iter().all(|l| l.gold && l.mini && l.chance == 12));
    }

    #[test]
    fn research_cap_follows_table_and_save() {
        assert_eq!(info()["rathalos"].research, 9);
        assert_eq!(info()["teostra"].research, 7);
        assert_eq!(info()["velkhana"].research, 4);
        assert_eq!(info()["zorah_magdaros"].research, 4);
        assert!(info().values().all(|i| (1..=9).contains(&i.research)));

        let mut b = Book::default();
        let r = rows(&b).into_iter().find(|r| r.key == "velkhana").unwrap();
        assert_eq!(r.research_cap, 4);

        b.save = Some(SaveFile {
            steam_id: 0,
            hunters: vec![Hunter {
                monsters: vec![game_world::save::MonsterRecord {
                    key: "velkhana".into(),
                    research: 6,
                    ..Default::default()
                }],
                ..Default::default()
            }],
        });
        let r = rows(&b).into_iter().find(|r| r.key == "velkhana").unwrap();
        assert_eq!((r.research, r.research_cap), (6, 6));
    }

    #[test]
    fn rows_use_manual_overrides() {
        let mut b = Book::default();
        b.store.world.insert(
            "rathalos".into(),
            Manual {
                big: Some(true),
                research: Some(7),
                ..Default::default()
            },
        );
        let r = rows(&b).into_iter().find(|r| r.key == "rathalos").unwrap();
        assert!(r.big && !r.small);
        assert_eq!(r.research, 7);
        let z = rows(&b).into_iter().find(|r| r.key == "zorah_magdaros").unwrap();
        assert!(!z.big && z.fixed);
    }
}
