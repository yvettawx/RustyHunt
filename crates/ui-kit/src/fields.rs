//! 数据字段表：主题模板和插件能读到的字段。
//! 版本号见 `rh_core::FIELDS_VERSION`，小版本只加不删。
//!
//! 模板里的数据是一棵 JSON：根上有 `monster`、`monsters`、`player`、`party`、
//! `quest`、`game`、`base`、`overlay`，插件模块另外有 `data`。

use std::collections::HashMap;

use rh_core::i18n::I18n;
use rh_core::model::*;
use serde_json::{Map, Value, json};

/// 字段说明，用来生成 `docs/fields.md`，也用来检查模板里写的字段。
pub const FIELDS: &[(&str, &str, &str)] = &[
    ("game.key", "字符串", "world、rise、wilds"),
    ("game.stage", "字符串", "当前地点的名字"),
    ("game.in_quest", "布尔", "正在任务中"),
    ("game.in_town", "布尔", "在据点或房间里"),
    ("game.hud_open", "布尔", "打开了游戏菜单或地图"),
    ("game.world_time", "字符串", "游戏内时间，形如 06:40"),
    ("game.online", "布尔", "多人任务"),
    ("overlay.density", "字符串", "full、standard、minimal"),
    ("overlay.time", "数字", "overlay 启动后的秒数，用来做动画"),
    ("quest.id", "数字", "任务编号"),
    ("quest.stars", "数字", "星级"),
    ("quest.rank", "字符串", "low、high、master"),
    ("quest.time_left", "数字", "剩余秒数"),
    ("quest.time_max", "数字", "任务时限，秒"),
    ("quest.elapsed", "数字", "已经过去的秒数"),
    ("quest.deaths", "数字", "已经倒下的次数"),
    ("quest.deaths_max", "数字", "最多可以倒下的次数"),
    ("monster.name", "字符串", "目标怪物的名字（按术语语言）"),
    ("monster.name_en", "字符串", "英文名"),
    ("monster.key", "字符串", "术语表里的键"),
    ("monster.hp", "数字", "当前血量"),
    ("monster.hp_max", "数字", "最大血量"),
    ("monster.hp_ratio", "数字", "血量比例 0 到 1"),
    ("monster.stamina", "数字", "精力"),
    ("monster.stamina_max", "数字", "精力上限"),
    ("monster.stamina_ratio", "数字", "精力比例"),
    ("monster.enraged", "布尔", "激怒中"),
    ("monster.enrage_timer", "数字", "激怒剩余秒数"),
    ("monster.enrage_ratio", "数字", "激怒剩余比例"),
    ("monster.tired", "布尔", "疲劳中"),
    ("monster.capturable", "布尔", "可以捕获"),
    ("monster.capture_ratio", "数字", "捕获线（血量比例）"),
    ("monster.crown", "字符串", "none、mini、silver、gold"),
    ("monster.size", "数字", "体型倍率"),
    ("monster.weaknesses", "列表", "弱点属性的名字"),
    ("monster.target", "字符串", "own、other、none"),
    ("monster.captured", "布尔", "已捕获"),
    ("monster.dead", "布尔", "已讨伐"),
    ("monster.damage_recent", "数字", "最近一连串攻击的伤害"),
    ("monster.parts", "列表", "部位，每项字段见 monster.parts[].*"),
    ("monster.parts[].name", "字符串", "部位名"),
    ("monster.parts[].key", "字符串", "部位的本地化键"),
    ("monster.parts[].kind", "字符串", "flinch、breakable、severable"),
    ("monster.parts[].flinch_ratio", "数字", "硬直值比例"),
    ("monster.parts[].durability_ratio", "数字", "距下一次破坏的耐久比例"),
    ("monster.parts[].break_progress", "数字", "破坏进度，1 减耐久比例"),
    ("monster.parts[].breaks", "数字", "已经破坏的次数"),
    ("monster.parts[].breaks_max", "数字", "能破坏的次数"),
    ("monster.parts[].broken", "布尔", "已经破坏完"),
    ("monster.parts[].severable", "布尔", "可以切断"),
    ("monster.parts[].tenderized", "数字", "弱点特效剩余秒数"),
    ("monster.parts[].hot", "布尔", "最近几秒被打到"),
    ("monster.parts[].idle", "布尔", "闲置，用来淡出"),
    ("monster.ailments", "列表", "怪物异常，每项字段见 monster.ailments[].*"),
    ("monster.ailments[].name", "字符串", "异常名"),
    ("monster.ailments[].key", "字符串", "异常的本地化键"),
    ("monster.ailments[].buildup_ratio", "数字", "累积比例"),
    ("monster.ailments[].timer", "数字", "生效剩余秒数"),
    ("monster.ailments[].timer_ratio", "数字", "生效剩余比例"),
    ("monster.ailments[].count", "数字", "触发次数"),
    ("monster.ailments[].active", "布尔", "正在生效"),
    ("monster.ailments[].hot", "布尔", "最近几秒有变化"),
    ("monster.ailments[].idle", "布尔", "闲置"),
    ("monsters", "列表", "本区域所有大型怪物，每项字段和 monster 相同"),
    ("player.name", "字符串", "猎人名"),
    ("player.hr", "数字", "猎人等级"),
    ("player.mr", "数字", "大师等级"),
    ("player.hp", "数字", "生命"),
    ("player.hp_max", "数字", "最大生命"),
    ("player.hp_ratio", "数字", "生命比例"),
    ("player.hp_recoverable", "数字", "红血能回到的值"),
    ("player.hp_healing_to", "数字", "正在回复到的值"),
    ("player.stamina", "数字", "耐力"),
    ("player.stamina_max", "数字", "最大耐力"),
    ("player.stamina_ratio", "数字", "耐力比例"),
    ("player.weapon", "字符串", "武器种类的键"),
    ("player.weapon_name", "字符串", "武器种类的名字"),
    ("player.sharpness.level", "数字", "斩味颜色 0 红到 6 紫"),
    ("player.sharpness.color", "字符串", "斩味颜色名"),
    ("player.sharpness.hits_left", "数字", "当前颜色剩余击数"),
    ("player.sharpness.ratio", "数字", "当前值占最大值的比例"),
    ("player.buffs", "列表", "增益，每项 name、timer、timer_max、ratio、infinite"),
    ("player.buffs[].name", "字符串", "增益名"),
    ("player.buffs[].timer", "数字", "剩余秒数"),
    ("player.buffs[].ratio", "数字", "剩余比例"),
    ("player.buffs[].infinite", "布尔", "没有倒计时"),
    ("player.debuffs", "列表", "自身异常，字段同 player.buffs"),
    ("party", "列表", "队伍，按伤害从高到低"),
    ("party[].name", "字符串", "猎人名"),
    ("party[].damage", "数字", "总伤害"),
    ("party[].dps", "数字", "每秒伤害"),
    ("party[].share", "数字", "伤害占比"),
    ("party[].rank", "数字", "名次，从 1 开始"),
    ("party[].is_self", "布尔", "是自己"),
    ("base.argosy_days", "数字", "交易船剩余天数"),
    ("base.tailraiders_quests", "数字", "随从探险队剩余次数"),
    ("base.harvest_items", "数字", "植生研究所的收获箱里有几样东西"),
    ("base.steam_stored", "数字", "蒸汽机关管理所储存的燃料"),
];

