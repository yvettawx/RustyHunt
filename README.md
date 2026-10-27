# RustyHunt

English | [简体中文](README.zh-CN.md)

An in-game overlay and desktop app for Monster Hunter: World, written in Rust. Rewritten from [HunterPie](https://github.com/HunterPie/HunterPie) under the Apache License 2.0; see [LICENSE](LICENSE) and [NOTICE](NOTICE). Not affiliated with the HunterPie team; Monster Hunter names belong to CAPCOM.

## Features

- **Overlay**: monster status for the monster you target (exact HP, parts, ailments with how often each triggered, enrage, fatigue, capture, crowns, weaknesses), player status (health tinted by status effects, stamina, sharpness, Frostcraft), buffs, damage stats with a DPS graph, monster list, quest time, weapon gauges, base facilities, performance. Looks close to the game's own HUD by default, with two more built-in looks. Transparent, topmost and click-through; follows the game across monitors with different scaling, and nothing is redrawn while the picture is still.
- **Layout mode**: drag to move, drag a corner to scale, drag a side to change the width; snap and undo. Several profiles per game, three density levels, and a hold-to-show-all key.
- **Records**: every hunt is stored in a local SQLite database with the DPS curve, per-player damage and a timeline; export to CSV or JSON. The Hunter's Notes are read from the save (read-only): crowns, research and scoutfly levels, and the quests most likely to give a crown. In a hunt, the overlay shows each monster's research and scoutfly level and marks one that beats your size record.
- **Themes**: three-layer XML themes (tokens, component styles, module templates) that apply on save. Comes with an XSD, two example themes and the `theme-check` command-line checker.
- **Plugins**: WebAssembly plugins. Permissions are declared in the manifest and confirmed on install; a stuck or failing plugin only stops itself. Plugins can register in-game modules, hotkey actions, settings, export formats and notification channels.
- **Patching and injection** (on by default, can be turned off in settings): integrity check patch and damage hook (crit stats, exact solo damage). Both modify the game process; use at your own risk.
- UI languages: Simplified Chinese, Traditional Chinese, Japanese, English; game terms can use a different language.
- Six-step first-run guide, tray icon, start at login, update checks (SHA-256 verified, with rollback), separate address map updates, crash logs.

No telemetry. The network is only used to check GitHub for updates; plugins need the user's permission to go online. Plugins can never read or write game memory.

## Install

Download from the [Releases](https://github.com/yvettawx/RustyHunt/releases) page:

- `RustyHunt-<version>-setup.exe`: installer. Installs into the current user's folder (`%LOCALAPPDATA%\Programs\RustyHunt`) without admin rights; data goes to `%APPDATA%\RustyHunt\`. Uninstall from Settings > Apps.
- `RustyHunt-<version>-portable.zip`: portable build. Unzip and run; data lives in `data\` next to the program.

Requires Windows 10 or later. Only Monster Hunter: World Iceborne is supported for now (Steam, game versions 421652, 421740, 421810); Rise and Wilds come later. When the game updates, address maps for the new version are published separately, without reinstalling the app.

The program isn't code-signed, so Windows SmartScreen warns on first run.

## Documentation

- [docs/theme.md](docs/theme.md): writing themes; [docs/theme.xsd](docs/theme.xsd): the theme format
- [docs/plugins.md](docs/plugins.md): writing plugins
- [docs/fields.md](docs/fields.md): data fields
- [docs/events.md](docs/events.md): events
- [docs/roadmap.md](docs/roadmap.md): roadmap and known issues
- Examples: [examples/themes](examples/themes), [examples/plugins/split-timer](examples/plugins/split-timer)

## Building from source

Needs Rust 1.88 or later (MSVC toolchain).

```
cargo build --release
```

Crates in the workspace:

| crate | Contents |
|---|---|
| `address-map` | Address map (`.map`) parsing |
| `platform` | Windows processes, memory, windows, hotkeys, Steam directories |
| `core` | Data snapshot, events, settings, copy, module descriptions |
| `game-world` | World memory reading, save decryption, demo data |
| `ui-kit` | Themes, field table, template engine, drawing; the `theme-check` tool |
| `plugin-host` | Plugin manifest and WebAssembly runtime |
| `native` | Damage hook injected into the game (`rustyhunt-native.dll`) |
| `rustyhunt` | The app: overlay, main window, tray, updates |
| `setup` | Installer and uninstaller for the installed build |

Build the release packages (portable, installer, SHA256SUMS):

```
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

During development you can look at the UI without the game, using demo data:

```
cargo run -p rustyhunt -- --demo
```
