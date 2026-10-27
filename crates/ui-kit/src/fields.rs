//! Data field table: the fields theme templates and plugins can read.
//! The version is `rh_core::FIELDS_VERSION`; minor versions only add, never remove.
//!
//! Template data is a JSON tree: the root has `monster`, `monsters`, `player`, `party`,
//! `quest`, `game`, `base`, `overlay`, and plugin modules also get `data`.

use std::collections::HashMap;

use rh_core::i18n::I18n;
use rh_core::model::*;
use serde_json::{Map, Value, json};

/// Field descriptions, used to generate `docs/fields.md` and to check the fields used in templates.
pub const FIELDS: &[(&str, &str, &str)] = &[
    ("game.key", "string", "world, rise, wilds"),
    ("game.stage", "string", "Name of the current location"),
    ("game.in_quest", "bool", "In a quest"),
    ("game.in_town", "bool", "In a hub or room"),
    ("game.hud_open", "bool", "The game menu or map is open"),
    ("game.world_time", "string", "In-game time, like 06:40"),
    ("game.online", "bool", "Multiplayer quest"),
    ("overlay.density", "string", "full, standard, minimal"),
    ("overlay.time", "number", "Seconds since the overlay started, for animations"),
    ("quest.id", "number", "Quest ID"),
    ("quest.stars", "number", "Star rating"),
    ("quest.rank", "string", "low, high, master"),
    ("quest.time_left", "number", "Seconds left"),
    ("quest.time_max", "number", "Time limit, seconds"),
    ("quest.elapsed", "number", "Seconds elapsed"),
    ("quest.deaths", "number", "Faints so far"),
    ("quest.deaths_max", "number", "Faints allowed"),
    ("monster.name", "string", "Target monster's name (in the term language)"),
    ("monster.name_en", "string", "English name"),
    ("monster.key", "string", "Key in the term table"),
    ("monster.hp", "number", "Current HP"),
    ("monster.hp_max", "number", "Max HP"),
    ("monster.hp_ratio", "number", "HP ratio, 0 to 1"),
    ("monster.stamina", "number", "Stamina"),
    ("monster.stamina_max", "number", "Max stamina"),
    ("monster.stamina_ratio", "number", "Stamina ratio"),
    ("monster.enraged", "bool", "Enraged"),
    ("monster.enrage_timer", "number", "Seconds of rage left"),
    ("monster.enrage_ratio", "number", "Ratio of rage left"),
    ("monster.tired", "bool", "Tired"),
    ("monster.capturable", "bool", "Can be captured"),
    ("monster.capture_ratio", "number", "Capture threshold (HP ratio)"),
    ("monster.crown", "string", "none, mini, silver, gold"),
    ("monster.size", "number", "Size multiplier"),
    ("monster.weaknesses", "list", "Names of weak elements"),
    ("monster.target", "string", "own, other, none"),
    ("monster.captured", "bool", "Captured"),
    ("monster.dead", "bool", "Slain"),
    ("monster.damage_recent", "number", "Damage of the latest attack chain"),
    ("monster.parts", "list", "Parts; item fields under monster.parts[].*"),
    ("monster.parts[].name", "string", "Part name"),
    ("monster.parts[].key", "string", "Localization key of the part"),
    ("monster.parts[].kind", "string", "flinch, breakable, severable"),
    ("monster.parts[].flinch_ratio", "number", "Flinch ratio"),
    ("monster.parts[].durability_ratio", "number", "Durability ratio left until the next break"),
    ("monster.parts[].break_progress", "number", "Break progress, 1 minus the durability ratio"),
    ("monster.parts[].breaks", "number", "Times broken"),
    ("monster.parts[].breaks_max", "number", "Possible breaks"),
    ("monster.parts[].broken", "bool", "Fully broken"),
    ("monster.parts[].severable", "bool", "Severable"),
    ("monster.parts[].tenderized", "number", "Seconds of tenderize left"),
    ("monster.parts[].hot", "bool", "Hit in the last few seconds"),
    ("monster.parts[].idle", "bool", "Idle, for fading out"),
    ("monster.ailments", "list", "Monster ailments; item fields under monster.ailments[].*"),
    ("monster.ailments[].name", "string", "Ailment name"),
    ("monster.ailments[].key", "string", "Localization key of the ailment"),
    ("monster.ailments[].buildup_ratio", "number", "Buildup ratio"),
    ("monster.ailments[].timer", "number", "Seconds left while active"),
    ("monster.ailments[].timer_ratio", "number", "Ratio left while active"),
    ("monster.ailments[].count", "number", "Times triggered"),
    ("monster.ailments[].active", "bool", "Active"),
    ("monster.ailments[].hot", "bool", "Changed in the last few seconds"),
    ("monster.ailments[].idle", "bool", "Idle"),
    ("monsters", "list", "All large monsters in the area; item fields are the same as monster"),
    ("player.name", "string", "Hunter name"),
    ("player.hr", "number", "Hunter Rank"),
    ("player.mr", "number", "Master Rank"),
    ("player.hp", "number", "Health"),
    ("player.hp_max", "number", "Max health"),
    ("player.hp_ratio", "number", "Health ratio"),
    ("player.hp_recoverable", "number", "Value red health can recover to"),
    ("player.hp_healing_to", "number", "Value currently being healed to"),
    ("player.stamina", "number", "Stamina"),
    ("player.stamina_max", "number", "Max stamina"),
    ("player.stamina_ratio", "number", "Stamina ratio"),
    ("player.weapon", "string", "Weapon type key"),
    ("player.weapon_name", "string", "Weapon type name"),
    ("player.sharpness.level", "number", "Sharpness color, 0 red to 6 purple"),
    ("player.sharpness.color", "string", "Sharpness color name"),
    ("player.sharpness.hits_left", "number", "Hits left in the current color"),
    ("player.sharpness.ratio", "number", "Current value as a ratio of the max"),
    ("player.buffs", "list", "Buffs; each has name, timer, timer_max, ratio, infinite"),
    ("player.buffs[].name", "string", "Buff name"),
    ("player.buffs[].timer", "number", "Seconds left"),
    ("player.buffs[].ratio", "number", "Ratio left"),
    ("player.buffs[].infinite", "bool", "No countdown"),
    ("player.debuffs", "list", "Status effects on the player; same fields as player.buffs"),
    ("party", "list", "Party, by damage from high to low"),
    ("party[].name", "string", "Hunter name"),
    ("party[].damage", "number", "Total damage"),
    ("party[].dps", "number", "Damage per second"),
    ("party[].share", "number", "Share of damage"),
    ("party[].rank", "number", "Rank, starting at 1"),
    ("party[].is_self", "bool", "Is the local player"),
    ("base.argosy_days", "number", "Days left for the Argosy"),
    ("base.tailraiders_quests", "number", "Tailraider expeditions left"),
    ("base.harvest_items", "number", "Items in the Botanical Research harvest box"),
    ("base.steam_stored", "number", "Fuel stored at the Steamworks"),
];

