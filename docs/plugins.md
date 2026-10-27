# Writing plugins

A plugin is a WebAssembly module plus a manifest, and it installs without changing the main program. Write it in any language that compiles to `wasm32-unknown-unknown`, such as Rust, C or Go. Plugin API version 1.0.

What plugins can do:

- read data snapshots (the same field table as theme templates, see `fields.md`) and subscribe to events (see `events.md`);
- register in-game modules that take part in layout mode, profiles and display settings like built-in modules; modules are drawn in the current theme;
- register hotkey actions, settings, record export formats and notification channels;
- write custom fields into hunt records.

What plugins can't do: read or write game memory, inject, touch files outside their own folder, or go online without permission. Anything not declared is unavailable.

`examples/plugins/split-timer/` in the repository is a complete example (a split timer) written in Rust.

## Plugin package

A zip or a folder:

```
split-timer/
  plugin.toml   manifest
  plugin.wasm   entry (the file name can be changed in the manifest)
  split.xml     UI description of the in-game module
```

"Install from file" on the Plugins page reads the zip, lists the author and the permissions it wants, and after confirmation installs it into `plugins/<id>/` in the data directory. The plugin's own folder is `data/` inside it and is kept across upgrades.

## Manifest plugin.toml

```toml
id = "split-timer"            # letters, digits, - and _ only
name = "Split timer"
description = "..."
version = "1.0.0"
author = "..."
app = "1.0"                   # required app version
api = "1.0"                   # required plugin API version
entry = "plugin.wasm"
permissions = ["files"]       # net, files, save, port
events = ["quest_start", "quest_end", "part_break"]
frame_hz = 4                  # data snapshots per second; 0 means none, at most 30

[[modules]]
id = "split"
name = "Split timer"
template = "split.xml"
default_on = true
elements = [{ key = "diff", name = "Difference from best" }]

[[actions]]                   # shown on the Hotkeys tab of Settings
id = "split"
name = "Split timer: manual split"
key = "F9"                    # default hotkey; may be empty

[[settings]]                  # shown on the Plugins page and saved by the app
key = "rows"
name = "Splits shown"
type = "number"               # bool, number, text
default = 5
min = 1
max = 10

[[exporters]]                 # shown in the Export menu of the Records page
id = "markdown"
name = "Markdown table"
ext = "md"

[[channels]]                  # shown on the Notifications tab of Settings
id = "phone"
name = "Phone push"
```

At startup the app checks `app` and `api`: if the app version is lower, or the API's major version differs or its minor version is higher, the plugin is disabled and the Plugins page shows why.

## Permissions

| Permission | Host functions it allows |
|---|---|
| `files` | `fs_read`, `fs_write`: only inside the plugin's own `data/` folder; paths can't contain `..`, a drive letter or a leading slash |
| `net` | `http_get`: HTTP or HTTPS GET, 5-second timeout, at most 4 MB |
| `save` | `save_read`: reads the Hunter's Notes from the save, as JSON |
| `port` | `serve_json`: opens a read-only port on 127.0.0.1 that serves the latest JSON, for an OBS browser source for example |

All permissions in the manifest are confirmed at install time. Afterwards each one can be revoked on the Plugins page, and turning it back on needs a new confirmation. Installing a new version from the Plugins page asks for confirmation too; when upgrading by replacing the folder directly, new permissions start off and the user turns them on in the Plugins page.

## How plugins run

- One thread and one separate wasmtime instance per plugin, with a 64 MB memory limit.
- If a single call takes more than 500 ms or traps, that plugin is stopped. The Plugins page shows it was disabled after an error and the log gives the reason; the main program and the overlay are unaffected. The user can enable it again on the Plugins page.
- Time spent waiting on network requests doesn't count toward the 500 ms.

## Binary interface

All strings are UTF-8 and all structured data is JSON.

### Plugin exports

