# Roadmap

## 1.0.0

Released. Still not tested in the real game: full install and uninstall of the installer build (the extraction logic has tests).

## 1.1.0

Released.

**Overlay**

- Multi-monitor setups with different scaling (for example a 150% primary screen and the game on a 100% screen): the overlay window no longer shrinks to 1280x720 or draws in the wrong place, and it follows the game between monitors.
- The overlay drew nothing on a 4K monitor at 150% scaling (OpenGL into a layered window). It now renders with wgpu on DX12 through DirectComposition.
- The overlay hides while the game is in the background instead of staying on top of other programs.
- Layout mode runs in its own window.
- Player backing follows the player module and fades out over a wide margin; blur is built from nested acrylic windows (default 60%).
- The monster and area modules show each monster's research and scoutfly levels, and mark a monster that beats the recorded largest or smallest size.

**Patching and injection**

- The integrity patch and the damage hook are on by default, and our own integrity patch is applied even when Stracker's Loader is installed.
- Injection waits until a character is loaded, so it no longer races Stracker's Loader plugins (more_auto_shoutouts' damage hook could fail).

**Hunter's notes**

- Scoutfly levels are read from the save; monsters are listed in the in-game research list order; the header shows when the game last saved, and the notes reread the save after the game writes it.
- Crown quest column, from Kiranico game data (`data/world/crown_quests.toml`). The Heart of the Nora (Deviljho) and The Proving (Anjanath), which community lists mark as guaranteed, are PS4-only Horizon Zero Dawn collaboration quests and aren't included.
- Save slots 59, 66, 67 and 70 (Scarred Yian Garuga, Furious Rajang, Raging Brachydios, Fatalis) are mapped, from MHWISaveEditor's save template.
- Research level caps corrected: High Rank monsters without Master Rank research (Bazelgeuse, Nergigante, Vaal Hazak, Deviljho, Behemoth, Leshen, Ancient Leshen) are 4; Dodogama, Lavasioth and Uragaan are 7.

**Multiplayer**

- As a guest the game doesn't sync part durability, so part breaks are estimated from flinch counts. Within one quest, a part of the same monster that has broken now stays broken; the record resets when the quest ends or the monster changes.

**Other**

- Log and error messages are in English (UI text stays localized).
- README in English and Simplified Chinese.

## 1.1.1: UI and copy

Built, not yet checked in the game.

**Overlay**

- The monster module shows only once you select a monster (lock-on, the in-game selection, a hit in last-hit mode, or a manual switch).
- Parts and ailments share one grid at full opacity; ailments show how many times they triggered and flash when they trigger again; all parts or only recently hit ones.
- Exact numbers instead of percentages; no floating change numbers. The monster list no longer shows HP.
- The player's health bar takes the colour of the worst status effect, as in game; an enraged monster's HP bar turns red and flashes once.
- Thicker, shorter sharpness bar with the hits left as a number; Frostcraft is a gauge under it instead of a buff.
- Default look close to the game's HUD (backings fading to the right, green health, yellow stamina); Plate and Label are built-in alternatives. Each module can turn its backing off; the player module keeps its own (off, solid, blur).
- Layout mode: no toolbar and no native HUD outlines; drag corners to scale and side edges to change the width; the mouse wheel no longer scales.
- Fixed: anything a module drew left of or above its origin was clipped, so the callouts above the bottom monster module never showed.

**Main window**

- Hover, press and keyboard focus states with short transitions; one type scale; orange only for warnings.
- Exact HP on the home page; the home page rows are aligned; theme cards preview the overlay look.

**Copy**

- Module names say what they show (Monster status, Player status, Damage stats...); jargon replaced; fine options folded under More.

**To check in game**

- Frostcraft gauge: the value range is unknown, the gauge uses the largest value seen so far.
- Health bar colours for each status effect; target selection without a lock-on; dragging corners and edges in the real layout window.

## 1.2.0: needs injection, disassembly or memory scanning

- Split damage into raw and elemental: the hooked damage function only sees the total, so the function that computes damage has to be found.
- Party members' current HP: the in-game party list shows it, but the address hasn't been found; needs scanning in a live multiplayer session.
- Detect whether we are the quest host, and hide part durability bars as a guest.
- Solo damage without the hook: currently estimated from the monster's HP loss, which also counts the Palico, turf wars and environmental damage.
