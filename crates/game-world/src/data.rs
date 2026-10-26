//! 世界的静态数据：怪物（部位、弱点、体型、捕获线）、怪物异常、增益。
//! 来源是上游的 `MonsterData.xml` 和 `AbnormalityData.xml`。

use std::collections::HashMap;
use std::sync::OnceLock;

use rh_core::model::{AbnormalityKind, Element};

const MONSTERS_XML: &str = include_str!("../../../data/world/monsters.xml");
const ABNORMALITIES_XML: &str = include_str!("../../../data/world/abnormalities.xml");
const MONSTER_KEYS: &str = include_str!("../../../data/world/monster_keys.toml");

#[derive(Debug, Clone, PartialEq)]
pub struct PartDef {
    pub id: u32,
    pub key: String,
    pub severable: bool,
    /// 每次破坏需要的累计硬直次数
    pub thresholds: Vec<i32>,
    pub tenderize_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonsterDef {
    pub id: i32,
    /// 捕获线，百分比
    pub capture: f32,
    pub mini: f32,
    pub silver: f32,
    pub gold: f32,
    pub weaknesses: Vec<Element>,
    pub parts: Vec<PartDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbnormalityDef {
    pub id: String,
    pub key: String,
    pub kind: AbnormalityKind,
    /// 从哪个基址读：Gears 分类走装备地址，其他走增益地址
    pub gear: bool,
    /// 从食事地址读
    pub food: bool,
    pub offset: i32,
    pub depends_on: i32,
    pub with_value: i32,
    pub infinite: bool,
}

pub struct Data {
    pub monsters: HashMap<i32, MonsterDef>,
    pub ailments: HashMap<i32, String>,
    pub abnormalities: Vec<AbnormalityDef>,
    pub monster_keys: HashMap<i32, String>,
    unknown: MonsterDef,
}

impl Data {
    pub fn monster(&self, id: i32) -> &MonsterDef {
        self.monsters.get(&id).unwrap_or(&self.unknown)
    }

    pub fn monster_key(&self, id: i32) -> String {
        self.monster_keys.get(&id).cloned().unwrap_or_else(|| format!("em{id:03}"))
    }

    /// 已知的怪物异常；字符串是纯数字的条目是未知异常，跳过
    pub fn ailment(&self, id: i32) -> Option<&str> {
        self.ailments.get(&id).map(String::as_str).filter(|s| !s.chars().all(|c| c.is_ascii_digit()))
    }
}

pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| parse().expect("内置的世界数据文件格式正确"))
}

fn element(name: &str) -> Option<Element> {
    Some(match name {
        "Fire" => Element::Fire,
        "Water" => Element::Water,
        "Ice" => Element::Ice,
        "Thunder" => Element::Thunder,
        "Dragon" => Element::Dragon,
        _ => return None,
    })
}

fn hex(s: &str) -> i32 {
    i32::from_str_radix(s, 16).unwrap_or(0)
}

