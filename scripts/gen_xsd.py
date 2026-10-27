# Generates docs/theme.xsd. Template elements and attributes follow the tables in crates/ui-kit/src/template.rs;
# rerun after changing them: python scripts/gen_xsd.py docs/theme.xsd
import sys

CONTAINERS = {
    "column": ("Children stacked vertically", ["gap", "align", "padding"]),
    "row": ("Children laid out horizontally", ["gap", "align", "padding"]),
    "stack": ("Children layered on top of each other", ["gap", "align", "padding"]),
    "for": ("Repeats the children for every item of a list, e.g. each=\"p in monster.parts\"", ["each", "limit", "gap", "direction", "align"]),
    "if": ("Draws the children only when the condition holds", ["when"]),
}
LEAVES = {
    "text": ("Text", ["text", "font", "size", "color", "align", "spacing", "shadow"]),
    "tag": ("Name tag: text on a filled block", ["text", "font", "size", "color", "fill", "shape", "cut", "radius", "padding"]),
    "bar": ("Bar; value is a ratio from 0 to 1", ["value", "ghost", "fill", "ghost-fill", "track", "kind", "shape", "segments", "gap", "empty", "caps", "skew", "radius", "edge", "marker", "marker-color"]),
    "arc": ("Arc or ring; from and to are angles", ["value", "radius", "thickness", "from", "to", "stroke", "track", "cap"]),
    "ring": ("Same as arc", ["value", "radius", "thickness", "from", "to", "stroke", "track", "cap"]),
    "rect": ("Rectangle, with optional rounded or cut corners and pattern fill", ["fill", "stroke", "stroke-width", "radius", "cut", "shape", "pattern", "density", "angle"]),
    "circle": ("Circle", ["radius", "fill", "stroke", "stroke-width"]),
    "polygon": ("Polygon, e.g. points=\"0,0 10,0 5,8\"", ["points", "fill", "stroke", "stroke-width"]),
    "path": ("Path in SVG path syntax; with value it fills along the path by that ratio", ["d", "fill", "stroke", "stroke-width", "value", "track"]),
    "image": ("PNG or SVG image; nine=\"left,top,right,bottom\" stretches it as a nine-slice", ["src", "nine", "tint"]),
    "pips": ("A row of pips", ["count", "filled", "size", "gap", "color"]),
    "line": ("Straight line", ["x1", "y1", "x2", "y2", "stroke", "stroke-width"]),
    "spacer": ("Empty space", []),
}
COMMON = ["opacity", "rotate", "scale", "skew", "flip", "glow", "visible", "x", "y", "width", "height"]
DOCS = {
    "gap": "Spacing between children",
    "align": "Alignment: start, center, end",
    "padding": "Padding",
    "each": "variable in list field",
    "limit": "Maximum number of items",
    "direction": "column or row",
    "when": "Condition expression, e.g. monster.enraged",
    "text": "Text; may contain {field | filter}",
    "font": "Font token name",
    "size": "Font size",
    "color": "Color or color token name",
    "value": "Ratio from 0 to 1",
    "fill": "Color or color token name",
    "shape": "Shape",
    "segments": "Number of segments; 0 means none",
    "empty": "Pattern of the empty track: none, hatch, dots, grid",
    "caps": "Small vertical caps at both ends",
    "visible": "Not drawn when the expression is false",
    "opacity": "0 to 1",
    "rotate": "Rotation angle",
    "scale": "Scale",
    "x": "Relative position",
    "y": "Relative position",
    "width": "Width",
    "height": "Height",
    "kind": "Which bar component style to use, e.g. monster-hp, part",
}
ROOT_ATTRS = [
    ("id", "Letters, digits, - and _ only; becomes the folder name on import", True),
    ("name", "Display name", True),
    ("version", "Theme version", False),
    ("author", "Author", False),
    ("description", "Summary", False),
    ("tags", "Tags, comma-separated", False),
    ("app", "Required app version, e.g. 1.0", False),
    ("preview", "Preview image file", False),
    ("base", "light or dark: which built-in theme fills in tokens left out", False),
]
TOKENS = [
    ("color", ["name", "value"], "value is #rgb, #rrggbb, #rrggbbaa or another color token name"),
    ("font", ["name", "family", "weight", "src"], "src is a font file in the theme package"),
    ("duration", ["name", "ms"], "Animation duration in milliseconds, e.g. fade, toast"),
    ("size", ["name", "value"], "Size"),
]
STYLES = {
    "bar": (["for", "shape", "skew", "radius", "height", "segments", "gap", "empty", "density", "angle", "caps", "edge", "glow"],
            "Bars. for changes only one kind of bar: monster-hp, player-hp, monster-stamina, stamina, part, part-flinch, ailment, damage, area, weapon, debuff; shape is rect, round, parallelogram"),
    "tag": (["shape", "cut", "radius"], "Name tags: shape is rect, cut, round"),
    "pip": (["shape"], "Pips: square, diamond, circle"),
    "switch": (["shape"], "Switches: square, round"),
    "panel": (["shape", "fill", "stroke", "padding", "cut", "radius"], "Module backing: none, rect, cut, round"),
    "text": (["shadow"], "Text shadow strength, 0 to 3"),
}
TOKENS_DOC = (
    "Tokens: colors, fonts, durations, sizes. Overlay colors: text, text-mute, text-faint, ink, accent, danger, cool, ok, "
    "ghost, heal, track, edge, shadow, hatch, crown-gold, crown-silver, qurio; main window colors start with window.: "
    "paper, ink, mute, faint, line, block, onblock, accent, desk"
)
MODULE_DOC = (
    "Module template with exactly one root element. id is monster, buffs, damage, area, player, clock, weapon, base, perf, "
    "or plugin ID.module ID for plugin modules"
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
w("<!-- RustyHunt theme format 1.x, documented in docs/theme.md. Generated by scripts/gen_xsd.py. -->")
w('<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns="urn:rustyhunt:theme:1"')
w('           targetNamespace="urn:rustyhunt:theme:1" elementFormDefault="qualified">')

w('  <xs:element name="theme">')
ann("Theme: manifest, tokens, component styles, module templates", "    ")
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
ann("Component styles: the look of each basic component", "    ")
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
ann("References another XML file in the theme package; every module in it is read", "    ")
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
ann("While when holds, these attributes apply and an animation can be added: blink, breathe, pulse, shift; period is in seconds", "    ")
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
