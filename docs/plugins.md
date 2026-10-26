# 写插件

插件是一个 WebAssembly 模块加一份清单，不用改主程序就能装进来。可以用 Rust、C、Go 等能编译到 `wasm32-unknown-unknown` 的语言写。插件 API 版本 1.0。

插件能做的事：

- 读数据快照（和主题模板同一张字段表，见 `fields.md`）、订阅事件（见 `events.md`）；
- 注册局内模块，和内置模块一样参与布局模式、方案、显示设置；模块按当前主题绘制；
- 注册热键动作、设置项、记录导出格式、通知渠道；
- 往狩猎记录里写自定义字段。

插件不能做的事：读写游戏内存、注入、碰自己文件夹以外的文件、在没有授权时联网。没有声明的能力一律拿不到。

仓库的 `examples/plugins/split-timer/` 是一个完整的示例（分段计时），用 Rust 写。

## 插件包

一个 zip 或文件夹：

```
split-timer/
  plugin.toml   清单
  plugin.wasm   入口（文件名可以在清单里改）
  split.xml     局内模块的界面描述
```

插件页"从文件安装"读 zip，安装前列出作者和要用的权限，确认后装到数据目录的 `plugins/<id>/`。插件自己的文件夹是其中的 `data/`，升级时保留。

## 清单 plugin.toml

```toml
id = "split-timer"            # 只用字母、数字、- 和 _
name = "分段计时"
description = "..."
version = "1.0.0"
author = "..."
app = "1.0"                   # 需要的程序版本
api = "1.0"                   # 需要的插件 API 版本
entry = "plugin.wasm"
permissions = ["files"]       # net、files、save、port
events = ["quest_start", "quest_end", "part_break"]
frame_hz = 4                  # 每秒收几次数据快照，0 表示不要，最多 30

[[modules]]
id = "split"
name = "分段计时"
template = "split.xml"
default_on = true
elements = [{ key = "diff", name = "和最好成绩的差" }]

[[actions]]                   # 出现在设置的热键页
id = "split"
name = "分段计时 手动分段"
key = "F9"                    # 默认热键，可以留空

[[settings]]                  # 出现在插件页，由程序保存
key = "rows"
name = "显示段数"
type = "number"               # bool、number、text
default = 5
min = 1
max = 10

[[exporters]]                 # 出现在记录页的"导出"菜单
id = "markdown"
name = "Markdown 表格"
ext = "md"

[[channels]]                  # 出现在设置的通知页
id = "phone"
name = "手机推送"
```

程序启动时检查 `app` 和 `api`：程序版本更低、或者 API 的大版本不同、小版本更高时，插件停用并在插件页写明原因。

## 权限

| 权限 | 能用的宿主函数 |
|---|---|
| `files` | `fs_read`、`fs_write`：只在插件自己的 `data/` 文件夹里，路径不能带 `..`、盘符或开头的斜杠 |
| `net` | `http_get`：HTTP 或 HTTPS 的 GET，5 秒超时，最多 4 MB |
| `save` | `save_read`：读图鉴（存档里的猎人笔记），JSON |
| `port` | `serve_json`：在 127.0.0.1 上开一个只读端口，回应最新的一份 JSON，例如给 OBS 浏览器源用 |

安装时确认清单里的全部权限。之后每项权限都能在插件页单独收回；再打开时要重新确认。从插件页装新版本时同样先确认；直接替换文件夹升级的，新增的权限默认关闭，要用户在插件页打开。

## 运行方式

- 每个插件一个线程、一个独立的 wasmtime 实例，内存上限 64 MB。
- 单次调用超过 500 ms，或者出错（trap），这个插件被停掉，插件页显示"运行出错，已自动停用"，日志里写明原因；主程序和 overlay 不受影响。用户可以在插件页重新启用。
- 联网请求等待的时间不算进 500 ms。

## 二进制接口

所有字符串都是 UTF-8，复杂数据都是 JSON。

### 插件导出

| 导出 | 签名 | 说明 |
|---|---|---|
| `memory` | | 线性内存（必需） |
| `rh_alloc` | `(len: i32) -> i32` | 宿主写数据进来之前先调用它要一块内存（必需） |
| `rh_init` | `(ptr: i32, len: i32)` | 启动时调用一次，参数是设置 JSON |
| `rh_settings` | `(ptr, len)` | 用户改了设置，参数是全部设置的 JSON |
| `rh_event` | `(ptr, len)` | 订阅的事件，JSON 见 `events.md` |
| `rh_frame` | `(ptr, len)` | 数据快照，按 `frame_hz` 调用，JSON 的字段见 `fields.md` |
| `rh_action` | `(ptr, len)` | 热键动作，参数是动作编号 |
| `rh_notify` | `(ptr, len)` | 通知渠道，`{"channel": 编号, "title": ..., "body": ..., "time": Unix 秒}` |
| `rh_export` | `(ptr, len) -> i64` | 导出器，输入 `{"exporter": 编号, "hunts": [...]}`，返回文件内容 |

除了 `memory` 和 `rh_alloc`，没导出的函数就不调用。

内存归属：宿主通过 `rh_alloc` 要来的内存，写进去之后归插件，用完由插件释放。宿主函数返回数据时也一样（先调 `rh_alloc`，再写进去）。

返回数据的函数用一个 `i64` 打包：高 32 位是指针，低 32 位是长度；负数表示失败（`-1` 出错，`-2` 没有权限）。`rh_export` 返回的内存在插件里留到下次调用再释放。

### 宿主函数（模块名 `rh`）

| 函数 | 签名 | 权限 |
|---|---|---|
| `log` | `(level: i32, ptr, len)` | 0 错误、1 警告、2 信息、3 调试 |
| `set_module_data` | `(module_ptr, module_len, json_ptr, json_len)` | 把数据交给某个局内模块，模板里在 `data` 下面读 |
| `set_record_field` | `(key_ptr, key_len, value_ptr, value_len)` | 往这一场狩猎的记录里写字段，任务结束时一起保存 |
| `notify` | `(title_ptr, title_len, body_ptr, body_len)` | 发一条通知 |
| `now_ms` | `() -> i64` | Unix 毫秒 |
| `fs_read` | `(path_ptr, path_len) -> i64` | `files` |
| `fs_write` | `(path_ptr, path_len, data_ptr, data_len) -> i32` | `files`；0 成功 |
| `http_get` | `(url_ptr, url_len) -> i64` | `net` |
| `save_read` | `() -> i64` | `save` |
| `serve_json` | `(port: i32, json_ptr, json_len) -> i32` | `port`；端口 1024 到 65535 |

## 局内模块

插件不直接画图。清单里 `template` 指向一份界面描述，格式和主题的模块模板相同（见 `theme.md`），根元素可以直接是 `<column>` 等，不用包 `<module>`。程序按当前主题统一绘制，所以插件模块和内置模块风格一致。

- 模板里能用全部数据字段，插件交来的数据在 `data` 下面，模块页的元素开关在 `module.show.<元素>`。
- 主题可以单独给插件模块写模板：`<module id="split-timer.split">`。
- 模块在设置和布局里的编号是 `插件编号.模块编号`，首次启用时放在任务时钟下面，用户可以在布局模式里拖走。

## 用 Rust 写

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

把 `target/wasm32-unknown-unknown/release/<名字>.wasm` 改名为 `plugin.wasm`，和 `plugin.toml`、模板一起打成 zip。

## 版本

插件 API 按语义化版本管理：小版本只加不删；要删的东西提前一个大版本标记废弃。
