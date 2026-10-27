# Generates assets/icon.ico: the same mark window::icon_rgba draws in the app (a dark square with an orange diamond),
# drawn at 4x and scaled down for antialiasing.python scripts/gen_icon.py assets/icon.ico
import sys
from PIL import Image, ImageDraw

def mark(size):
    s = size * 4
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    inset = s * 0.06
    d.rectangle([inset, inset, s - inset, s - inset], fill=(0x14, 0x14, 0x14, 255))
    c, r = s / 2, s * 0.22
    d.polygon([(c, c - r), (c + r, c), (c, c + r), (c - r, c)], fill=(0xFF, 0x6B, 0x1A, 255))
    return im.resize((size, size), Image.LANCZOS)

sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
mark(256).save(sys.argv[1], sizes=[(n, n) for n in sizes])