fn parse() -> Result<Data, String> {
    let doc = roxmltree::Document::parse(MONSTERS_XML.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let mut ailments = HashMap::new();
    let mut monsters = HashMap::new();
    for node in doc.descendants() {
        match node.tag_name().name() {
            "Ailment" if node.parent().is_some_and(|p| p.has_tag_name("Ailments")) => {
                let id = node.attribute("Id").and_then(|v| v.parse().ok()).unwrap_or(-1);
                ailments.insert(id, node.attribute("String").unwrap_or_default().to_string());
            }
            "Monster" => {
                let id: i32 = node.attribute("Id").and_then(|v| v.parse().ok()).unwrap_or(-1);
                let mut def = MonsterDef {
                    id,
                    capture: node.attribute("Capture").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                    mini: 0.9,
                    silver: 1.15,
                    gold: 1.23,
                    weaknesses: Vec::new(),
                    parts: Vec::new(),
                };
                for child in node.descendants() {
                    match child.tag_name().name() {
                        "Crowns" => {
                            let f = |a: &str, d: f32| child.attribute(a).and_then(|v| v.parse().ok()).unwrap_or(d);
                            def.mini = f("Mini", 0.9);
                            def.silver = f("Silver", 1.15);
                            def.gold = f("Gold", 1.23);
                        }
                        "Weakness" => {
                            if let Some(e) = child.attribute("Name").and_then(element) {
                                def.weaknesses.push(e);
                            }
                        }
                        "Part" => {
                            def.parts.push(PartDef {
                                id: child.attribute("Id").and_then(|v| v.parse().ok()).unwrap_or(0),
                                key: child.attribute("String").unwrap_or("PART_UNKNOWN").to_string(),
                                severable: child.attribute("IsSeverable").is_some_and(|v| v.eq_ignore_ascii_case("true")),
                                thresholds: child
                                    .children()
                                    .filter(|c| c.has_tag_name("Break"))
                                    .filter_map(|c| c.attribute("Threshold")?.parse().ok())
                                    .collect(),
                                tenderize_ids: child
                                    .attribute("TenderizeIds")
                                    .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                                    .unwrap_or_default(),
                            });
                        }
                        _ => {}
                    }
                }
                monsters.insert(id, def);
            }
            _ => {}
        }
    }

    let doc = roxmltree::Document::parse(ABNORMALITIES_XML.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let mut abnormalities = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Abnormality")) {
        let group = node.parent().map(|p| p.tag_name().name()).unwrap_or_default();
        let category = node.attribute("Category").unwrap_or(group);
        let raw_id = node.attribute("Id").unwrap_or_default();
        let id = if raw_id.starts_with("ABN_") {
            raw_id.to_string()
        } else {
            format!("{group}_{raw_id}")
        };
        let kind = match category {
            "Songs" => AbnormalityKind::Song,
            "Palico" => AbnormalityKind::Palico,
            "Debuffs" => AbnormalityKind::Debuff,
            "Skills" => AbnormalityKind::Skill,
            "Gears" => AbnormalityKind::Gear,
            "Foods" => AbnormalityKind::Food,
            _ => AbnormalityKind::Consumable,
        };
        abnormalities.push(AbnormalityDef {
            id,
            key: node.attribute("Name").unwrap_or("ABNORMALITY_UNKNOWN").to_string(),
            kind,
            gear: category == "Gears",
            food: group == "Foods",
            offset: hex(node.attribute("Offset").unwrap_or(raw_id)),
            depends_on: hex(node.attribute("DependsOn").unwrap_or("0")),
            with_value: node.attribute("WithValue").and_then(|v| v.parse().ok()).unwrap_or(0),
            infinite: node.attribute("IsInfinite").is_some_and(|v| v.eq_ignore_ascii_case("true")),
        });
    }

    let keys: toml::Table = toml::from_str(MONSTER_KEYS).map_err(|e| e.to_string())?;
    let monster_keys = keys.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect();

    Ok(Data {
        monsters,
        ailments,
        abnormalities,
        monster_keys,
        unknown: MonsterDef {
            id: -1,
            capture: 0.0,
            mini: 0.9,
            silver: 1.15,
            gold: 1.23,
            weaknesses: Vec::new(),
            parts: Vec::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundled_data() {
        let d = data();
        assert!(d.monsters.len() > 70);
        let anja = d.monster(0);
        assert_eq!(anja.capture, 25.0);
        assert_eq!(anja.gold, 1.20);
        assert_eq!(anja.weaknesses, vec![Element::Water, Element::Ice]);
        let head = anja.parts.iter().find(|p| p.key == "PART_HEAD").unwrap();
        assert_eq!(head.thresholds, vec![5]);
        assert_eq!(head.tenderize_ids, vec![0, 4]);
        assert!(anja.parts.iter().any(|p| p.severable));
        assert_eq!(d.monster_key(1), "rathalos");
        assert_eq!(d.ailment(1), Some("AILMENT_POISON"));
        assert_eq!(d.ailment(13), None);
    }

    #[test]
    fn parses_abnormalities() {
        let d = data();
        let demon = d.abnormalities.iter().find(|a| a.key == "ABNORMALITY_DEMONDRUG").unwrap();
        assert_eq!(demon.offset, 0x6CC);
        assert_eq!(demon.depends_on, 0x6D4);
        assert_eq!(demon.with_value, 1);
        assert!(demon.infinite);
        let food = d.abnormalities.iter().find(|a| a.kind == AbnormalityKind::Food).unwrap();
        assert!(food.food);
        assert_eq!(food.offset, 0xE4);
        let mantle = d.abnormalities.iter().find(|a| a.key == "ABNORMALITY_EVASION_MANTLE").unwrap();
        assert!(mantle.gear);
        let song = d.abnormalities.iter().find(|a| a.id == "Songs_38").unwrap();
        assert_eq!(song.offset, 0x38);
        assert_eq!(song.kind, AbnormalityKind::Song);
    }
}
