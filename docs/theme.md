# Writing themes

A theme decides how the overlay and the main window look: colors, fonts, bar shapes, all the way to the structure of whole modules. Themes are data only, with no scripts; extensions that need code are plugins (see `plugins.md`).

This directory has:

- `theme.xsd`: the format's schema. In VS Code with the XML extension (Red Hat), add `xsi:schemaLocation` at the top of the theme file as in the examples to get completion, hover docs and live errors.
- `fields.md`: the data fields templates can use.
- `examples/themes/` in the repository has two example themes: Ember (changes tokens, component styles and one module template) and Frost (tokens and component styles only).

## Theme package

A folder with a `theme.xml` plus the images (PNG, SVG) and fonts it uses. Long templates can be split into several XML files referenced with `<include src="..."/>`.

- The whole package is at most 20 MB, images at most 4096 pixels per side, and a single template at most 512 nodes.
- "Import" on the Themes page reads a zip or a folder and installs it into `themes/<id>/` in the data directory.
- The theme in use reloads as soon as a file is saved, so you see changes as you edit. Errors show the file and line on the Log and Themes pages; a broken template falls back to the default look and other modules are unaffected.

Command-line check (format, field references, image sizes, package size):

```
theme-check theme-folder
```

## Three layers

Anything a layer leaves out inherits from the layer above or the built-in defaults. A theme that only changes colors needs nothing but `<tokens>`.

```xml
<theme xmlns="urn:rustyhunt:theme:1"
       id="ember" name="Ember" version="1.0.0" author="Your name" app="1.0" base="dark">
  <tokens> ... </tokens>
  <styles> ... </styles>
  <module id="clock"> ... </module>
</theme>
```

Attributes of `<theme>`:

| Attribute | Description |
|---|---|
| `id` | Letters, digits, `-` and `_` only; becomes the folder name on import |
| `name`, `version`, `author`, `description`, `tags` | Manifest; `tags` is comma-separated |
| `app` | Required app version, e.g. `1.0`; with an older app the theme is disabled and the reason shown |
| `base` | `light` or `dark`: which built-in theme fills in tokens left out; default `dark` |
| `preview` | Preview image file |

### 1. Tokens

```xml
<tokens>
  <color name="accent" value="#ffb347"/>
  <color name="ghost" value="accent"/>       <!-- may refer to another color token -->
  <font name="number" family="Montserrat" weight="800"/>
  <font name="title" family="My Font" src="fonts/my.ttf"/>
  <duration name="fade" ms="400"/>
  <size name="gap" value="6"/>
</tokens>
```

Colors are `#rgb`, `#rrggbb` or `#rrggbbaa`.

Colors used by the overlay:

| Token | Used for |
|---|---|
| `text`, `text-mute`, `text-faint` | The three text levels |
| `ink` | Dark blocks such as name tag backgrounds |
| `accent` | Emphasis: rage, your own damage, break callouts |
| `danger` | Low HP, almost over |
| `cool`, `ok`, `heal` | Cool hints, normal, healing |
| `ghost` | HP trail |
| `track`, `edge`, `shadow`, `hatch` | Bar track, outline, text shadow, hatch pattern |
| `crown-gold`, `crown-silver` | Crowns |
| `qurio` | Qurio |

The main window uses nine colors starting with `window.`: `paper` (background), `ink` (text), `mute`, `faint`, `line` (dividers), `block`, `onblock` (primary buttons and name tags), `accent`, `desk` (the dark area around previews).

Font tokens: `number`, `number-heavy`, `number-bold`, `text`, `text-bold`. Duration tokens: `fade` (fade in and out), `toast` (how long callouts stay).

### 2. Component styles

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

| Element | Attributes |
|---|---|
| `bar` | `shape` (rect, round, parallelogram), `skew`, `radius`, `height`, `segments`, `gap`, `empty` (none, hatch, dots, grid), `density`, `angle`, `caps`, `edge`, `glow`; `for` changes only one kind of bar: `monster-hp`, `player-hp`, `monster-stamina`, `stamina`, `part`, `part-flinch`, `ailment`, `damage`, `area`, `weapon`, `debuff` |
| `tag` | `shape` (rect, cut, round), `cut`, `radius` |
| `pip` | `shape` (square, diamond, circle) |
| `switch` | `shape` (square, round) |
| `panel` | `shape` (none, rect, cut, round), `fill`, `stroke`, `padding`, `cut`, `radius` |
| `text` | `shadow`: text shadow strength, 0 to 3 |

