# Roadmap

## 1.0.0

Released. Fixed since the release:

- On a multi-monitor setup with different scaling (for example a 150% primary screen and the game on a 100% screen), the overlay window shrank to 1280x720 and drew in the wrong place.
- The integrity patch and the damage hook are now on by default, and our own integrity patch is applied even when Stracker's Loader is installed.
- The overlay drew nothing on a 4K monitor at 150% scaling (OpenGL into a layered window). It now renders with wgpu on DX12 through DirectComposition and was checked on both monitors.
- With RustyHunt already running when the game started, the hook was installed in the same seconds as Stracker's Loader plugins and more_auto_shoutouts' damage hook could fail. Injection now waits until a character is loaded.

Still not tested in the real game:

- After the game window moves to another monitor, the overlay sometimes shows the old frame for a second or two; it now keeps repainting for a second after each move, not yet checked in game.
- Full install and uninstall of the installer build (the extraction logic has tests).

## 1.1.0: issues that need no injection or reverse engineering

**Hunter's notes**

- Save slots 59, 66, 67 and 70+ are not mapped to monsters yet, so Furious Rajang, Raging Brachydios, Fatalis and Scarred Yian Garuga don't appear in the notes. Each one needs checking in the in-game Hunter's Notes.
- Research level caps: checked against the in-game research list. Dodogama, Lavasioth, Uragaan, Bazelgeuse and Nergigante were corrected to 7. Not seen in the list yet: Kulve Taroth, Behemoth, Ancient Leshen, and Leshen's Master Rank stars.
- Crown quest column: done, from Kiranico game data (`data/world/crown_quests.toml`). The Heart of the Nora (Deviljho) and The Proving (Anjanath), which community lists mark as guaranteed, are PS4-only Horizon Zero Dawn collaboration quests and aren't included.
- Done: navigation levels are read from the save; monsters are listed in the in-game research list order; the header shows when the game last saved, and the notes reread the save after the game writes it.

**Multiplayer**

- As a guest the game doesn't sync part durability, so part breaks are estimated from flinch counts and may be wrong. Done so far: within one quest, a part of the same monster that has broken stays broken; the record resets when the quest ends or the monster changes.

**Other**

- Memory: the target is 80 MB. With OpenGL the overlay used about 110 MB; with wgpu on DX12 (needed for the 4K fix) it is about 205 MB working set and 360 MB private on a 4K monitor, much of it the full-screen swapchain. Options: fewer swapchain buffers, or an overlay made of small windows sized to each module like HunterPie's.
- The backing blur strength is an approximation. The blur window is solid black on the 1080p monitor, and it stays on top of other programs while the overlay is hidden because the game isn't in front.

## 1.1.1: UI and copy

- Polish the UI and the wording, and make the app easier to use.

## 1.2.0: needs injection, disassembly or memory scanning

- Split damage into raw and elemental: the hooked damage function only sees the total, so the function that computes damage has to be found.
- Party members' current HP: the in-game party list shows it, but the address hasn't been found; needs scanning in a live multiplayer session.
- Detect whether we are the quest host, and hide part durability bars as a guest.
- Solo damage without the hook: currently estimated from the monster's HP loss, which also counts the Palico, turf wars and environmental damage.
