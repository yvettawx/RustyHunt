# 写主题

主题决定 overlay 和主界面长什么样：颜色、字体、血条的形状，直到整个模块的结构。主题只含数据，不含脚本；需要写代码的扩展走插件（见 `plugins.md`）。

本目录有：

- `theme.xsd`：格式的 schema。VS Code 装上 XML 插件（Red Hat），在主题文件开头照示例写上 `xsi:schemaLocation`，就能自动补全、悬停看说明、即时报错。
- `fields.md`：模板里能用的数据字段。
- 仓库的 `examples/themes/` 里有两个示例主题：熔岩（改了令牌、组件样式和一个模块模板）、冰原（只改令牌和组件样式）。

## 主题包

一个文件夹，里面一个 `theme.xml`，加上用到的图片（PNG、SVG）和字体。模板太长可以拆成多个 XML 文件，用 `<include src="..."/>` 引用。

- 整个包不超过 20 MB，图片单边不超过 4096 像素，单个模板不超过 512 个节点。
- 主题页的"导入"读 zip 或文件夹，装到数据目录的 `themes/<id>/`。
- 正在用的主题，文件一保存就重新载入，边改边看。出错时在日志页和主题页写明文件和行号；出错的模板退回默认样式，其他模块不受影响。

命令行检查（格式、字段引用、图片尺寸、包大小）：

```
theme-check 主题文件夹
```

## 三层

下层没写的部分继承上层或内置默认值。只想换颜色的主题，写一个 `<tokens>` 就够了。

```xml
<theme xmlns="urn:rustyhunt:theme:1"
       id="ember" name="熔岩" version="1.0.0" author="你的名字" app="1.0" base="dark">
  <tokens> ... </tokens>
  <styles> ... </styles>
  <module id="clock"> ... </module>
</theme>
```

`<theme>` 的属性：

| 属性 | 说明 |
|---|---|
| `id` | 只用字母、数字、`-`、`_`；导入后就是文件夹名 |
| `name`、`version`、`author`、`description`、`tags` | 清单；`tags` 用逗号分隔 |
| `app` | 需要的程序版本，例如 `1.0`；程序版本更低时主题停用并写明原因 |
| `base` | `light` 或 `dark`：没写的令牌按哪套内置主题补，默认 `dark` |
| `preview` | 预览图文件 |

### 1. 令牌

```xml
<tokens>
  <color name="accent" value="#ffb347"/>
  <color name="ghost" value="accent"/>       <!-- 可以引用别的颜色令牌 -->
  <font name="number" family="Montserrat" weight="800"/>
  <font name="title" family="My Font" src="fonts/my.ttf"/>
  <duration name="fade" ms="400"/>
  <size name="gap" value="6"/>
</tokens>
```

颜色写 `#rgb`、`#rrggbb` 或 `#rrggbbaa`。

overlay 用到的颜色：

| 令牌 | 用在哪 |
|---|---|
| `text`、`text-mute`、`text-faint` | 文字的三档 |
| `ink` | 名牌底色等深色块 |
| `accent` | 强调：激怒、自己的伤害、破坏播报 |
| `danger` | 低血、快结束 |
| `cool`、`ok`、`heal` | 冷色提示、正常、回复 |
| `ghost` | 血量残影 |
| `track`、`edge`、`shadow`、`hatch` | 条的底槽、描边、文字阴影、斜线图案 |
| `crown-gold`、`crown-silver` | 皇冠 |
| `qurio` | 傀异化 |

主界面用 `window.` 开头的九个颜色：`paper`（底色）、`ink`（文字）、`mute`、`faint`、`line`（分隔线）、`block`、`onblock`（主按钮和名牌）、`accent`、`desk`（预览框外的深色底）。

字体令牌：`number`、`number-heavy`、`number-bold`、`text`、`text-bold`。时长令牌：`fade`（淡入淡出）、`toast`（播报停留）。

### 2. 组件样式

```xml
<styles>
  <bar shape="parallelogram" skew="4" empty="hatch"/>
  <bar for="monster-hp" segments="10" gap="2" caps="true"/>
  <tag shape="cut" cut="4"/>
  <pip shape="diamond"/>
  <switch shape="round"/>
  <panel shape="round" radius="6" fill="#0d151c99"/>
  <text shadow="1.5"/>
</styles>
```

| 元素 | 属性 |
|---|---|
| `bar` | `shape`（rect、round、parallelogram）、`skew`、`radius`、`height`、`segments`、`gap`、`empty`（none、hatch、dots、grid）、`density`、`angle`、`caps`、`edge`、`glow`；`for` 只改某一种条：`monster-hp`、`player-hp`、`monster-stamina`、`stamina`、`part`、`part-flinch`、`ailment`、`damage`、`area`、`weapon`、`debuff` |
| `tag` | `shape`（rect、cut、round）、`cut`、`radius` |
| `pip` | `shape`（square、diamond、circle） |
| `switch` | `shape`（square、round） |
| `panel` | `shape`（none、rect、cut、round）、`fill`、`stroke`、`padding`、`cut`、`radius` |
| `text` | `shadow`：文字阴影强度，0 到 3 |