/// 部位和异常"最近有变化"的时间，overlay 维护，生成 `hot`、`idle`。
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
    /// 部位和异常保持"热"的秒数
    pub hot_secs: f64,
    pub activity: Option<&'a Activity>,
    /// 目标怪物的 uid
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

/// 整棵数据树。
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

/// 用来检查模板字段的样例数据：每个列表至少有一项。
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

/// 检查路径在样例数据里存在。`[]` 表示列表的任意一项。
/// `data` 下面是插件自己的数据，`module` 下面是模块自己的开关，不检查。
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
                        // 空列表里元素的字段没法检查
                        None => return true,
                    }
                } else {
                    // 列表本身的属性
                    return seg == "length";
                }
            }
            _ => return false,
        };
    }
    true
}

/// 生成 `docs/fields.md`：字段表就是文档，测试保证两边一致。
pub fn markdown() -> String {
    let mut out = String::from(
        "# 数据字段

主题模板和插件拿到的数据快照用的是同一张字段表。字段表版本 ",
    );
    out += rh_core::FIELDS_VERSION;
    out += "，小版本只加不删。

`[]` 表示列表里的每一项，模板里用 `<for each=\"p in monster.parts\">` 遍历，插件拿到的是 JSON 数组。
本文件由 `cargo test -p ui-kit fields_doc` 检查，改了字段表后用 `RH_WRITE_DOCS=1` 重新生成。
";
    let mut group = "";
    for (path, kind, desc) in FIELDS {
        let g = path.split(['.', '[']).next().unwrap_or("");
        if g != group {
            group = g;
            out += &format!(
                "
## {g}

| 字段 | 类型 | 说明 |
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
        assert!(on_disk == text, "docs/fields.md 和字段表不一致，用 RH_WRITE_DOCS=1 重新生成");
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