### 3. Module templates

Rewrites the whole structure of a module. Module IDs: `monster`, `buffs`, `damage`, `area`, `player`, `clock`, `weapon`, `base`, `perf`; plugin modules are `plugin ID.module ID`. Each `<module>` has exactly one root element.

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

#### Elements

| Element | Purpose | Its own attributes |
|---|---|---|
| `column`, `row`, `stack` | Vertical, horizontal, layered | `gap`, `align`, `padding` |
| `for` | Repeats once per list item | `each="p in monster.parts"`, `limit`, `gap`, `direction`, `align` |
| `if` | Drawn only when the condition holds | `when` |
| `text` | Text | `text`, `font`, `size`, `color`, `align`, `spacing`, `shadow` |
| `tag` | Name tag | `text`, `font`, `size`, `color`, `fill`, `shape`, `cut`, `radius`, `padding` |
| `bar` | Bar | `value`, `ghost`, `fill`, `ghost-fill`, `track`, `kind` (which bar style to use), plus the bar attributes from component styles, `marker`, `marker-color` |
| `arc`, `ring` | Arc, ring | `value`, `radius`, `thickness`, `from`, `to`, `stroke`, `track`, `cap` |
| `rect` | Rectangle | `fill`, `stroke`, `stroke-width`, `radius`, `cut`, `shape`, `pattern`, `density`, `angle` |
| `circle` | Circle | `radius`, `fill`, `stroke`, `stroke-width` |
| `polygon` | Polygon | `points="0,0 10,0 5,8"`, `fill`, `stroke`, `stroke-width` |
| `path` | Path in SVG path syntax; with `value` it fills along the path by that ratio, for curved bars | `d`, `fill`, `stroke`, `stroke-width`, `value`, `track` |
| `image` | PNG or SVG | `src`, `nine="left,top,right,bottom"` (nine-slice stretching), `tint` |
| `pips` | A row of pips | `count`, `filled`, `size`, `gap`, `color` |
| `line` | Straight line | `x1`, `y1`, `x2`, `y2`, `stroke`, `stroke-width` |
| `spacer` | Empty space | |

Every element accepts: `opacity`, `rotate`, `scale`, `skew`, `flip`, `glow`, `visible`, `x`, `y`, `width`, `height`.

Sizes are pixels on a 1080-high frame; the overlay scales them to the actual resolution.

#### Data and expressions

Write expressions inside `{...}` in attribute values; the rest is literal: `text="{monster.hp_ratio | percent}"`, `text="Left {quest.time_left | time}"`. `when` is an expression already and needs no braces.

- Fields: see `fields.md`. Iterate lists with `for`, or use an index: `monster.parts.0.name`.
- Operators: `+ - * / %`, comparisons `== != < <= > >=`, logic `&& || !`. `<` and `&` must be escaped in XML attributes, so there are word forms too: `lt le gt ge and or not`. Division by 0 gives 0.
- Filters: `percent` (0.63 → 63%), `int`, `fixed:2`, `thousands` (thousands separators), `time` (seconds → m:ss), `upper`, `lower`, `default:'—'`, `abs`, `clamp01`.
- Color attributes take a token name, or an expression that evaluates to a token name or a color value.
- Element switches from the Modules page: `module.show.<element>`, e.g. `<if when="module.show.m-weak">`. Element names are on the Modules page; the element keys of built-in modules are in the `[element]` section of `locales/ui/zh-Hans.toml`.
- Data sent by plugin modules is under `data`; its fields are up to the plugin and aren't checked.

Loop expressions and custom functions aren't supported.

#### States and animation

```xml
<bar value="{monster.hp_ratio}" width="820">
  <state when="monster.enraged" fill="accent" animate="pulse"/>
  <state when="monster.capturable" fill="ok"/>
</bar>
```

`<state>` goes inside an element; while `when` holds, its attributes override the element's, and later states win. `animate` is `blink`, `breathe`, `pulse` or `shift`, and `period` is the period in seconds. A template has at most 24 animations.

## Versioning

The theme format version is in the namespace (`urn:rustyhunt:theme:1`). Minor versions only add, never remove; anything to be removed is marked deprecated one major version ahead. At startup the app checks the app version each theme needs, disables incompatible ones and shows why on the Themes page.