### 3. 模块模板

给某个模块重写整个结构。模块编号：`monster`、`buffs`、`damage`、`area`、`player`、`clock`、`weapon`、`base`、`perf`，插件模块是 `插件编号.模块编号`。每个 `<module>` 里只能有一个根元素。

```xml
<module id="clock">
  <row gap="12" align="center">
    <arc value="{quest.time_left / quest.time_max | clamp01}" radius="18" thickness="4"
         from="0" to="360" stroke="accent" track="track">
      <state when="quest.time_left lt 300" stroke="danger" animate="pulse"/>
    </arc>
    <column gap="0">
      <text text="{quest.time_left | time}" font="number" size="26"/>
      <text text="{game.stage}" size="12" color="text-mute"/>
    </column>
  </row>
</module>
```

#### 元素

| 元素 | 用途 | 自己的属性 |
|---|---|---|
| `column`、`row`、`stack` | 竖排、横排、叠放 | `gap`、`align`、`padding` |
| `for` | 列表的每一项重复一次 | `each="p in monster.parts"`、`limit`、`gap`、`direction`、`align` |
| `if` | 条件成立才画 | `when` |
| `text` | 文字 | `text`、`font`、`size`、`color`、`align`、`spacing`、`shadow` |
| `tag` | 名牌 | `text`、`font`、`size`、`color`、`fill`、`shape`、`cut`、`radius`、`padding` |
| `bar` | 条 | `value`、`ghost`、`fill`、`ghost-fill`、`track`、`kind`（套用哪种条的样式），以及组件样式里条的属性、`marker`、`marker-color` |
| `arc`、`ring` | 圆弧、圆环 | `value`、`radius`、`thickness`、`from`、`to`、`stroke`、`track`、`cap` |
| `rect` | 矩形 | `fill`、`stroke`、`stroke-width`、`radius`、`cut`、`shape`、`pattern`、`density`、`angle` |
| `circle` | 圆 | `radius`、`fill`、`stroke`、`stroke-width` |
| `polygon` | 多边形 | `points="0,0 10,0 5,8"`、`fill`、`stroke`、`stroke-width` |
| `path` | SVG path 语法的路径；带 `value` 时沿路径按比例填充，可以做弯曲的血条 | `d`、`fill`、`stroke`、`stroke-width`、`value`、`track` |
| `image` | PNG 或 SVG | `src`、`nine="左,上,右,下"`（九宫格拉伸）、`tint` |
| `pips` | 一排小格 | `count`、`filled`、`size`、`gap`、`color` |
| `line` | 直线 | `x1`、`y1`、`x2`、`y2`、`stroke`、`stroke-width` |
| `spacer` | 空白 | |

所有元素都能用：`opacity`、`rotate`、`scale`、`skew`、`flip`、`glow`、`visible`、`x`、`y`、`width`、`height`。

尺寸单位是 1080 高画面里的像素，overlay 按实际分辨率缩放。

#### 数据和表达式

属性值里用 `{...}` 写表达式，其余是字面值：`text="{monster.hp_ratio | percent}"`、`text="剩余 {quest.time_left | time}"`。`when` 本身就是表达式，不用加花括号。

- 字段：见 `fields.md`。列表用 `for` 遍历，或者写下标：`monster.parts.0.name`。
- 运算：`+ - * / %`，比较 `== != < <= > >=`，逻辑 `&& || !`。XML 属性里 `<` 和 `&` 要转义，所以另有单词写法：`lt le gt ge and or not`。除以 0 得 0。
- 过滤器：`percent`（0.63 → 63%）、`int`、`fixed:2`、`thousands`（千分位）、`time`（秒 → m:ss）、`upper`、`lower`、`default:'—'`、`abs`、`clamp01`。
- 颜色属性可以写令牌名，也可以写表达式算出令牌名或颜色值。
- 模块页的元素开关：`module.show.<元素>`，例如 `<if when="module.show.m-weak">`。元素名见模块页，内置模块的元素键见 `locales/ui/zh-Hans.toml` 的 `[element]` 一节。
- 插件模块交来的数据在 `data` 下面，字段由插件决定，不做检查。

不支持循环表达式和自定义函数。

#### 状态和动画

```xml
<bar value="{monster.hp_ratio}" width="820">
  <state when="monster.enraged" fill="accent" animate="pulse"/>
  <state when="monster.capturable" fill="ok"/>
</bar>
```

`<state>` 写在元素里面，`when` 成立时用它的属性覆盖元素的属性；后写的优先。`animate` 是 `blink`（闪烁）、`breathe`（呼吸）、`pulse`（脉冲）、`shift`（位移），`period` 是周期（秒）。一个模板最多 24 个动画。

## 版本

主题格式的版本写在命名空间里（`urn:rustyhunt:theme:1`）。小版本只加不删；要删的东西提前一个大版本标记废弃。程序启动时检查每个主题需要的程序版本，不兼容的停用，并在主题页写明原因。
