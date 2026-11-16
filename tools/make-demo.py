#!/usr/bin/env python3
"""Turn a `ember --record DIR` capture into the website's motion-graphic GIFs.

    ember --record rec        # writes rec/frames/*.png and rec/marks.txt
    python tools/make-demo.py rec docs/img

Produces demo-tour.gif (camera moves, key-press overlays, captions, a macOS-style window on a
gradient wallpaper) and demo-themes.gif (soft diagonal wipes between all 12 themes).
Requires Pillow and numpy.
"""
import glob
import math
import os
import sys

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

REC = sys.argv[1] if len(sys.argv) > 1 else "rec"
OUT = sys.argv[2] if len(sys.argv) > 2 else "docs/img"
os.makedirs(OUT, exist_ok=True)

CW, CH = 940, 620          # canvas
WW = 840                    # window width at zoom 1
TB = 30                     # title bar height
FPS_MS = 66


def font(name, size):
    for base in ("C:/Windows/Fonts/", "/System/Library/Fonts/", "/usr/share/fonts/truetype/dejavu/"):
        p = base + name
        if os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


F_TITLE = font("segoeui.ttf", 13)
F_CAP = font("segoeuib.ttf", 30)
F_SUB = font("segoeui.ttf", 18)
F_KEY = font("seguisym.ttf", 24)
F_CHIP = font("segoeuib.ttf", 20)

frames_paths = sorted(glob.glob(os.path.join(REC, "frames", "f*.png")))
if not frames_paths:
    sys.exit("no frames found in " + REC)
marks = []
for line in open(os.path.join(REC, "marks.txt"), encoding="utf-8").read().splitlines():
    n, _, name = line.partition(" ")
    marks.append((int(n), name))
mk = {}
for n, name in marks:
    mk.setdefault(name, n)
theme_marks = [(n, name.split(":", 1)[1]) for n, name in marks if name.startswith("theme:") and name != "theme:end"]
N = len(frames_paths)
print("frames", N, "marks", len(marks))

_cache = {}
def load(i):
    i = max(0, min(N - 1, i))
    if i not in _cache:
        im = Image.open(frames_paths[i]).convert("RGB")
        h = round(WW * im.height / im.width)
        _cache[i] = im.resize((WW, h), Image.LANCZOS)
    return _cache[i]

APP_H = load(0).height
WIN_H = APP_H + TB
WX, WY = (CW - WW) // 2, (CH - WIN_H) // 2 + 6


# ---------------------------------------------------------------- wallpaper
def wallpaper():
    y, x = np.mgrid[0:CH, 0:CW].astype(np.float32)
    d = (x / CW * 0.55 + y / CH * 0.75)
    stops = [(0.0, (22, 14, 48)), (0.45, (74, 24, 70)), (0.75, (170, 52, 60)), (1.0, (255, 128, 58))]
    out = np.zeros((CH, CW, 3), np.float32)
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        m = (d >= a) & (d <= b)
        t = ((d - a) / (b - a))[m][:, None]
        out[m] = np.array(ca) * (1 - t) + np.array(cb) * t
    out[d > 1.0] = stops[-1][1]
    img = Image.fromarray(out.astype(np.uint8))
    glow = Image.new("RGB", (CW, CH), (0, 0, 0))
    g = ImageDraw.Draw(glow)
    g.ellipse((CW * 0.55, -CH * 0.25, CW * 1.2, CH * 0.5), fill=(120, 60, 160))
    g.ellipse((-CW * 0.2, CH * 0.55, CW * 0.45, CH * 1.2), fill=(255, 110, 50))
    glow = glow.filter(ImageFilter.GaussianBlur(120))
    return ImageChops.screen(img, glow.point(lambda v: int(v * 0.55)))


WALL = wallpaper()
RADIUS = 12


def rounded_mask(w, h, r):
    m = Image.new("L", (w * 2, h * 2), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, w * 2 - 1, h * 2 - 1), r * 2, fill=255)
    return m.resize((w, h), Image.LANCZOS)


