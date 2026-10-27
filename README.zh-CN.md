# RustyHunt

[English](README.md) | 简体中文

《怪物猎人 世界》的游戏内 overlay 和桌面程序，用 Rust 编写。基于 [HunterPie](https://github.com/HunterPie/HunterPie) 按 Apache License 2.0 重写，见 [LICENSE](LICENSE) 和 [NOTICE](NOTICE)。与 HunterPie 团队无关；怪物猎人相关名称归 CAPCOM 所有。

## 功能

- **Overlay**：怪物状态，只显示你锁定的怪物（精确血量、部位、异常及触发次数、激怒、疲劳、可捕获、金冠、弱点）；玩家状态（按异常变色的体力、耐力、斩味、寒气炼成）；增益效果；伤害统计和 DPS 曲线；怪物列表；任务时间；武器状态；据点设施；性能数据。默认外观接近游戏原生 HUD，另有两套内置外观。透明、置顶、鼠标穿透；跟随游戏在不同缩放的显示器之间移动，画面不变时不重绘。
- **布局模式**：拖动移动，拖四角缩放，拖左右两边改宽度；吸附、撤销。每个游戏可以有多套方案，三档密度（完整、实战、极简），以及按住显示全部的按键。
- **记录**：每场狩猎存进本地 SQLite 数据库，包括 DPS 曲线、每个玩家的伤害和时间线，可导出为 CSV 或 JSON。图鉴从存档读取（只读）：金冠、研究和导航等级，以及最容易出金冠的任务。狩猎中 overlay 会显示每只怪的研究和导航等级，体型打破纪录时会标出来。
- **主题**：三层 XML 主题（令牌、组件样式、模块模板），保存即生效。附带 XSD、两个示例主题和命令行检查工具 `theme-check`。
- **插件**：WebAssembly 插件。权限在清单里声明，安装时由用户确认；插件卡住或出错只会停掉它自己。插件可以注册局内模块、快捷键动作、设置项、导出格式和通知渠道。
- **补丁和注入**（默认开启，可在设置里关闭）：完整性检查补丁和伤害 hook（会心统计、单人准确伤害）。两者都会修改游戏进程，风险自负。
- 界面语言：简体中文、繁体中文、日语、英语；游戏术语可以单独选语言。
- 六步首次启动引导、托盘图标、开机启动、检查更新（SHA-256 校验，失败可回退）、地址表单独更新、崩溃日志。

不收集任何数据。联网只用于在 GitHub 上检查更新；插件要联网需要用户授权。插件无法读写游戏内存。

## 安装

从 [Releases](https://github.com/yvettawx/RustyHunt/releases) 页面下载：

- `RustyHunt-<版本>-setup.exe`：安装版。安装到当前用户的目录（`%LOCALAPPDATA%\Programs\RustyHunt`），不需要管理员权限；数据放在 `%APPDATA%\RustyHunt\`。在 设置 > 应用 里卸载。
- `RustyHunt-<版本>-portable.zip`：便携版。解压即用；数据放在程序旁边的 `data\` 里。

需要 Windows 10 或更高版本。目前只支持《怪物猎人 世界：冰原》（Steam，游戏版本 421652、421740、421810）；崛起和荒野以后支持。游戏更新后，新版本的地址表会单独发布，不用重装程序。

程序没有代码签名，第一次运行时 Windows SmartScreen 会弹出警告。

## 文档

文档目前只有英文。

- [docs/theme.md](docs/theme.md)：编写主题；[docs/theme.xsd](docs/theme.xsd)：主题格式
- [docs/plugins.md](docs/plugins.md)：编写插件
- [docs/fields.md](docs/fields.md)：数据字段
- [docs/events.md](docs/events.md)：事件
- [docs/roadmap.md](docs/roadmap.md)：路线图和已知问题
- 示例：[examples/themes](examples/themes)、[examples/plugins/split-timer](examples/plugins/split-timer)

## 从源码构建

需要 Rust 1.88 或更高版本（MSVC 工具链）。

```
cargo build --release
```

工作区里的 crate：

| crate | 内容 |
|---|---|
| `address-map` | 地址表（`.map`）解析 |
| `platform` | Windows 进程、内存、窗口、快捷键、Steam 目录 |
| `core` | 数据快照、事件、设置、文本、模块描述 |
| `game-world` | 世界的内存读取、存档解密、演示数据 |
| `ui-kit` | 主题、字段表、模板引擎、绘制；`theme-check` 工具 |
| `plugin-host` | 插件清单和 WebAssembly 运行时 |
| `native` | 注入游戏的伤害 hook（`rustyhunt-native.dll`） |
| `rustyhunt` | 主程序：overlay、主窗口、托盘、更新 |
| `setup` | 安装版的安装和卸载程序 |

打包发布文件（便携版、安装版、SHA256SUMS）：

```
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

开发时可以不开游戏，用演示数据查看界面：

```
cargo run -p rustyhunt -- --demo
```
