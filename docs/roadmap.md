# Roadmap

## 1.0.0

Released. Fixed since the release:

- On a multi-monitor setup with different scaling (for example a 150% primary screen and the game on a 100% screen), the overlay window shrank to 1280x720 and drew in the wrong place.
- The integrity patch and the damage hook are now on by default, and our own integrity patch is applied even when Stracker's Loader is installed.

Still not tested in the real game:

- Integrity patch and damage hook inside the game process: the hook attached in one session, but the first attempt in an earlier session timed out on the IPC connection.
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

- With only the overlay open, memory use is about 128 MB against an 80 MB target. Measured on an RTX 4060 Ti with empty windows: glow 125 MB, wgpu DX12 185 MB, wgpu Vulkan 161 MB, so switching to wgpu doesn't help.
- The backing blur strength is an approximation.

## 1.1.1: UI and copy

- Polish the UI and the wording, and make the app easier to use.

## 1.2.0: needs injection, disassembly or memory scanning

- Split damage into raw and elemental: the hooked damage function only sees the total, so the function that computes damage has to be found.
- Party members' current HP: the in-game party list shows it, but the address hasn't been found; needs scanning in a live multiplayer session.
- Detect whether we are the quest host, and hide part durability bars as a guest.
- Solo damage without the hook: currently estimated from the monster's HP loss, which also counts the Palico, turf wars and environmental damage.
