"""Regenerate the FrameLatch ICO; requires Pillow. The SVG is its vector source."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SCALE = 4
im = Image.new("RGBA", (256 * SCALE, 256 * SCALE), (0, 0, 0, 0))
draw = ImageDraw.Draw(im)
def xy(values):
    return tuple(int(v * SCALE) for v in values)
def path(points):
    draw.line([xy(p) for p in points], fill="#f5f5f4", width=13 * SCALE, joint="curve")
    radius = 6.5 * SCALE
    for x, y in (points[0], points[-1]):
        x, y = x * SCALE, y * SCALE
        draw.ellipse((int(x-radius), int(y-radius), int(x+radius), int(y+radius)), fill="#f5f5f4")
draw.rounded_rectangle(xy((4, 4, 252, 252)), radius=52 * SCALE, fill="#1e1f22")
# A small corner radius is rasterized at 4x before the final icon sizes are generated.
path([(57, 98), (57, 70), (59, 64), (64, 59), (70, 57), (98, 57)])
path([(158, 57), (186, 57), (192, 59), (197, 64), (199, 70), (199, 98)])
path([(57, 158), (57, 186), (59, 192), (64, 197), (70, 199), (98, 199)])
path([(158, 199), (166, 199)])
path([(199, 158), (199, 166)])
draw.ellipse(xy((175, 175, 215, 215)), fill="#d54845")
im = im.resize((256, 256), Image.Resampling.LANCZOS)
im.save(ROOT / "assets" / "framelatch.ico", format="ICO", sizes=[(n, n) for n in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
