# Data fields

Theme templates and plugins get data snapshots built from the same field table. Field table version 1.0; minor versions only add fields, never remove them.

`[]` means every item of a list. Templates iterate with `<for each="p in monster.parts">`; plugins get a JSON array.
This file is checked by `cargo test -p ui-kit fields_doc`; after changing the field table, regenerate it with `RH_WRITE_DOCS=1`.

## game

| Field | Type | Description |
|---|---|---|
| `game.key` | string | world, rise, wilds |
| `game.stage` | string | Name of the current location |
| `game.in_quest` | bool | In a quest |
| `game.in_town` | bool | In a hub or room |
| `game.hud_open` | bool | The game menu or map is open |
| `game.world_time` | string | In-game time, like 06:40 |
| `game.online` | bool | Multiplayer quest |

## overlay

| Field | Type | Description |
|---|---|---|
| `overlay.density` | string | full, standard, minimal |
| `overlay.time` | number | Seconds since the overlay started, for animations |

## quest

| Field | Type | Description |
|---|---|---|
| `quest.id` | number | Quest ID |
| `quest.stars` | number | Star rating |
| `quest.rank` | string | low, high, master |
| `quest.time_left` | number | Seconds left |
| `quest.time_max` | number | Time limit, seconds |
| `quest.elapsed` | number | Seconds elapsed |
| `quest.deaths` | number | Faints so far |
| `quest.deaths_max` | number | Faints allowed |

## monster

| Field | Type | Description |
|---|---|---|
| `monster.name` | string | Target monster's name (in the term language) |
| `monster.name_en` | string | English name |
| `monster.key` | string | Key in the term table |
| `monster.hp` | number | Current HP |
| `monster.hp_max` | number | Max HP |
| `monster.hp_ratio` | number | HP ratio, 0 to 1 |
| `monster.stamina` | number | Stamina |
| `monster.stamina_max` | number | Max stamina |
| `monster.stamina_ratio` | number | Stamina ratio |
| `monster.enraged` | bool | Enraged |
| `monster.enrage_timer` | number | Seconds of rage left |
| `monster.enrage_ratio` | number | Ratio of rage left |
| `monster.tired` | bool | Tired |
| `monster.capturable` | bool | Can be captured |
| `monster.capture_ratio` | number | Capture threshold (HP ratio) |
| `monster.crown` | string | none, mini, silver, gold |
| `monster.size` | number | Size multiplier |
| `monster.weaknesses` | list | Names of weak elements |
| `monster.target` | string | own, other, none |
| `monster.captured` | bool | Captured |
| `monster.dead` | bool | Slain |
| `monster.damage_recent` | number | Damage of the latest attack chain |
| `monster.parts` | list | Parts; item fields under monster.parts[].* |
| `monster.parts[].name` | string | Part name |
| `monster.parts[].key` | string | Localization key of the part |
| `monster.parts[].kind` | string | flinch, breakable, severable |
| `monster.parts[].flinch_ratio` | number | Flinch ratio |
| `monster.parts[].durability_ratio` | number | Durability ratio left until the next break |
| `monster.parts[].break_progress` | number | Break progress, 1 minus the durability ratio |
| `monster.parts[].breaks` | number | Times broken |
| `monster.parts[].breaks_max` | number | Possible breaks |
| `monster.parts[].broken` | bool | Fully broken |
| `monster.parts[].severable` | bool | Severable |
| `monster.parts[].tenderized` | number | Seconds of tenderize left |
| `monster.parts[].hot` | bool | Hit in the last few seconds |
| `monster.parts[].idle` | bool | Idle, for fading out |
| `monster.ailments` | list | Monster ailments; item fields under monster.ailments[].* |
| `monster.ailments[].name` | string | Ailment name |
| `monster.ailments[].key` | string | Localization key of the ailment |
| `monster.ailments[].buildup_ratio` | number | Buildup ratio |
| `monster.ailments[].timer` | number | Seconds left while active |
| `monster.ailments[].timer_ratio` | number | Ratio left while active |
| `monster.ailments[].count` | number | Times triggered |
| `monster.ailments[].active` | bool | Active |
| `monster.ailments[].hot` | bool | Changed in the last few seconds |
| `monster.ailments[].idle` | bool | Idle |

## monsters

| Field | Type | Description |
|---|---|---|
| `monsters` | list | All large monsters in the area; item fields are the same as monster |

## player

| Field | Type | Description |
|---|---|---|
| `player.name` | string | Hunter name |
| `player.hr` | number | Hunter Rank |
| `player.mr` | number | Master Rank |
| `player.hp` | number | Health |
| `player.hp_max` | number | Max health |
| `player.hp_ratio` | number | Health ratio |
| `player.hp_recoverable` | number | Value red health can recover to |
| `player.hp_healing_to` | number | Value currently being healed to |
| `player.stamina` | number | Stamina |
| `player.stamina_max` | number | Max stamina |
| `player.stamina_ratio` | number | Stamina ratio |
| `player.weapon` | string | Weapon type key |
| `player.weapon_name` | string | Weapon type name |
| `player.sharpness.level` | number | Sharpness color, 0 red to 6 purple |
| `player.sharpness.color` | string | Sharpness color name |
| `player.sharpness.hits_left` | number | Hits left in the current color |
| `player.sharpness.ratio` | number | Current value as a ratio of the max |
| `player.buffs` | list | Buffs; each has name, timer, timer_max, ratio, infinite |
| `player.buffs[].name` | string | Buff name |
| `player.buffs[].timer` | number | Seconds left |
| `player.buffs[].ratio` | number | Ratio left |
| `player.buffs[].infinite` | bool | No countdown |
| `player.debuffs` | list | Status effects on the player; same fields as player.buffs |

## party

| Field | Type | Description |
|---|---|---|
| `party` | list | Party, by damage from high to low |
| `party[].name` | string | Hunter name |
| `party[].damage` | number | Total damage |
| `party[].dps` | number | Damage per second |
| `party[].share` | number | Share of damage |
| `party[].rank` | number | Rank, starting at 1 |
| `party[].is_self` | bool | Is the local player |

## base

| Field | Type | Description |
|---|---|---|
| `base.argosy_days` | number | Days left for the Argosy |
| `base.tailraiders_quests` | number | Tailraider expeditions left |
| `base.harvest_items` | number | Items in the Botanical Research harvest box |
| `base.steam_stored` | number | Fuel stored at the Steamworks |
