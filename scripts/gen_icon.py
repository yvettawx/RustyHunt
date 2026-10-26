# 生成 assets/icon.ico：和程序里 window::icon_rgba 画的是同一个标记（深色方块加橙色菱形），
# 先画 4 倍大再缩小做抗锯齿。python scripts/gen_icon.py assets/icon.ico
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