/// When parts and ailments last changed, kept by the overlay to produce `hot` and `idle`.
#[derive(Default, Clone)]
pub struct Activity {
    pub last_change: HashMap<String, f64>,
}

impl Activity {
    pub fn touch(&mut self, key: String, now: f64) {
        self.last_change.insert(key, now);
    }

    pub fn age(&self, key: &str, now: f64) -> f64 {
        self.last_change.get(key).map_or(f64::INFINITY, |t| now - t)
    }
}

pub struct ContextOptions<'a> {
    pub i18n: &'a I18n,
    pub density: &'a str,
    pub time: f64,
    /// Seconds a part or ailment stays "hot"
    pub hot_secs: f64,
    pub activity: Option<&'a Activity>,
    /// uid of the target monster
    pub target: Option<u64>,
    pub low_hp: f32,
    pub damage_recent: f32,
}

fn round2(x: f32) -> f64 {
    ((x as f64) * 10000.0).round() / 10000.0
}

pub fn monster_value(m: &Monster, o: &ContextOptions) -> Value {
    let act = o.activity;
    let age = |k: String| act.map_or(f64::INFINITY, |a| a.age(&k, o.time));
    let parts: Vec<Value> = m
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let a = age(format!("{}:p{i}", m.uid));
            json!({
                "name": o.i18n.game(&p.key),
                "key": p.key,
                "kind": match p.kind { PartKind::Flinch => "flinch", PartKind::Breakable => "breakable", PartKind::Severable => "severable" },
                "flinch_ratio": round2(p.flinch_ratio()),
                "durability_ratio": round2(p.durability_ratio()),
                "break_progress": round2(if p.breaks_max > 0 { 1.0 - p.durability_ratio() } else { 0.0 }),
                "breaks": p.breaks,
                "breaks_max": p.breaks_max,
                "broken": p.broken(),
                "severable": p.kind == PartKind::Severable,
                "tenderized": round2(p.tenderized),
                "hot": a < o.hot_secs,
                "idle": a >= o.hot_secs,
            })
        })
        .collect();
    let ailments: Vec<Value> = m
        .ailments
        .iter()
        .map(|a| {
            let ag = age(format!("{}:a{}", m.uid, a.key));
            json!({
                "name": o.i18n.game(&a.key),
                "key": a.key,
                "buildup_ratio": round2(a.buildup_ratio()),
                "timer": round2(a.timer),
                "timer_ratio": round2(a.timer_ratio()),
                "count": a.count,
                "active": a.active(),
                "hot": ag < o.hot_secs || a.active(),
                "idle": ag >= o.hot_secs && !a.active(),
            })
        })
        .collect();
    json!({
        "name": o.i18n.monster(&m.key),
        "name_en": o.i18n.monster_in(&m.key, rh_core::i18n::Lang::En),
        "key": m.key,
        "hp": m.hp.round(),
        "hp_max": m.hp_max.round(),
        "hp_ratio": round2(m.hp_ratio()),
        "stamina": m.stamina.round(),
        "stamina_max": m.stamina_max.round(),
        "stamina_ratio": round2(m.stamina_ratio()),
        "enraged": m.enraged,
        "enrage_timer": round2(m.enrage_timer),
        "enrage_ratio": round2(ratio(m.enrage_timer, m.enrage_timer_max)),
        "tired": m.tired,
        "capturable": m.capturable(),
        "capture_ratio": round2(m.capture_threshold),
        "crown": match m.crown { Crown::None => "none", Crown::Mini => "mini", Crown::Silver => "silver", Crown::Gold => "gold" },
        "size": round2(m.size),
        "weaknesses": m.weaknesses.iter().map(|e| o.i18n.t(&format!("element.{}", e.key()))).collect::<Vec<_>>(),
        "target": match m.target { Target::Own => "own", Target::Other => "other", Target::None => "none" },
        "captured": m.captured,
        "dead": m.hp <= 0.0 && !m.captured,
        "damage_recent": if Some(m.uid) == o.target { o.damage_recent.round() } else { 0.0 },
        "parts": parts,
        "ailments": ailments,
    })
}