def window_image(app):
    """App frame + macOS title bar, with rounded corners (RGBA, size WW x WIN_H)."""
    w = Image.new("RGB", (WW, WIN_H))
    bar = app.getpixel((3, 3))
    w.paste(app, (0, TB))
    d = ImageDraw.Draw(w)
    d.rectangle((0, 0, WW, TB), fill=bar)
    lum = sum(bar) / 3
    for i, c in enumerate(((255, 95, 87), (254, 188, 46), (40, 200, 64))):
        cx = 18 + i * 20
        d.ellipse((cx - 6, TB // 2 - 6, cx + 6, TB // 2 + 6), fill=c)
    txt = "main.rs — tasks"
    tw = d.textlength(txt, font=F_TITLE)
    d.text(((WW - tw) / 2, TB // 2 - 9), txt, font=F_TITLE, fill=(150, 150, 160) if lum < 128 else (90, 90, 100))
    d.line((0, TB, WW, TB), fill=tuple(max(0, c - 18) for c in bar))
    w = w.convert("RGBA")
    w.putalpha(rounded_mask(WW, WIN_H, RADIUS))
    return w


def smooth(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def compose(app, cam=(1.0, 0.5, 0.5), caption=None, keys=None, fade=1.0, chip=None):
    z, cx, cy = cam
    win = window_image(app)
    sw, sh = int(WW * z), int(WIN_H * z)
    big = win.resize((sw, sh), Image.BILINEAR)
    # place so that (cx, cy) of the window sits at the canvas centre, but never expose more than needed
    px = int(CW / 2 - cx * sw)
    py = int(CH / 2 - cy * sh) + (6 if z <= 1.0 else 0)
    canvas = WALL.copy().convert("RGBA")
    # shadow
    sh_layer = Image.new("RGBA", (CW, CH), (0, 0, 0, 0))
    ImageDraw.Draw(sh_layer).rounded_rectangle((px, py + 16, px + sw, py + sh + 16), int(RADIUS * z), fill=(0, 0, 0, 150))
    sh_layer = sh_layer.filter(ImageFilter.GaussianBlur(24))
    canvas.alpha_composite(sh_layer)
    canvas.alpha_composite(big, (px, py))
    ov = Image.new("RGBA", (CW, CH), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    if caption:
        title, sub = caption
        tw = d.textlength(title, font=F_CAP)
        bw = max(tw, d.textlength(sub or "", font=F_SUB)) + 44
        d.rounded_rectangle((28, CH - 108, 28 + bw, CH - 28), 18, fill=(14, 12, 20, 200), outline=(255, 255, 255, 30))
        d.text((50, CH - 100), title, font=F_CAP, fill=(255, 255, 255, 255))
        if sub:
            d.text((50, CH - 60), sub, font=F_SUB, fill=(255, 176, 120, 255))
    if keys:
        parts = keys
        widths = [d.textlength(p, font=F_KEY) + 26 for p in parts]
        total = sum(widths) + 10 * (len(parts) - 1)
        x = CW - total - 34
        for p, w in zip(parts, widths):
            d.rounded_rectangle((x, 34, x + w, 34 + 46), 12, fill=(240, 240, 245, 235), outline=(255, 255, 255, 255))
            d.text((x + 13, 40), p, font=F_KEY, fill=(30, 30, 38, 255))
            x += w + 10
    if chip:
        name, col = chip
        tw = d.textlength(name, font=F_CHIP)
        bw = tw + 64
        x0 = (CW - bw) / 2
        d.rounded_rectangle((x0, CH - 74, x0 + bw, CH - 26), 24, fill=(14, 12, 20, 215), outline=(255, 255, 255, 40))
        d.ellipse((x0 + 18, CH - 59, x0 + 36, CH - 41), fill=col + (255,))
        d.text((x0 + 46, CH - 66), name, font=F_CHIP, fill=(255, 255, 255, 255))
    canvas.alpha_composite(ov)
    out = canvas.convert("RGB")
    if fade < 1.0:
        out = Image.blend(Image.new("RGB", (CW, CH), (12, 10, 18)), out, fade)
    return out


def wipe(a, b, t, soft=0.35):
    """Soft diagonal wipe from image a to image b; t in 0..1."""
    A = np.asarray(a, np.float32)
    B = np.asarray(b, np.float32)
    h, w = A.shape[:2]
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    d = (x / w * 0.7 + y / h * 0.3)
    m = np.clip((t * (1 + soft) - d) / soft, 0, 1)[..., None]
    m = m * m * (3 - 2 * m)
    return Image.fromarray((A * (1 - m) + B * m).astype(np.uint8))


def accent_of(app):
    # pick the status-bar colour (accent) from the bottom-left of the frame
    return app.getpixel((WW // 2, APP_H - 3))[:3]


def save_gif(frames, name, ms=FPS_MS, colors=160):
    q = [f.quantize(colors=colors, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE) for f in frames]
    path = os.path.join(OUT, name)
    q[0].save(path, save_all=True, append_images=q[1:], duration=ms, loop=0, optimize=False, disposal=1)
    print(name, len(frames), "frames", round(os.path.getsize(path) / 1024 / 1024, 2), "MB")


# ---------------------------------------------------------------- tour
def seg(a, b):
    return list(range(mk.get(a, 0), mk.get(b, N)))


tour = []   # (kind, payload, segment)
for i in seg("start", "typing"):
    tour.append(("f", i, "start"))
for i in seg("typing", "save"):
    tour.append(("f", i, "typing"))
for i in seg("save", "palette"):
    tour.append(("f", i, "save"))
for i in seg("palette", theme_marks[0][1] and "theme:" + theme_marks[0][1]):
    tour.append(("f", i, "palette"))
# condensed theme tour: a handful of themes with soft wipes
pick = [theme_marks[k] for k in (0, 8, 4, 2, 10, 1) if k < len(theme_marks)]
for (a, an), (b, bn) in zip(pick, pick[1:] + [pick[0]]):
    for _ in range(5):
        tour.append(("f", a + 5, "themes"))
    for s in range(1, 7):
        tour.append(("w", (a + 5, b + 5, s / 6.0), "themes"))
for i in seg("terminal", "debug"):
    tour.append(("f", i, "terminal"))
for i in seg("debug", "end"):
    tour.append(("f", i, "debug"))
for i in range(mk.get("end", N - 1), N):
    tour.append(("f", i, "end"))

cams = {"start": (1.0, 0.5, 0.5), "typing": (1.2, 0.6, 0.52), "save": (1.16, 0.6, 0.45), "palette": (1.18, 0.6, 0.4),
        "themes": (1.0, 0.5, 0.5), "terminal": (1.22, 0.58, 0.66), "debug": (1.14, 0.5, 0.5), "end": (1.0, 0.5, 0.5)}
captions = {"typing": ("Write code", "syntax highlighting as you type"), "save": ("Save", "one keystroke, instantly"),
            "palette": ("Command palette", "every command, one search"), "themes": ("Twelve themes", "switch live, or write your own"),
            "terminal": ("A real terminal", "your shell, right in the editor"), "debug": ("Debugger", "breakpoints, stepping, variables")}
keymap = {"save": ["⌘", "S"], "palette": ["⇧", "⌘", "P"], "debug": ["F5"]}

out = []
seg_start, prev_cam, cur_cam, last_seg = {}, cams["start"], cams["start"], None
for p, (kind, payload, sg) in enumerate(tour):
    if sg != last_seg:
        prev_cam = cur_cam
        seg_start[sg] = p
        last_seg = sg
    t = smooth((p - seg_start[sg]) / 16.0)
    tgt = cams[sg]
    cur_cam = tuple(prev_cam[k] * (1 - t) + tgt[k] * t for k in range(3))
    if kind == "f":
        app = load(payload)
        t_in_seg = p - seg_start[sg]
    else:
        a, b, s = payload
        app = wipe(load(a), load(b), smooth(s))
        t_in_seg = p - seg_start[sg]
    keys = keymap.get(sg) if t_in_seg < 10 else None
    if sg == "debug" and 22 <= t_in_seg < 30:
        keys = ["F10"]
    cap = captions.get(sg) if t_in_seg >= 3 else None
    fade = min(1.0, (p + 1) / 8.0, (len(tour) - p) / 8.0)
    out.append(compose(app, cur_cam, cap, keys, fade))
save_gif(out, "demo-tour.gif")
Image.open(os.path.join(OUT, "demo-tour.gif")).convert("RGB").save(os.path.join(OUT, "demo-tour-poster.png"))

# ---------------------------------------------------------------- themes
HOLD, WIPE = 12, 9
th = []
chips = []
for n, name in theme_marks:
    app = load(n + 5)
    chips.append((app, name))
seq = []
for k, (app, name) in enumerate(chips):
    nxt = chips[(k + 1) % len(chips)]
    for _ in range(HOLD):
        seq.append((app, app, 0.0, name, nxt[1]))
    for s in range(1, WIPE + 1):
        seq.append((app, nxt[0], smooth(s / WIPE), name, nxt[1]))
tf = []
for app, nxt, t, name, nname in seq:
    img = wipe(app, nxt, t) if t > 0 else app
    label = nname if t > 0.5 else name
    tf.append(compose(img, (1.0, 0.5, 0.5), None, None, 1.0, (label, accent_of(nxt if t > 0.5 else app))))
save_gif(tf, "demo-themes.gif", ms=FPS_MS)
