# 生成 docs/theme.xsd。模板元素和属性照 crates/ui-kit/src/template.rs 里的表，
# 改了那边的表就重新跑一次：python scripts/gen_xsd.py docs/theme.xsd
import sys

CONTAINERS = {
    "column": ("竖排子元素", ["gap", "align", "padding"]),
    "row": ("横排子元素", ["gap", "align", "padding"]),
    "stack": ("子元素叠在一起", ["gap", "align", "padding"]),
    "for": ("对列表里的每一项重复子元素，例如 each=\"p in monster.parts\"", ["each", "limit", "gap", "direction", "align"]),
    "if": ("条件成立时才画子元素", ["when"]),
}
LEAVES = {
    "text": ("文字", ["text", "font", "size", "color", "align", "spacing", "shadow"]),
    "tag": ("名牌：底色块上的文字", ["text", "font", "size", "color", "fill", "shape", "cut", "radius", "padding"]),
    "bar": ("条，value 是 0 到 1 的比例", ["value", "ghost", "fill", "ghost-fill", "track", "kind", "shape", "segments", "gap", "empty", "caps", "skew", "radius", "edge", "marker", "marker-color"]),
    "arc": ("圆弧或圆环，from、to 是角度", ["value", "radius", "thickness", "from", "to", "stroke", "track", "cap"]),
    "ring": ("同 arc", ["value", "radius", "thickness", "from", "to", "stroke", "track", "cap"]),
    "rect": ("矩形，可以圆角、切角、图案填充", ["fill", "stroke", "stroke-width", "radius", "cut", "shape", "pattern", "density", "angle"]),
    "circle": ("圆", ["radius", "fill", "stroke", "stroke-width"]),
    "polygon": ("多边形，例如 points=\"0,0 10,0 5,8\"", ["points", "fill", "stroke", "stroke-width"]),
    "path": ("SVG path 语法的路径；带 value 时按比例沿路径填充", ["d", "fill", "stroke", "stroke-width", "value", "track"]),
    "image": ("PNG 或 SVG 图片；nine=\"左,上,右,下\" 按九宫格拉伸", ["src", "nine", "tint"]),
    "pips": ("一排小格", ["count", "filled", "size", "gap", "color"]),
    "line": ("直线", ["x1", "y1", "x2", "y2", "stroke", "stroke-width"]),
    "spacer": ("空白", []),
}
COMMON = ["opacity", "rotate", "scale", "skew", "flip", "glow", "visible", "x", "y", "width", "height"]
DOCS = {
    "gap": "子元素间距",
    "align": "对齐：start、center、end",
    "padding": "内边距",
    "each": "变量 in 列表字段",
    "limit": "最多几项",
    "direction": "column 或 row",
    "when": "条件表达式，例如 monster.enraged",
    "text": "文字，可以写 {字段 | 过滤器}",
    "font": "字体令牌名",
    "size": "字号",
    "color": "颜色或颜色令牌名",
    "value": "0 到 1 的比例",
    "fill": "颜色或颜色令牌名",
    "shape": "形状",
    "segments": "分几段，0 表示不分",
    "empty": "空槽图案：none、hatch、dots、grid",
    "caps": "两端加小竖线",
    "visible": "表达式为假时不画",
    "opacity": "0 到 1",
    "rotate": "旋转角度",
    "scale": "缩放",
    "x": "相对位置",
    "y": "相对位置",
    "width": "宽",
    "height": "高",
    "kind": "套用哪种条的组件样式，例如 monster-hp、part",
}
ROOT_ATTRS = [
    ("id", "只用字母、数字、- 和 _；导入后就是文件夹名", True),
    ("name", "显示的名字", True),
    ("version", "主题版本", False),
    ("author", "作者", False),
    ("description", "简介", False),
    ("tags", "标签，逗号分隔", False),
    ("app", "需要的程序版本，例如 1.0", False),
    ("preview", "预览图文件", False),
    ("base", "light 或 dark：没写的令牌按哪套内置主题补", False),
]
TOKENS = [
    ("color", ["name", "value"], "value 是 #rgb、#rrggbb、#rrggbbaa 或别的颜色令牌名"),
    ("font", ["name", "family", "weight", "src"], "src 是主题包里的字体文件"),
    ("duration", ["name", "ms"], "动画时长，毫秒，例如 fade、toast"),
    ("size", ["name", "value"], "尺寸"),
]
STYLES = {
    "bar": (["for", "shape", "skew", "radius", "height", "segments", "gap", "empty", "density", "angle", "caps", "edge", "glow"],
            "条。for 只改某一种条：monster-hp、player-hp、monster-stamina、stamina、part、part-flinch、ailment、damage、area、weapon、debuff；shape 是 rect、round、parallelogram"),
    "tag": (["shape", "cut", "radius"], "名牌：shape 是 rect、cut、round"),
    "pip": (["shape"], "小格：square、diamond、circle"),
    "switch": (["shape"], "开关：square、round"),
    "panel": (["shape", "fill", "stroke", "padding", "cut", "radius"], "模块底板：none、rect、cut、round"),
    "text": (["shadow"], "文字阴影强度，0 到 3"),
}
TOKENS_DOC = (
    "令牌：颜色、字体、时长、尺寸。overlay 的颜色：text、text-mute、text-faint、ink、accent、danger、cool、ok、"
    "ghost、heal、track、edge、shadow、hatch、crown-gold、crown-silver、qurio；主界面的颜色以 window. 开头："
    "paper、ink、mute、faint、line、block、onblock、accent、desk"
)
MODULE_DOC = (
    "模块模板，只能有一个根元素。id 是 monster、buffs、damage、area、player、clock、weapon、base、perf，"
    "或者插件模块的 插件编号.模块编号"
)