fn buff_value(a: &Abnormality, o: &ContextOptions) -> Value {
    json!({
        "name": o.i18n.game(&a.key),
        "id": a.id,
        "timer": round2(a.timer),
        "timer_max": round2(a.timer_max),
        "ratio": round2(if a.infinite { 1.0 } else { ratio(a.timer, a.timer_max) }),
        "infinite": a.infinite,
    })
}

pub fn party_values(s: &Snapshot) -> Vec<Value> {
    let elapsed = s.elapsed.max(1.0);
    let total: f32 = s.party.iter().map(|p| p.damage).sum::<f32>().max(1.0);
    let mut members: Vec<&PartyMember> = s.party.iter().collect();
    members.sort_by(|a, b| b.damage.total_cmp(&a.damage));
    members
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({
                "name": p.name,
                "damage": p.damage.round(),
                "dps": (p.damage / elapsed).round(),
                "share": round2(p.damage / total),
                "rank": i + 1,
                "is_self": p.is_self,
                "weapon": p.weapon.key(),
            })
        })
        .collect()
}

/// The whole data tree.
pub fn context(s: &Snapshot, o: &ContextOptions) -> Value {
    let target = o.target.and_then(|uid| s.monster(uid)).or_else(|| s.large_monsters().next());
    let p = &s.player;
    let sharp = p
        .sharpness
        .as_ref()
        .map_or(json!({"level": 0, "color": "", "hits_left": 0, "ratio": 0.0}), |sh| {
            json!({
                "level": sh.level.index(),
                "color": o.i18n.t(&format!("sharpness.{}", sh.level.index())),
                "hits_left": sh.hits_left(),
                "ratio": round2(ratio(sh.current as f32, sh.max as f32)),
            })
        });
    let mut root = Map::new();
    root.insert(
        "game".into(),
        json!({
            "key": s.game.key(),
            "stage": if s.stage_key.is_empty() { o.i18n.game(&s.stage_id.to_string()) } else { o.i18n.map(&s.stage_key) },
            "in_quest": s.in_quest(),
            "in_town": s.in_town,
            "hud_open": s.hud_open,
            "world_time": s.world_time.map(|(h, m)| format!("{h:02}:{m:02}")).unwrap_or_default(),
            "online": s.online,
        }),
    );
    root.insert("overlay".into(), json!({"density": o.density, "time": round2(o.time as f32)}));
    root.insert(
        "quest".into(),
        match &s.quest {
            Some(q) => json!({
                "id": q.id, "stars": q.stars,
                "rank": match q.rank { Rank::Low => "low", Rank::High => "high", Rank::Master => "master" },
                "time_left": round2(q.time_left), "time_max": round2(q.time_max), "elapsed": round2(s.elapsed),
                "deaths": q.deaths, "deaths_max": q.deaths_max,
            }),
            None => json!({"id": 0, "stars": 0, "rank": "low", "time_left": 0, "time_max": 0, "elapsed": round2(s.elapsed), "deaths": 0, "deaths_max": 0}),
        },
    );
    root.insert(
        "monster".into(),
        target.map_or_else(|| monster_value(&Monster::default(), o), |m| monster_value(m, o)),
    );
    root.insert("monsters".into(), Value::Array(s.large_monsters().map(|m| monster_value(m, o)).collect()));
    root.insert(
        "player".into(),
        json!({
            "name": p.name, "hr": p.hr, "mr": p.mr,
            "hp": p.hp.round(), "hp_max": p.hp_max.round(), "hp_ratio": round2(ratio(p.hp, p.hp_max)),
            "hp_recoverable": p.hp_recoverable.round(), "hp_healing_to": p.hp_healing_to.round(),
            "low": p.hp_max > 0.0 && p.hp / p.hp_max < o.low_hp,
            "stamina": p.stamina.round(), "stamina_max": p.stamina_max.round(), "stamina_ratio": round2(ratio(p.stamina, p.stamina_max)),
            "weapon": p.weapon.key(),
            "weapon_name": o.i18n.t(&format!("weapon.{}", p.weapon.key())),
            "sharpness": sharp,
            "buffs": p.abnormalities.iter().filter(|a| a.kind != AbnormalityKind::Debuff).map(|a| buff_value(a, o)).collect::<Vec<_>>(),
            "debuffs": p.debuffs().map(|a| buff_value(a, o)).collect::<Vec<_>>(),
        }),
    );
    root.insert("party".into(), Value::Array(party_values(s)));
    root.insert(
        "base".into(),
        s.base
            .as_ref()
            .map_or(json!({"argosy_days": 0, "tailraiders_quests": 0, "harvest_items": 0, "steam_stored": 0}), |b| {
                json!({
                    "argosy_days": b.argosy_days, "tailraiders_quests": b.tailraiders_quests,
                    "harvest_items": b.harvest_items, "steam_stored": b.steam_stored,
                })
            }),
    );
    Value::Object(root)
}

