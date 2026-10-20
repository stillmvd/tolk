import io
import math
import pathlib
import subprocess
import tempfile

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
ICONS = PROJECT / "src-tauri/icons"
INKSCAPE = "C:/Program Files/Inkscape/bin/inkscape.com"

TOP = "#f3c6ff"
BOTTOM = "#c77de8"
TOP_ON_LIGHT = "#c77de8"
BOTTOM_ON_LIGHT = "#9a3fc2"
OFF_DARK = "#8a8a92"
OFF_LIGHT = "#6b6b73"

FULL = {"gap": 3.6, "t": 6, "r": 7}
SMALL = {"gap": 5, "t": 7.6, "r": 7}
ICO_SIZES = [24, 30, 32, 36, 40, 48, 60, 64, 72, 96, 128, 256]
APP_SHARE = 0.94


def arc(cx, cy, r, a0, a1, step=3):
    n = max(2, int(abs(a1 - a0) / step) + 1)
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / (n - 1))),
             cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / (n - 1)))) for i in range(n)]


def flaps(gap, t, r, w=40, bar=10, ty=10, stem=32):
    mid = w / 2
    cx = w / 2
    yb = mid - gap / 2
    yt = mid + gap / 2
    sx0, sx1 = cx - t / 2, cx + t / 2
    by0, by1 = ty - t / 2, ty + t / 2
    top = (arc(w - r, r, r, -90, 0) + arc(w - r, yb - r, r, 0, 90)
           + [(sx1, yb), (sx1, by1), (w - bar, by1), (w - bar, by0), (bar, by0), (bar, by1), (sx0, by1), (sx0, yb)]
           + arc(r, yb - r, r, 90, 180) + arc(r, r, r, 180, 270))
    bottom = (arc(w - r, yt + r, r, -90, 0) + arc(w - r, w - r, r, 0, 90) + arc(r, w - r, r, 90, 180)
              + arc(r, yt + r, r, 180, 270) + [(sx0, yt), (sx0, stem), (sx1, stem), (sx1, yt)])
    return top, bottom


def d(pts, k=1.0, o=0.0):
    return "M" + " L".join(f"{x * k + o:.3f} {y * k + o:.3f}" for x, y in pts) + " Z"


def svg(geom, size, share, top, bottom):
    k = size * share / 40
    o = size * (1 - share) / 2
    a, b = flaps(**geom)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">'
            f'<path d="{d(a, k, o)}" fill="{top}"/><path d="{d(b, k, o)}" fill="{bottom}"/></svg>')


def raster(text, px):
    with tempfile.TemporaryDirectory() as tmp:
        src = pathlib.Path(tmp) / "in.svg"
        out = pathlib.Path(tmp) / "out.png"
        src.write_text(text, encoding="utf-8")
        subprocess.run([INKSCAPE, str(src),
                        f"--actions=export-filename:{out};export-width:{px};export-height:{px};export-do"],
                       check=True, capture_output=True)
        return Image.open(io.BytesIO(out.read_bytes())).convert("RGBA")


def write_ico():
    frames = [raster(svg(SMALL if px <= 32 else FULL, 64, APP_SHARE, TOP, BOTTOM), px) for px in ICO_SIZES]
    frames[-1].save(ICONS / "icon.ico", format="ICO", sizes=[(px, px) for px in ICO_SIZES],
                    append_images=frames[:-1])
    frames[ICO_SIZES.index(128)].save(ROOT / "app-128.png")


def write_tray():
    for name, top, bottom in (("dark-on", TOP, BOTTOM), ("dark-off", OFF_DARK, OFF_DARK),
                              ("light-on", TOP_ON_LIGHT, BOTTOM_ON_LIGHT), ("light-off", OFF_LIGHT, OFF_LIGHT)):
        raster(svg(SMALL, 32, 1, top, bottom), 32).save(ICONS / "tray" / f"{name}.png")


def write_svgs():
    (ROOT / "mark.svg").write_text(svg(FULL, 64, APP_SHARE, TOP, BOTTOM), encoding="utf-8")
    (ROOT / "mark-small.svg").write_text(svg(SMALL, 64, APP_SHARE, TOP, BOTTOM), encoding="utf-8")
    (ROOT / "mark-mono.svg").write_text(
        svg(FULL, 64, 1, "currentColor", "currentColor"), encoding="utf-8")


def write_ui():
    a, b = flaps(**SMALL)
    k = 16 / 40
    (PROJECT / "src/mark.ts").write_text(
        f'export const MARK_TOP = "{d(a, k)}";\nexport const MARK_BOTTOM = "{d(b, k)}";\n', encoding="utf-8")


if __name__ == "__main__":
    write_svgs()
    write_ui()
    write_tray()
    write_ico()
    print("ok")
