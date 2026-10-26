# 数据字段

主题模板和插件拿到的数据快照用的是同一张字段表。字段表版本 1.0，小版本只加不删。

`[]` 表示列表里的每一项，模板里用 `<for each="p in monster.parts">` 遍历，插件拿到的是 JSON 数组。
本文件由 `cargo test -p ui-kit fields_doc` 检查，改了字段表后用 `RH_WRITE_DOCS=1` 重新生成。

## game

| 字段 | 类型 | 说明 |
|---|---|---|
| `game.key` | 字符串 | world、rise、wilds |
| `game.stage` | 字符串 | 当前地点的名字 |
| `game.in_quest` | 布尔 | 正在任务中 |
| `game.in_town` | 布尔 | 在据点或房间里 |
| `game.hud_open` | 布尔 | 打开了游戏菜单或地图 |
| `game.world_time` | 字符串 | 游戏内时间，形如 06:40 |
| `game.online` | 布尔 | 多人任务 |

## overlay

| 字段 | 类型 | 说明 |
|---|---|---|
| `overlay.density` | 字符串 | full、standard、minimal |
| `overlay.time` | 数字 | overlay 启动后的秒数，用来做动画 |

## quest

| 字段 | 类型 | 说明 |
|---|---|---|
| `quest.id` | 数字 | 任务编号 |
| `quest.stars` | 数字 | 星级 |
| `quest.rank` | 字符串 | low、high、master |
| `quest.time_left` | 数字 | 剩余秒数 |
| `quest.time_max` | 数字 | 任务时限，秒 |
| `quest.elapsed` | 数字 | 已经过去的秒数 |
| `quest.deaths` | 数字 | 已经倒下的次数 |
| `quest.deaths_max` | 数字 | 最多可以倒下的次数 |

## monster

| 字段 | 类型 | 说明 |
|---|---|---|
| `monster.name` | 字符串 | 目标怪物的名字（按术语语言） |
| `monster.name_en` | 字符串 | 英文名 |
| `monster.key` | 字符串 | 术语表里的键 |
| `monster.hp` | 数字 | 当前血量 |
| `monster.hp_max` | 数字 | 最大血量 |
| `monster.hp_ratio` | 数字 | 血量比例 0 到 1 |
| `monster.stamina` | 数字 | 精力 |
| `monster.stamina_max` | 数字 | 精力上限 |
| `monster.stamina_ratio` | 数字 | 精力比例 |
| `monster.enraged` | 布尔 | 激怒中 |
| `monster.enrage_timer` | 数字 | 激怒剩余秒数 |
| `monster.enrage_ratio` | 数字 | 激怒剩余比例 |
| `monster.tired` | 布尔 | 疲劳中 |
| `monster.capturable` | 布尔 | 可以捕获 |
| `monster.capture_ratio` | 数字 | 捕获线（血量比例） |
| `monster.crown` | 字符串 | none、mini、silver、gold |
| `monster.size` | 数字 | 体型倍率 |
| `monster.weaknesses` | 列表 | 弱点属性的名字 |
| `monster.target` | 字符串 | own、other、none |
| `monster.captured` | 布尔 | 已捕获 |
| `monster.dead` | 布尔 | 已讨伐 |
| `monster.damage_recent` | 数字 | 最近一连串攻击的伤害 |
| `monster.parts` | 列表 | 部位，每项字段见 monster.parts[].* |
| `monster.parts[].name` | 字符串 | 部位名 |
| `monster.parts[].key` | 字符串 | 部位的本地化键 |
| `monster.parts[].kind` | 字符串 | flinch、breakable、severable |
| `monster.parts[].flinch_ratio` | 数字 | 硬直值比例 |
| `monster.parts[].durability_ratio` | 数字 | 距下一次破坏的耐久比例 |
| `monster.parts[].break_progress` | 数字 | 破坏进度，1 减耐久比例 |
| `monster.parts[].breaks` | 数字 | 已经破坏的次数 |
| `monster.parts[].breaks_max` | 数字 | 能破坏的次数 |
| `monster.parts[].broken` | 布尔 | 已经破坏完 |
| `monster.parts[].severable` | 布尔 | 可以切断 |
| `monster.parts[].tenderized` | 数字 | 弱点特效剩余秒数 |
| `monster.parts[].hot` | 布尔 | 最近几秒被打到 |
| `monster.parts[].idle` | 布尔 | 闲置，用来淡出 |
| `monster.ailments` | 列表 | 怪物异常，每项字段见 monster.ailments[].* |
| `monster.ailments[].name` | 字符串 | 异常名 |
| `monster.ailments[].key` | 字符串 | 异常的本地化键 |
| `monster.ailments[].buildup_ratio` | 数字 | 累积比例 |
| `monster.ailments[].timer` | 数字 | 生效剩余秒数 |
| `monster.ailments[].timer_ratio` | 数字 | 生效剩余比例 |
| `monster.ailments[].count` | 数字 | 触发次数 |
| `monster.ailments[].active` | 布尔 | 正在生效 |
| `monster.ailments[].hot` | 布尔 | 最近几秒有变化 |
| `monster.ailments[].idle` | 布尔 | 闲置 |