/// Sample data for checking template fields: every list has at least one item.
pub fn sample() -> Value {
    let i18n = I18n::default();
    let mut s = Snapshot::default();
    s.monsters.push(Monster {
        large: true,
        parts: vec![Part::default()],
        ailments: vec![Ailment::default()],
        ..Default::default()
    });
    s.player.abnormalities = vec![
        Abnormality::default(),
        Abnormality {
            kind: AbnormalityKind::Debuff,
            ..Default::default()
        },
    ];
    s.party.push(PartyMember::default());
    s.base = Some(Base::default());
    let o = ContextOptions {
        i18n: &i18n,
        density: "standard",
        time: 0.0,
        hot_secs: 4.0,
        activity: None,
        target: None,
        low_hp: 0.3,
        damage_recent: 0.0,
    };
    let mut v = context(&s, &o);
    v.as_object_mut().unwrap().insert("data".into(), json!({}));
    v
}

/// Checks that a path exists in the sample data. `[]` means any item of a list.
/// Under `data` is the plugin's own data and under `module` the module's own switches; neither is checked.
pub fn path_exists(sample: &Value, path: &[String]) -> bool {
    if matches!(path.first().map(String::as_str), Some("data" | "module")) {
        return true;
    }
    let mut cur = sample;
    for seg in path {
        cur = match cur {
            Value::Object(m) => match m.get(seg) {
                Some(v) => v,
                None => return false,
            },
            Value::Array(a) => {
                if seg == "[]" || seg.parse::<usize>().is_ok() {
                    match a.first() {
                        Some(v) => v,
                        // Fields of items in an empty list can't be checked
                        None => return true,
                    }
                } else {
                    // Properties of the list itself
                    return seg == "length";
                }
            }
            _ => return false,
        };
    }
    true
}

