//! 图鉴：从 Steam 存档读猎人笔记（只读），读不到时可以手动登记；
//! 狩猎结束时体型达到金冠，自动在图鉴里点亮并发通知。

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
    pub species: String,
    #[serde(default)]
    pub maps: String,
    #[serde(default)]
    pub fixed: bool,
}

pub fn info() -> &'static BTreeMap<String, Info> {
    static I: OnceLock<BTreeMap<String, Info>> = OnceLock::new();
    I.get_or_init(|| toml::from_str(DATA).expect("内置图鉴数据格式正确"))
}

/// 地图代号对应术语表里的地图。
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

/// 手动登记的值，盖过存档里的值。
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
    pub read_at: Option<i64>,
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

/// 存档路径：设置里选的 Steam 账号；没选时用第一个有世界存档的账号。
pub fn save_path(account: Option<u32>) -> Option<PathBuf> {
    let steam = platform::steam::steam_dir()?;
    let app = Game::World.steam_app_id();
    let file = |id: u32| platform::steam::remote_dir(&steam, id, app).join("SAVEDATA1000");
    if let Some(id) = account {
        return Some(file(id));
    }
    platform::steam::accounts(&steam).into_iter().map(|a| file(a.account_id)).find(|p| p.is_file())
}

/// 游戏内部编号到皇冠阈值：按术语表的键反查。
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
    info()
        .iter()
        .map(|(key, inf)| {
            let r = records.get(key.as_str());
            let (mini, gold) = thresholds(key);
            let largest = r.map_or(0, |r| r.largest);
            let smallest = r.map_or(0, |r| r.smallest);
            let big = largest > 0 && largest as f32 >= (gold * 100.0).round();
            let small = smallest > 0 && smallest as f32 <= (mini * 100.0).round();
            let m = book.store.world.get(key).cloned().unwrap_or_default();
            Row {
                key: key.clone(),
                species: inf.species.clone(),
                maps: map_keys(&inf.maps),
                fixed: inf.fixed,
                big: !inf.fixed && m.big.unwrap_or(big),
                small: !inf.fixed && m.small.unwrap_or(small),
                research: m.research.unwrap_or(r.map_or(0, |r| r.research)),
                research_cap: 9,
                nav: m.nav.unwrap_or(0),
                slain: r.map_or(0, |r| r.slain),
                captured: r.map_or(0, |r| r.captured),
                largest,
                smallest,
            }
        })
        .collect()
}

/// 后台读存档。读完后比较前后点亮的金冠数量，发通知。
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
            let result = SaveFile::read(&path, &rh_core::paths::cache());
            let ok = with(|b| {
                b.reading = false;
                b.path = Some(path.clone());
                match result {
                    Ok(Ok(save)) => {
                        // 连着游戏时选当前角色，否则选游戏时间最长的
                        b.hunter = save
                            .hunters
                            .iter()
                            .position(|h| !player.is_empty() && h.name == player)
                            .or_else(|| save.hunters.iter().enumerate().max_by_key(|(_, h)| h.playtime).map(|(i, _)| i))
                            .unwrap_or(0);
                        log::info!("读取存档 {}：{} 个角色", path.display(), save.hunters.len());
                        b.save = Some(save);
                        b.read_at = Some(chrono::Local::now().timestamp());
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

/// 手动改一项。
pub fn set(key: &str, f: impl FnOnce(&mut Manual)) {
    with(|b| {
        let m = b.store.world.entry(key.to_string()).or_default();
        f(m);
        b.store.save();
    });
}

/// 狩猎结束时：金冠或小金冠还没登记的，自动点亮并发通知。
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

/// 导入备份：作为手动登记写入。
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