out = []
w = out.append


def ann(text, ind):
    w(f"{ind}<xs:annotation><xs:documentation>{text}</xs:documentation></xs:annotation>")


def attr(name, ind, required=False):
    use = ' use="required"' if required else ""
    if name in DOCS:
        w(f'{ind}<xs:attribute name="{name}" type="xs:string"{use}>')
        ann(DOCS[name], ind + "  ")
        w(f"{ind}</xs:attribute>")
    else:
        w(f'{ind}<xs:attribute name="{name}" type="xs:string"{use}/>')


w('<?xml version="1.0" encoding="utf-8"?>')
w("<!-- RustyHunt 主题格式 1.x，说明见 docs/theme.md。由 scripts/gen_xsd.py 生成。 -->")
w('<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns="urn:rustyhunt:theme:1"')
w('           targetNamespace="urn:rustyhunt:theme:1" elementFormDefault="qualified">')

w('  <xs:element name="theme">')
ann("主题：清单、令牌、组件样式、模块模板", "    ")
w("    <xs:complexType>")
w('      <xs:choice minOccurs="0" maxOccurs="unbounded">')
for r in ["tokens", "styles", "module", "include"]:
    w(f'        <xs:element ref="{r}"/>')
w("      </xs:choice>")
for name, doc, req in ROOT_ATTRS:
    w(f'      <xs:attribute name="{name}" type="xs:string"' + (' use="required">' if req else ">"))
    ann(doc, "        ")
    w("      </xs:attribute>")
w("    </xs:complexType>")
w("  </xs:element>")

w('  <xs:element name="tokens">')
ann(TOKENS_DOC, "    ")
w('    <xs:complexType><xs:choice minOccurs="0" maxOccurs="unbounded">')
for name, attrs, doc in TOKENS:
    w(f'      <xs:element name="{name}">')
    ann(doc, "        ")
    w("        <xs:complexType>")
    for a in attrs:
        w(f'          <xs:attribute name="{a}" type="xs:string"' + (' use="required"/>' if a == "name" else "/>"))
    w("        </xs:complexType>")
    w("      </xs:element>")
w("    </xs:choice></xs:complexType>")
w("  </xs:element>")

w('  <xs:element name="styles">')
ann("组件样式：每种基础组件的外形", "    ")
w('    <xs:complexType><xs:choice minOccurs="0" maxOccurs="unbounded">')
for name, (attrs, doc) in STYLES.items():
    w(f'      <xs:element name="{name}">')
    ann(doc, "        ")
    w("        <xs:complexType>")
    for a in attrs:
        w(f'          <xs:attribute name="{a}" type="xs:string"/>')
    w("        </xs:complexType>")
    w("      </xs:element>")
w("    </xs:choice></xs:complexType>")
w("  </xs:element>")

w('  <xs:element name="include">')
ann("引用主题包里的另一个 XML 文件，里面的 module 都会读进来", "    ")
w('    <xs:complexType><xs:attribute name="src" type="xs:string" use="required"/></xs:complexType>')
w("  </xs:element>")

w('  <xs:element name="module">')
ann(MODULE_DOC, "    ")
w('    <xs:complexType><xs:group ref="node"/><xs:attribute name="id" type="xs:string" use="required"/></xs:complexType>')
w("  </xs:element>")

w('  <xs:group name="node"><xs:choice>')
for name in list(CONTAINERS) + list(LEAVES):
    w(f'    <xs:element ref="{name}"/>')
w("  </xs:choice></xs:group>")

w('  <xs:element name="state">')
ann("满足 when 时改用这里写的属性，可以加动画：blink、breathe、pulse、shift；period 是周期，秒", "    ")
w("    <xs:complexType>")
attr("when", "      ", True)
w('      <xs:attribute name="animate" type="xs:string"/>')
w('      <xs:attribute name="period" type="xs:string"/>')
w('      <xs:anyAttribute processContents="skip"/>')
w("    </xs:complexType>")
w("  </xs:element>")

for name, (doc, attrs) in CONTAINERS.items():
    w(f'  <xs:element name="{name}">')
    ann(doc, "    ")
    w("    <xs:complexType>")
    w("      <xs:sequence>")
    w('        <xs:element ref="state" minOccurs="0" maxOccurs="unbounded"/>')
    w('        <xs:group ref="node" minOccurs="0" maxOccurs="unbounded"/>')
    w("      </xs:sequence>")
    for a in attrs + COMMON:
        attr(a, "      ", (name, a) in (("if", "when"), ("for", "each")))
    w("    </xs:complexType>")
    w("  </xs:element>")

for name, (doc, attrs) in LEAVES.items():
    w(f'  <xs:element name="{name}">')
    ann(doc, "    ")
    w("    <xs:complexType>")
    w('      <xs:sequence><xs:element ref="state" minOccurs="0" maxOccurs="unbounded"/></xs:sequence>')
    for a in attrs + [c for c in COMMON if c not in attrs]:
        attr(a, "      ")
    w("    </xs:complexType>")
    w("  </xs:element>")

w("</xs:schema>")

with open(sys.argv[1], "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(out) + "\n")