/// Generates `docs/fields.md`: the field table is the documentation, and a test keeps the two in sync.
pub fn markdown() -> String {
    let mut out = String::from(
        "# Data fields

Theme templates and plugins get data snapshots built from the same field table. Field table version ",
    );
    out += rh_core::FIELDS_VERSION;
    out += "; minor versions only add fields, never remove them.

`[]` means every item of a list. Templates iterate with `<for each=\"p in monster.parts\">`; plugins get a JSON array.
This file is checked by `cargo test -p ui-kit fields_doc`; after changing the field table, regenerate it with `RH_WRITE_DOCS=1`.
";
    let mut group = "";
    for (path, kind, desc) in FIELDS {
        let g = path.split(['.', '[']).next().unwrap_or("");
        if g != group {
            group = g;
            out += &format!(
                "
## {g}

| Field | Type | Description |
|---|---|---|
"
            );
        }
        out += &format!(
            "| `{path}` | {kind} | {desc} |
"
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_doc_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fields.md");
        let text = markdown();
        if std::env::var_os("RH_WRITE_DOCS").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert!(
            on_disk == text,
            "docs/fields.md is out of sync with the field table; regenerate it with RH_WRITE_DOCS=1"
        );
    }

    #[test]
    fn documented_fields_exist() {
        let s = sample();
        for (path, _, _) in FIELDS {
            let segs: Vec<String> = path.replace("[]", ".[]").split('.').map(str::to_string).collect();
            assert!(path_exists(&s, &segs), "{path}");
        }
    }

    #[test]
    fn unknown_field_is_rejected() {
        let s = sample();
        assert!(!path_exists(&s, &["monster".into(), "hp_ratoi".into()]));
        assert!(path_exists(&s, &["monster".into(), "parts".into(), "[]".into(), "name".into()]));
        assert!(path_exists(&s, &["data".into(), "anything".into()]));
    }
}
