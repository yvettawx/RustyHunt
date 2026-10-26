# RustyHunt

《怪物猎人 世界》的局内信息 overlay 和桌面程序，用 Rust 写成。基于 [HunterPie](https://github.com/HunterPie/HunterPie) 改写，遵循 Apache License 2.0，见 [LICENSE](LICENSE) 和 [NOTICE](NOTICE)。与 HunterPie 团队无关；怪物猎人相关名称归 CAPCOM 所有。

## 功能

- **overlay**：怪物（血量、精力、部位、异常、激怒和疲劳、皇冠、弱点）、自身（体力、耐力、斩味、异常）、增益、伤害和 DPS 曲线、本区域、时钟、武器专用槽、据点设施、性能。透明、置顶、鼠标穿透；画面静止时不重绘。
- **布局模式**：拖动、缩放、吸附、撤销；每个游戏可以存多套方案，三档显示密度，按住显示全部。
- **记录**：每场狩猎存进本地 SQLite，看 DPS 曲线、各人伤害、时间轴；导出 CSV、JSON。图鉴读存档里的猎人笔记（只读），看金冠和研究等级。
- **主题**：XML 写的三层主题（令牌、组件样式、模块模板），保存即生效；附 XSD、两个示例主题和命令行校验工具 `theme-check`。
- **插件**：WebAssembly 插件，权限在清单里声明、安装时确认；插件卡住或出错只停掉它自己。可以注册局内模块、热键动作、设置、导出格式、通知渠道。
- **补丁与注入**（默认关闭，开启前确认）：完整性检查补丁、伤害 hook（会心统计、单人时的准确伤害）。
- 界面语言：简体中文、繁體中文、日本語、English；游戏术语可以单独选语言。
- 首次启动六步引导；托盘；开机启动；更新检查（SHA-256 校验，失败回退）；地址表单独更新；崩溃日志。

不发送任何遥测。联网只用于访问 GitHub 检查更新；插件联网需要用户授权。插件永远不能读写游戏内存。

## 安装

在 Releases 页下载：

- `RustyHunt-<版本>-setup.exe`：安装版，装在当前用户目录（`%LOCALAPPDATA%\Programs\RustyHunt`），不需要管理员权限；数据在 `%APPDATA%\RustyHunt\`。可以在系统的"设置 > 应用"里卸载。
- `RustyHunt-<版本>-portable.zip`：便携版，解压即用，数据放在程序旁边的 `data\`。

需要 Windows 10 或更新的版本。目前只支持《怪物猎人 世界》（Steam 版），崛起和荒野以后支持。

## 文档

- [docs/theme.md](docs/theme.md)：写主题；[docs/theme.xsd](docs/theme.xsd)：主题格式
- [docs/plugins.md](docs/plugins.md)：写插件
- [docs/fields.md](docs/fields.md)：数据字段
- [docs/events.md](docs/events.md)：事件
- 示例：[examples/themes](examples/themes)、[examples/plugins/split-timer](examples/plugins/split-timer)

## 从源码编译

```
cargo build --release
```

工作区里的 crate：

| crate | 内容 |
|---|---|
| `address-map` | 地址表（`.map`）解析 |
| `platform` | Windows 进程、内存、窗口、热键、Steam 目录 |
| `core` | 数据快照、事件、设置、文案、模块描述 |
| `game-world` | 世界的内存读取、存档解密、演示数据 |
| `ui-kit` | 主题、字段表、模板引擎、绘制；`theme-check` 工具 |
| `plugin-host` | 插件清单和 WebAssembly 运行时 |
| `native` | 注入游戏的伤害 hook（`rustyhunt-native.dll`） |
| `rustyhunt` | 程序本体：overlay、主界面、托盘、更新 |
| `setup` | 安装版的安装和卸载程序 |

打发布包（便携版、安装版、SHA256SUMS）：

```
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

开发时可以不开游戏，用演示数据看界面：

```
cargo run -p rustyhunt -- --demo
```
