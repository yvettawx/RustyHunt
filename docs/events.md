# 事件

读取线程每读一帧，就和上一帧比较，得出这一帧发生的事件。overlay 的播报、狩猎记录的时间轴和插件都用这一份事件。

事件列表版本 1.0：小版本只加不删，要删的事件提前一个大版本标记废弃。插件在 `plugin.toml` 的 `events` 里写要订阅的事件名，写了不存在的名字会装不上。

插件收到的每个事件是一个 JSON 对象：`type` 是事件名，其余是下表的字段，另外都带 `elapsed`（事件发生时的任务时间，秒）。

```json
{"type": "part_break", "uid": 2216434944, "part": "PART_HEAD", "count": 1, "elapsed": 412.5}
```

## 列表

| 事件 | 字段 | 什么时候 |
|---|---|---|
| `quest_start` | `quest_id`、`stars` | 任务开始；换任务时先发上一个的 `quest_end` |
| `quest_end` | `quest_id`、`status`、`elapsed` | 任务结束。`status` 是 `success`、`fail`、`quit` |
| `monster_spawn` | `uid`、`key` | 一只大型怪物出现在读取范围里 |
| `monster_despawn` | `uid`、`key` | 大型怪物离开读取范围（换区域、离开任务） |
| `monster_death` | `uid`、`key` | 讨伐 |
| `monster_capture` | `uid`、`key` | 捕获 |
| `part_break` | `uid`、`part`、`count` | 部位破坏，`count` 是这个部位已经破坏的次数 |
| `part_sever` | `uid`、`part` | 尾巴等部位被切断 |
| `ailment_trigger` | `uid`、`ailment`、`count` | 异常发作（麻痹、睡眠、眩晕等），`count` 是发作次数 |
| `enrage` | `uid` | 进入激怒 |
| `enrage_end` | `uid` | 激怒结束 |
| `tired` | `uid` | 进入疲劳 |
| `capturable` | `uid` | 血量降到捕获线以下 |
| `player_down` | `deaths`、`deaths_max` | 力尽 |
| `stage_change` | `stage_id` | 换了地点 |
| `weapon_change` | `weapon` | 换了武器，值是武器编号（`longsword` 等） |
| `abnormality_start` | `id`、`key` | 一个增益或异常开始 |
| `abnormality_end` | `id`、`key` | 一个增益或异常结束 |

## 字段说明

- `uid`：这只怪物在本次连接中的编号，同一只怪物不变，可以用来对上 `monsters[]` 里的那一项。
- `key`：怪物的术语键，例如 `raging_brachydios`，和 `locales/terms/world.toml` 里的键一致。
- `part`、`ailment`：部位、异常的文案键，例如 `PART_HEAD`、`AILMENT_PARALYSIS`，和 `locales/game/*.toml` 里的键一致。
- `id`、`key`（增益）：`id` 是增益在数据表里的编号，`key` 是文案键，例如 `ABNORMALITY_DEMONDRUG`。

## 注意

- 只有大型怪物会产生怪物事件。
- 联机时队友的伤害和倒地从游戏数据里读不到完整信息，`player_down` 只算自己。
- 在任务中途连上游戏时，第一帧会发出这个任务的 `quest_start` 和在场大型怪物的 `monster_spawn`。