## monsters

| 字段 | 类型 | 说明 |
|---|---|---|
| `monsters` | 列表 | 本区域所有大型怪物，每项字段和 monster 相同 |

## player

| 字段 | 类型 | 说明 |
|---|---|---|
| `player.name` | 字符串 | 猎人名 |
| `player.hr` | 数字 | 猎人等级 |
| `player.mr` | 数字 | 大师等级 |
| `player.hp` | 数字 | 生命 |
| `player.hp_max` | 数字 | 最大生命 |
| `player.hp_ratio` | 数字 | 生命比例 |
| `player.hp_recoverable` | 数字 | 红血能回到的值 |
| `player.hp_healing_to` | 数字 | 正在回复到的值 |
| `player.stamina` | 数字 | 耐力 |
| `player.stamina_max` | 数字 | 最大耐力 |
| `player.stamina_ratio` | 数字 | 耐力比例 |
| `player.weapon` | 字符串 | 武器种类的键 |
| `player.weapon_name` | 字符串 | 武器种类的名字 |
| `player.sharpness.level` | 数字 | 斩味颜色 0 红到 6 紫 |
| `player.sharpness.color` | 字符串 | 斩味颜色名 |
| `player.sharpness.hits_left` | 数字 | 当前颜色剩余击数 |
| `player.sharpness.ratio` | 数字 | 当前值占最大值的比例 |
| `player.buffs` | 列表 | 增益，每项 name、timer、timer_max、ratio、infinite |
| `player.buffs[].name` | 字符串 | 增益名 |
| `player.buffs[].timer` | 数字 | 剩余秒数 |
| `player.buffs[].ratio` | 数字 | 剩余比例 |
| `player.buffs[].infinite` | 布尔 | 没有倒计时 |
| `player.debuffs` | 列表 | 自身异常，字段同 player.buffs |

## party

| 字段 | 类型 | 说明 |
|---|---|---|
| `party` | 列表 | 队伍，按伤害从高到低 |
| `party[].name` | 字符串 | 猎人名 |
| `party[].damage` | 数字 | 总伤害 |
| `party[].dps` | 数字 | 每秒伤害 |
| `party[].share` | 数字 | 伤害占比 |
| `party[].rank` | 数字 | 名次，从 1 开始 |
| `party[].is_self` | 布尔 | 是自己 |

## base

| 字段 | 类型 | 说明 |
|---|---|---|
| `base.argosy_days` | 数字 | 交易船剩余天数 |
| `base.tailraiders_quests` | 数字 | 随从探险队剩余次数 |
| `base.harvest_items` | 数字 | 植生研究所的收获箱里有几样东西 |
| `base.steam_stored` | 数字 | 蒸汽机关管理所储存的燃料 |