| Export | Signature | Description |
|---|---|---|
| `memory` | | Linear memory (required) |
| `rh_alloc` | `(len: i32) -> i32` | Called by the host to get a block of memory before writing data in (required) |
| `rh_init` | `(ptr: i32, len: i32)` | Called once at startup with the settings JSON |
| `rh_settings` | `(ptr, len)` | The user changed settings; the argument is the JSON of all settings |
| `rh_event` | `(ptr, len)` | A subscribed event; JSON described in `events.md` |
| `rh_frame` | `(ptr, len)` | Data snapshot, called at `frame_hz`; JSON fields in `fields.md` |
| `rh_action` | `(ptr, len)` | Hotkey action; the argument is the action ID |
| `rh_notify` | `(ptr, len)` | Notification channel: `{"channel": id, "title": ..., "body": ..., "time": Unix seconds}` |
| `rh_export` | `(ptr, len) -> i64` | Exporter: takes `{"exporter": id, "hunts": [...]}` and returns the file contents |

Apart from `memory` and `rh_alloc`, functions that aren't exported are simply not called.

Memory ownership: memory the host gets through `rh_alloc` belongs to the plugin once written, and the plugin frees it when done. The same goes for data returned by host functions (the host calls `rh_alloc`, then writes into it).

Functions that return data pack it into an `i64`: the high 32 bits are the pointer and the low 32 bits the length; a negative value means failure (`-1` error, `-2` no permission). Memory returned by `rh_export` stays in the plugin until the next call frees it.

### Host functions (module name `rh`)

| Function | Signature | Permission |
|---|---|---|
| `log` | `(level: i32, ptr, len)` | 0 error, 1 warning, 2 info, 3 debug |
| `set_module_data` | `(module_ptr, module_len, json_ptr, json_len)` | Hands data to an in-game module; templates read it under `data` |
| `set_record_field` | `(key_ptr, key_len, value_ptr, value_len)` | Writes a field into this hunt's record, saved when the quest ends |
| `notify` | `(title_ptr, title_len, body_ptr, body_len)` | Sends a notification |
| `now_ms` | `() -> i64` | Unix milliseconds |
| `fs_read` | `(path_ptr, path_len) -> i64` | `files` |
| `fs_write` | `(path_ptr, path_len, data_ptr, data_len) -> i32` | `files`; 0 on success |
| `http_get` | `(url_ptr, url_len) -> i64` | `net` |
| `save_read` | `() -> i64` | `save` |
| `serve_json` | `(port: i32, json_ptr, json_len) -> i32` | `port`; port 1024 to 65535 |

## In-game modules

Plugins don't draw directly. `template` in the manifest points to a UI description in the same format as a theme's module templates (see `theme.md`); the root element can be `<column>` or similar directly, without a `<module>` wrapper. The app draws it in the current theme, so plugin modules match the built-in ones.

- Templates can use every data field; the plugin's data is under `data`, and the element switches from the Modules page are under `module.show.<element>`.
- Themes can write their own template for a plugin module: `<module id="split-timer.split">`.
- In settings and layouts a module's ID is `plugin ID.module ID`. When first enabled it is placed under the quest clock, and the user can drag it elsewhere in layout mode.

## Writing one in Rust

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
serde_json = "1"

[profile.release]
opt-level = "s"
lto = true
panic = "abort"
```

```rust
#[link(wasm_import_module = "rh")]
unsafe extern "C" {
    fn log(level: i32, ptr: *const u8, len: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn rh_alloc(len: i32) -> *mut u8 {
    Box::into_raw(vec![0u8; len as usize].into_boxed_slice()) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn rh_event(ptr: *mut u8, len: i32) {
    let bytes = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len as usize)) };
    let text = String::from_utf8_lossy(&bytes);
    unsafe { log(2, text.as_ptr(), text.len() as i32) };
}
```

```
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Rename `target/wasm32-unknown-unknown/release/<name>.wasm` to `plugin.wasm` and zip it with `plugin.toml` and the template.

## Versioning

The plugin API follows semantic versioning: minor versions only add, never remove; anything to be removed is marked deprecated one major version ahead.
