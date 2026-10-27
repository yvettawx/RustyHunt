# Roadmap

## 1.0.0

Released. Fixed since the release:

- On a multi-monitor setup with different scaling (for example a 150% primary screen and the game on a 100% screen), the overlay window shrank to 1280x720 and drew in the wrong place.
- The integrity patch and the damage hook are now on by default, and our own integrity patch is applied even when Stracker's Loader is installed.
- The overlay drew nothing on a 4K monitor at 150% scaling (OpenGL into a layered window). It now renders with wgpu on DX12 through DirectComposition; checked in game on both monitors and when moving between them.
- The overlay (topmost) stayed on top of other programs when the game was in the background; it now hides until the game is in front again.
- With RustyHunt already running when the game started, the hook was installed in the same seconds as Stracker's Loader plugins and more_auto_shoutouts' damage hook could fail. Injection now waits until a character is loaded.

Still not tested in the real game:

- Full install and uninstall of the installer build (the extraction logic has tests).

## 1.1.0: issues that need no injection or reverse engineering

Finished on 2026-10-27. Left open: save slots 59, 66, 67 and 70 haven't been checked against a save that has hunted those monsters, and the memory target is deferred.

**Hunter's notes**

- Save slots 59, 66, 67 and 70 (Scarred Yian Garuga, Furious Rajang, Raging Brachydios, Fatalis) are mapped from MHWISaveEditor's save template, which agrees with every slot already checked. Not yet seen in a save that has hunted them.
- Research level caps: High Rank monsters without Master Rank research (Bazelgeuse, Nergigante, Vaal Hazak, Deviljho, Behemoth, Leshen, Ancient Leshen) are 4, from a community list and PSNProfiles' trophy guide; Dodogama, Lavasioth and Uragaan are 7. Kulve Taroth is 7: it has Master Rank research.
- Crown quest column: done, from Kiranico game data (`data/world/crown_quests.toml`). The Heart of the Nora (Deviljho) and The Proving (Anjanath), which community lists mark as guaranteed, are PS4-only Horizon Zero Dawn collaboration quests and aren't included.
- Done: the monster and area modules show when a monster beats the recorded largest or smallest size, and its research and scoutfly levels.
- Done: navigation levels are read from the save; monsters are listed in the in-game research list order; the header shows when the game last saved, and the notes reread the save after the game writes it.

**Multiplayer**

- As a guest the game doesn't sync part durability, so part breaks are estimated from flinch counts and may be wrong. Done so far: within one quest, a part of the same monster that has broken stays broken; the record resets when the quest ends or the monster changes.

**Other**

- Memory: the target is 80 MB. With OpenGL the overlay used about 110 MB; with wgpu on DX12 (needed for the 4K fix) about 190 MB working set and 350 MB private. Fewer swapchain buffers, low-power adapter or immediate present change it by under 5 MB, and an empty 400x300 wgpu window already takes 340 MB private: the cost is the GPU driver itself (on NVIDIA, nvgpucomp64.dll and nvwgf2umx.dll alone are 180 MB of images), so smaller or fewer GPU windows won't help. Reaching the target needs an overlay that doesn't touch the GPU: draw egui on the CPU and hand each module's bitmap to a small layered window with UpdateLayeredWindow, the way WPF transparent windows (HunterPie) work.
- Log and error messages are in English (UI text stays localized).
- Player backing: done. It follows the player module and fades out over a wide margin; blur is built from nested acrylic windows (default 60%) because the plain blur effect showed solid black and acrylic has a hard edge. Layout mode now runs in its own window.

## 1.1.1: UI and copy

- Polish the UI and the wording, and make the app easier to use.

## 1.2.0: needs injection, disassembly or memory scanning

- Split damage into raw and elemental: the hooked damage function only sees the total, so the function that computes damage has to be found.
- Party members' current HP: the in-game party list shows it, but the address hasn't been found; needs scanning in a live multiplayer session.
- Detect whether we are the quest host, and hide part durability bars as a guest.
- Solo damage without the hook: currently estimated from the monster's HP loss, which also counts the Palico, turf wars and environmental damage.
