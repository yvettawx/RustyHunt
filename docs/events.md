# Events

Each time the reader thread reads a frame, it compares it with the previous one to find the events that happened in this frame. Overlay callouts, the hunt record timeline and plugins all use these events.

Event list version 1.0: minor versions only add events, never remove them; an event to be removed is marked deprecated one major version ahead. Plugins list the events they subscribe to under `events` in `plugin.toml`; a name that doesn't exist makes the install fail.

Each event a plugin receives is a JSON object: `type` is the event name, the other keys are the fields in the table below, and every event also carries `elapsed` (quest time when it happened, in seconds).

```json
{"type": "part_break", "uid": 2216434944, "part": "PART_HEAD", "count": 1, "elapsed": 412.5}
```

## List

| Event | Fields | When |
|---|---|---|
| `quest_start` | `quest_id`, `stars` | A quest starts; when switching quests, the previous quest's `quest_end` comes first |
| `quest_end` | `quest_id`, `status`, `elapsed` | A quest ends. `status` is `success`, `fail` or `quit` |
| `monster_spawn` | `uid`, `key` | A large monster comes into reading range |
| `monster_despawn` | `uid`, `key` | A large monster leaves reading range (area change, leaving the quest) |
| `monster_death` | `uid`, `key` | Slain |
| `monster_capture` | `uid`, `key` | Captured |
| `part_break` | `uid`, `part`, `count` | A part breaks; `count` is how many times this part has broken |
| `part_sever` | `uid`, `part` | A tail or similar part is severed |
| `ailment_trigger` | `uid`, `ailment`, `count` | An ailment triggers (paralysis, sleep, stun and so on); `count` is the number of times |
| `enrage` | `uid` | Becomes enraged |
| `enrage_end` | `uid` | Rage ends |
| `tired` | `uid` | Becomes tired |
| `capturable` | `uid` | HP drops below the capture threshold |
| `player_down` | `deaths`, `deaths_max` | The player faints |
| `stage_change` | `stage_id` | The location changes |
| `weapon_change` | `weapon` | The weapon changes; the value is the weapon key (`longsword` and so on) |
| `abnormality_start` | `id`, `key` | A buff or status effect starts |
| `abnormality_end` | `id`, `key` | A buff or status effect ends |

## Fields

- `uid`: this monster's ID for the current connection. It stays the same for the same monster and matches the item in `monsters[]`.
- `key`: the monster's term key, e.g. `raging_brachydios`, the same as the keys in `locales/terms/world.toml`.
- `part`, `ailment`: copy keys of the part or ailment, e.g. `PART_HEAD`, `AILMENT_PARALYSIS`, the same as the keys in `locales/game/*.toml`.
- `id`, `key` (buffs): `id` is the buff's ID in the data table, `key` its copy key, e.g. `ABNORMALITY_DEMONDRUG`.

## Notes

- Only large monsters produce monster events.
- In multiplayer the game data doesn't fully cover other players' damage and faints, so `player_down` only counts the local player.
- When connecting to the game in the middle of a quest, the first frame sends this quest's `quest_start` and a `monster_spawn` for each large monster present.
