#!/usr/bin/env python3
"""Free Pack menu art, in the look Michael picked (taped stickers on a
halftone paper strip, worn chrome logo, muted lime accents).

Writes build/ui/<DAT stem>/<image path inside the DAT>.png (backslashes as
slashes), drawn at twice the stock size; the viewer scales each image to
its control. Screens whose buttons outgrow the stock spots also get
build/ui/<DAT stem>.toml with [controls.<Name>] rect = [x, y, w, h] in
640x480 units.

Usage: make_ui.py BACKGROUND_PHOTO.png [LOGO.png]
"""
import os, sys, math
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageOps
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import mock_main as M
from mock_main import font, icon, tape, INK, BONE, OLIVE, LIME

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(HERE, "build", "ui")
X = 2  # drawn at twice the stock size
STATES = ("Up", "Lit", "Down", "Grey")
RUST_SOFT = (150, 74, 46)
BOLD = "barlowcondensed/BarlowCondensed-Bold.ttf"
MED = "barlowcondensed/BarlowCondensed-Medium.ttf"
written = []

def save(dat, path, im):
    p = os.path.join(OUT, dat, *path.split("\\")) .rsplit(".", 1)[0] + ".png"
    os.makedirs(os.path.dirname(p), exist_ok=True); im.save(p); written.append(p)

def fit_font(text, w, h, rel=BOLD, start=None):
    size = start or int(h * 0.8)
    while size > 6:
        f = font(rel, size); bb = f.getbbox(text)
        if bb[2] - bb[0] <= w and bb[3] - bb[1] <= h: return f
        size -= 1
    return font(rel, 6)

# ---------------------------------------------------------------- stickers

def plate(w, h, bg, seed):
    """A flat sticker: colour, a little paper tooth and a few scuffs."""
    r = np.random.default_rng(seed)
    a = np.zeros((h, w, 4), np.float32); a[..., :3] = np.array(bg[:3]) / 255; a[..., 3] = 1
    a[..., :3] *= (0.97 + 0.05 * r.random((h, w)))[..., None]
    sc = r.random((h, w)) < 0.01; a[sc, :3] = np.clip(a[sc, :3] + 0.22, 0, 1)
    return Image.fromarray((a * 255).astype(np.uint8))

def label(im, text, fg, ico=None, sub=None, align="left"):
    w, h = im.size; d = ImageDraw.Draw(im); pad = int(h * 0.12)
    subh = int(h * 0.2) if sub else 0
    isz = int((h - subh) * 0.66) if ico else 0
    if ico: im.alpha_composite(icon(ico, isz, fg), (pad, (h - subh - isz) // 2))
    tx0 = pad + isz + (pad if ico else 0)
    f = fit_font(text, w - tx0 - pad, int((h - subh) * 0.72))
    bb = d.textbbox((0, 0), text, font=f); tw, th = bb[2] - bb[0], bb[3] - bb[1]
    tx = tx0 if (ico or align == "left") else (w - tw) / 2
    d.text((tx - bb[0], (h - subh - th) / 2 - bb[1]), text, font=f, fill=fg)
    if sub:
        sf = font(MED, max(8, int(subh * 0.62)))
        d.line([pad * 0.7, h - subh - 2, w - pad * 0.7, h - subh - 2], fill=fg, width=max(1, h // 60))
        bb = d.textbbox((0, 0), sub, font=sf)
        d.text((pad * 0.7, h - subh + (subh - (bb[3] - bb[1])) / 2 - bb[1] - 1), sub, font=sf, fill=fg)
    return im

def finish(face, W, H, rot, state):
    """Rotate, shadow and fit a sticker face into a W x H canvas, per state."""
    face = face.copy()
    if state == "Grey":
        a = np.asarray(face).astype(np.float32); g = a[..., :3] @ [0.3, 0.59, 0.11]
        a[..., :3] = g[..., None] * 0.6 + 0.4 * 200; a[..., 3] *= 0.55
        face = Image.fromarray(a.astype(np.uint8))
    if state == "Down":
        a = np.asarray(face).astype(np.float32); a[..., :3] *= 0.86; face = Image.fromarray(a.astype(np.uint8))
    if state == "Lit":
        d = ImageDraw.Draw(face); lw = max(2, face.size[1] // 28)
        d.rectangle([lw // 2, lw // 2, face.size[0] - lw // 2 - 1, face.size[1] - lw // 2 - 1], outline=LIME + (255,), width=lw)
    sh_off = {"Up": 6, "Lit": 9, "Down": 2, "Grey": 3}[state]
    fr = face.rotate(rot, Image.BICUBIC, expand=True)
    sh = Image.new("RGBA", fr.size, (0, 0, 0, 0)); sh.paste((0, 0, 0, 110), (0, 0), fr.split()[3]); sh = sh.filter(ImageFilter.GaussianBlur(3))
    out = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ox, oy = (W - fr.size[0]) // 2, (H - fr.size[1]) // 2
    if state == "Down": ox += 2; oy += 2
    if state == "Lit": ox -= 1; oy -= 2
    out.alpha_composite(sh, (ox + sh_off, oy + sh_off)); out.alpha_composite(fr, (ox, oy))
    if state == "Lit":
        t = tape(int(W * 0.16), max(8, int(H * 0.16)), col=(200, 214, 150)).rotate(-35, expand=True)
        out.alpha_composite(t, (W - t.size[0], 0))
    return out

def sticker_set(dat, base, names, W, H, text, bg, fg, ico=None, sub=None, rot=0.0, seed=0, align="left"):
    """names: state -> image path (or a format string with {s})."""
    margin = 0.84  # leave room for rotation, shadow and tape
    fw, fh = int(W * margin), int(H * 0.8)
    face = label(plate(fw, fh, bg, seed), text, fg, ico, sub, align)
    for s in STATES:
        path = names.get(s) if isinstance(names, dict) else names.format(s=s)
        if path: save(dat, path, finish(face, W, H, rot, s))

# ---------------------------------------------------------------- panels

def board(w, h, tone=INK, seed=1, edge=None):
    """A dark clipboard panel with a fine grain and a lime top rule."""
    r = np.random.default_rng(seed)
    a = np.zeros((h, w, 4), np.float32); a[..., :3] = np.array(tone) / 255; a[..., 3] = 1
    n = np.asarray(Image.fromarray((r.random((h, w)) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.8)), np.float32) / 255
    a[..., :3] *= (0.9 + 0.25 * n)[..., None]
    im = Image.fromarray((a * 255).astype(np.uint8))
    if edge:
        ImageDraw.Draw(im).rectangle([0, 0, w - 1, h - 1], outline=edge + (255,), width=max(2, w // 200))
    return im

def inset(im, box, fill=(14, 13, 12), line=(70, 66, 58)):
    d = ImageDraw.Draw(im); x, y, w, h = [v * X for v in box]
    d.rectangle([x, y, x + w - 1, y + h - 1], fill=fill + (255,), outline=line + (255,), width=X)

def spine(im, text, w=70):
    """An olive strip down the left of a panel with a big vertical label."""
    H = im.size[1]; s = Image.new("RGBA", (H, w * X), OLIVE + (255,)); d = ImageDraw.Draw(s)
    f = fit_font(text, H - 40 * X, int(w * X * 0.62)); bb = d.textbbox((0, 0), text, font=f)
    d.text((H - (bb[2] - bb[0]) - 24 * X - bb[0], (w * X - (bb[3] - bb[1])) / 2 - bb[1]), text, font=f, fill=BONE)
    im.alpha_composite(s.rotate(90, expand=True), (0, 0))

def header(im, text, x, y, w, h):
    d = ImageDraw.Draw(im); x, y, w, h = x * X, y * X, w * X, h * X
    d.rectangle([x, y, x + w, y + h], fill=LIME + (255,))
    f = fit_font(text, w - 12 * X, int(h * 0.8)); bb = d.textbbox((0, 0), text, font=f)
    d.text((x + 6 * X - bb[0], y + (h - (bb[3] - bb[1])) / 2 - bb[1]), text, font=f, fill=INK)

def taped(im, seed=0):
    r = np.random.default_rng(seed); w, h = im.size
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0)); out.alpha_composite(im)
    for (tx, ty, rot) in ((-0.02, -0.01, 30), (0.88, -0.01, -28)):
        t = tape(int(w * 0.16), max(12, int(h * 0.07))).rotate(rot + r.uniform(-5, 5), expand=True)
        out.alpha_composite(t, (int(w * tx), int(h * ty)))
    return out

def unroll(full, n=15):
    """Pull-down frames: the panel unrolls from the top, settling with a small bounce."""
    w, h = full.size; frames = []
    for k in range(1, n + 1):
        t = k / n; e = 1 - (1 - t) ** 3
        if k == n - 2: e = 1.03
        if k == n - 1: e = 0.99
        vis = int(h * min(e, 1.0))
        f = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        if vis > 0: f.alpha_composite(full.crop((0, 0, w, vis)), (0, 0))
        frames.append(f)
    return frames

# ---------------------------------------------------------------- small widgets

def glyph(kind, w, h, state, bg=INK, fg=BONE):
    im = Image.new("RGBA", (w * X, h * X), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    W, H = im.size
    if state == "Grey": bg, fg = (110, 106, 98), (170, 166, 156)
    if state == "Down": bg = tuple(int(c * 0.8) for c in bg)
    d.rectangle([0, 0, W - 1, H - 1], fill=bg + (255,))
    if state == "Lit": d.rectangle([0, 0, W - 1, H - 1], outline=LIME + (255,), width=max(2, W // 10))
    c = (W / 2, H / 2); r = min(W, H) * 0.28
    tri = {"up": [(c[0], c[1] - r), (c[0] + r, c[1] + r * 0.6), (c[0] - r, c[1] + r * 0.6)],
           "down": [(c[0], c[1] + r), (c[0] + r, c[1] - r * 0.6), (c[0] - r, c[1] - r * 0.6)],
           "left": [(c[0] - r, c[1]), (c[0] + r * 0.6, c[1] - r), (c[0] + r * 0.6, c[1] + r)],
           "right": [(c[0] + r, c[1]), (c[0] - r * 0.6, c[1] - r), (c[0] - r * 0.6, c[1] + r)]}
    if kind in tri: d.polygon(tri[kind], fill=(LIME if state == "Lit" else fg) + (255,))
    elif kind == "thumb":
        for i in (-1, 0, 1):
            if W >= H: d.line([c[0] + i * W * 0.15, H * 0.25, c[0] + i * W * 0.15, H * 0.75], fill=fg + (255,), width=X)
            else: d.line([W * 0.25, c[1] + i * H * 0.15, W * 0.75, c[1] + i * H * 0.15], fill=fg + (255,), width=X)
    return im

def check(kind, on, grey):
    w, h = (14, 18) if kind == "check" else (14, 14)
    im = Image.new("RGBA", (w * X, h * X), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    fg = (150, 146, 138) if grey else BONE; ac = (120, 130, 90) if grey else LIME
    s = 12 * X; y0 = (h * X - s) // 2
    if kind == "check":
        d.rectangle([1, y0, s, y0 + s], outline=fg + (255,), width=X)
        if on: d.line([(4, y0 + s * 0.55), (s * 0.42, y0 + s - 4), (s + 3, y0 - 3)], fill=ac + (255,), width=X * 2)
    else:
        d.ellipse([1, y0, s, y0 + s], outline=fg + (255,), width=X)
        if on: d.ellipse([s * 0.3, y0 + s * 0.3, s * 0.7, y0 + s * 0.7], fill=ac + (255,))
    return im

def track(w, h, vertical=True):
    im = Image.new("RGBA", (w * X, h * X), (20, 19, 17, 255)); d = ImageDraw.Draw(im)
    W, H = im.size
    if vertical: d.line([W / 2, 2, W / 2, H - 3], fill=(80, 76, 68, 255), width=X)
    else: d.line([2, H / 2, W - 3, H / 2], fill=(80, 76, 68, 255), width=X)
    return im

def field(w, h, drop=False):
    """Text box / drop-down field: dark inset with a bone hairline."""
    im = Image.new("RGBA", (w * X, h * X), (16, 15, 14, 255)); d = ImageDraw.Draw(im)
    W, H = im.size; d.rectangle([0, 0, W - 1, H - 1], outline=(96, 92, 82, 255), width=X)
    if drop:
        bw = min(H, W // 4); d.rectangle([W - bw, 0, W - 1, H - 1], fill=LIME + (255,))
        c = (W - bw / 2, H / 2); r = bw * 0.22
        d.polygon([(c[0] - r, c[1] - r * 0.5), (c[0] + r, c[1] - r * 0.5), (c[0], c[1] + r * 0.6)], fill=INK + (255,))
    return im

# ---------------------------------------------------------------- screens

MAIN_BUTTONS = [  # control, image stem, label, icon, colours, sub
    ("MenuSingle", "Single", "SINGLE", "flag", INK, BONE, "01 / RACE"),
    ("MenuMulti", "Multi", "MULTI", "helmets", BONE, INK, "02 / ONLINE"),
    ("MenuOptions", "MenuOptions", "OPTIONS", "wrench", OLIVE, BONE, "03 / SET-UP"),
    ("MenuUserProfile", "Profile", "RIDER", "helmet", INK, BONE, "04 / PROFILE"),
    ("MenuHighScore", "HighScore", "SCORES", "trophy", LIME, INK, "05 / RECORDS"),
    ("MenuReplay", "Replay", "REPLAY", "replay", BONE, INK, "06 / TAPES"),
    ("Menuexit", "Exit", "EXIT", "exit", INK, BONE, None),
]
SIDE_BUTTONS = [("MenuHelp", "MenuHelp", "HELP", "help", BONE, INK), ("MenuCredits", "Credits", "CREDITS", "star", OLIVE, BONE)]

def main_menu(photo):
    dat = "MAINMENU"; rects = {}
    bg, _ = M.compose_bg(photo)
    bg = M.grain(bg, 6).convert("RGBA")
    save(dat, r"01_Main\BackGround_01.tga", bg)
    r = np.random.default_rng(21)
    for i, (ctl, stem, text, ico, b, f, sub) in enumerate(MAIN_BUTTONS):
        x, y, w, h = 18 + (i % 2) * 11, 27 + i * 60, 180, 58
        rects[ctl] = [x, y, w, h]
        sticker_set(dat, None, "01_Main\\" + stem + "{s}.tga", w * X, h * X, text, b, f, ico, sub, float(r.uniform(-3, 3)), seed=i)
    for j, (ctl, stem, text, ico, b, f) in enumerate(SIDE_BUTTONS):
        x, y, w, h = 452, 338 + j * 58, 160, 54
        rects[ctl] = [x, y, w, h]
        sticker_set(dat, None, "01_Main\\" + stem + "{s}.tga", w * X, h * X, text, b, f, ico, None, float(r.uniform(-3, 3)), seed=10 + j)
    # Free Pack's own Donate button (a new control: the loader adds it from the layout file)
    x, y, w, h = 452, 280, 160, 54
    rects["FreeDonate"] = [x, y, w, h]
    sticker_set(dat, None, "01_Main\\FreeDonate{s}.tga", w * X, h * X, "DONATE", RUST_SOFT, BONE, "heart", "KEEP IT FREE", float(r.uniform(-3, 3)), seed=12)
    builder_button(rects)
    # generic blank buttons (text drawn by the game)
    for k, (b, f) in enumerate(((INK, BONE), (BONE, INK)), 1):
        sticker_set(dat, None, "01_Main\\Button{s}0%d_18.tga" % k, 116 * X, 33 * X, "", b, f, seed=30 + k)
    # multiplayer pull-down choices
    for stem, text, ico, (b, f), (w, h) in (("TCP", "DIRECT IP", None, (BONE, INK), (103, 81)), ("Zone", "ONLINE RACES", "helmets", (LIME, INK), (156, 64)),
                                           ("IPX", "LAN", None, (INK, BONE), (112, 48)), ("Cable", "CABLE", None, (INK, BONE), (112, 37)), ("Modem", "MODEM", None, (INK, BONE), (111, 37))):
        sticker_set(dat, None, "01_Main\\" + stem + "{s}.tga", w * X, h * X, text, b, f, ico, None, float(r.uniform(-2, 2)), seed=40 + len(stem), align="center")
    # profile pull-down buttons
    for k, text in enumerate(("", "", "", ""), 1):  # the game draws these labels itself
        b, f = (LIME, INK) if k == 2 else (BONE, INK)
        sticker_set(dat, None, "01_Main\\UserButton0%d{s}.tga" % k, 117 * X, 34 * X, text, b, f, seed=50 + k, align="center")
    for s in ("Up", "Lit", "Down"):
        save(dat, "01_Main\\UserUp%s.tga" % s, glyph("up", 20, 18, s)); save(dat, "01_Main\\UserDown%s.tga" % s, glyph("down", 20, 18, s))
    save(dat, r"01_Main\UserScrollBar.tga", track(20, 157)); save(dat, r"01_Main\UserThumb.tga", glyph("thumb", 8, 7, "Up", bg=BONE, fg=INK))
    # frame over the event clip in the Single pull-down: a taped polaroid edge, clear middle
    fw, fh = 144 * X, 104 * X; fr = Image.new("RGBA", (fw, fh), BONE + (255,)); d = ImageDraw.Draw(fr)
    d.rectangle([6 * X, 6 * X, fw - 6 * X, fh - 6 * X], fill=(0, 0, 0, 0))
    fr = taped(fr, 3); save(dat, r"01_Main\screenover.tga", fr)
    # pull-downs slide right to clear the bigger buttons (their contents are anchored to them)
    rects["MenuSingleAnimation"] = [182, 76, 458, 223]
    rects["MenuMultiAnimation"] = [205, 128, 383, 274]
    rects["MenuUserAnimation"] = [204, 152, 434, 328]
    write_layout(dat, rects)

def builder_button(rects):
    """The Course Builder's button on the main menu, above Donate (a new
    control, as Donate is)."""
    x, y, w, h = 452, 222, 160, 54
    rects["FreeBuilder"] = [x, y, w, h]
    sticker_set("MAINMENU", None, "01_Main\\FreeBuilder{s}.tga", w * X, h * X, "BUILDER", LIME, INK, "shovel", "MAKE A TRACK", 2.0, seed=13)

def pulldowns():
    dat = "GLOBAL"
    # Single: event list on the left (text by the game), clip frame on the right
    w, h = 458, 223; p = board(w * X, h * X, seed=2)
    spine(p, "SINGLE", 70); header(p, "PICK YOUR EVENT", 82, 12, 200, 20)
    inset(p, (82, 48, 118, 166)); inset(p, (244, 34, 150, 110), fill=(8, 8, 8))
    d = ImageDraw.Draw(p); f = font(MED, 11 * X)
    d.text((248 * X, 150 * X), "CLIP  ·  PREVIEW", font=f, fill=(150, 146, 136))
    full = taped(p, 4)
    for k, fr in enumerate(unroll(full), 1): save(dat, "01_Main\\SinglePullDown%04d.tga" % k, fr)
    # Multi: connection choices sit on the board
    w, h = 383, 274; p = board(w * X, h * X, seed=3); spine(p, "MULTI", 60); header(p, "RIDE WITH OTHERS", 72, 12, 200, 20)
    full = taped(p, 5)
    for k, fr in enumerate(unroll(full), 1): save(dat, "01_Main\\MultiPullDown%04d.tga" % k, fr)
    # User: profile list left, buttons right
    w, h = 434, 328; p = board(w * X, h * X, seed=4); spine(p, "RIDER", 70); header(p, "RIDER PROFILES", 150, 12, 180, 20)
    inset(p, (84, 44, 161, 185))
    full = taped(p, 6)
    for k, fr in enumerate(unroll(full), 1): save(dat, "01_Main\\UserPullDown%04d.tga" % k, fr)

def widgets():
    dat = "GLOBAL"
    for kind, stem, (w, h) in (("up", "UpArrow", (21, 21)), ("down", "DownArrow", (21, 21)), ("left", "LeftArrow", (21, 21)), ("right", "RightArrow", (21, 21)),
                               ("left", "ScrollLeft", (28, 24)), ("right", "ScrollRight", (28, 24)), ("thumb", "ElevatorThumb", (17, 17)),
                               ("thumb", "HorizSliderThumb", (12, 24)), ("thumb", "VertSliderThumb", (24, 12))):
        for s in STATES:
            g = glyph(kind, w, h, s, bg=(BONE if "Thumb" in stem else INK), fg=(INK if "Thumb" in stem else BONE))
            save(dat, "Resources\\%s%s.tga" % (stem, s), g)
    for on in (False, True):
        for grey in (False, True):
            n = ("On" if on else "Off") + ("Grey" if grey else "")
            save(dat, "Rev2GlobalSource\\CheckBox%s.tga" % n, check("check", on, grey))
            save(dat, "Resources\\RadioButton%s.tga" % n, check("radio", on, grey))
    # bottom bar of the single/multi set-up screens: big icon stickers
    r = np.random.default_rng(9)
    for stem, text, ico, (w, h), (b, f) in (("R2Back", "BACK", "back", (79, 76), (BONE, INK)), ("R2Help", "HELP", "help", (56, 76), (BONE, INK)),
                                           ("R2Joy", "PAD", "pad", (58, 76), (BONE, INK)), ("R2Option", "OPTIONS", "wrench", (87, 76), (OLIVE, BONE)),
                                           ("R2Start", "START", "flag", (64, 76), (LIME, INK))):
        fw, fh = int(w * X * 0.86), int(h * X * 0.8)
        face = plate(fw, fh, b, len(stem)); d = ImageDraw.Draw(face)
        isz = int(fh * 0.5)
        if ico: face.alpha_composite(icon(ico, isz, f), ((fw - isz) // 2, int(fh * 0.08)))
        else:
            ff = fit_font("PAD", fw - 10, int(fh * 0.45)); bb = d.textbbox((0, 0), "PAD", font=ff)
            d.text(((fw - bb[2] - bb[0]) / 2, fh * 0.12 - bb[1]), "PAD", font=ff, fill=f)
        lf = fit_font(text, fw - 8, int(fh * 0.24)); bb = d.textbbox((0, 0), text, font=lf)
        d.text(((fw - (bb[2] - bb[0])) / 2 - bb[0], fh * 0.66 - bb[1]), text, font=lf, fill=f)
        for s in ("Up", "Lit", "Down"):
            save(dat, "Rev2GlobalSource\\%s%s.tga" % (stem, s), finish(face, w * X, h * X, float(r.uniform(-3, 3)), s))

def flat_sticker(w, h, text, bg, fg, seed=0):
    face = plate(w * X, h * X, bg, seed)
    return label(face, text, fg, align="center") if text else face

def corner(which, col=LIME):
    im = Image.new("RGBA", (4 * X, 4 * X), (0, 0, 0, 0)); d = ImageDraw.Draw(im); W = 4 * X
    pts = {"UL": [(0, 0), (W, 0), (0, W)], "UR": [(0, 0), (W, 0), (W, W)], "LL": [(0, 0), (0, W), (W, W)], "LR": [(W, 0), (W, W), (0, W)]}[which]
    d.polygon(pts, fill=col + (255,)); return im

def screen_frame(w, h):
    im = Image.new("RGBA", (w * X, h * X), (8, 8, 8, 255)); d = ImageDraw.Draw(im)
    d.rectangle([0, 0, w * X - 1, h * X - 1], outline=BONE + (255,), width=3 * X)
    return im

def page_back(w, h, title, seed):
    p = board(w * X, h * X, tone=(30, 28, 25), seed=seed)
    d = ImageDraw.Draw(p); f = font(MED, 10 * X)
    d.text((w * X - 150 * X, h * X - 16 * X), title, font=f, fill=(120, 116, 106))
    return taped(p, seed)

def sp_base(photo):
    dat = "SPBASE"; W, H = 640 * X, 480 * X
    bg = ImageOps.fit(Image.open(photo).convert("RGB"), (W, H), centering=(0.5, 0.6)).convert("RGBA")
    # set-up screens: the photo strip sits under the bottom bar, paper on top
    c = Image.new("RGBA", (W, H), M.BONE + (255,))
    ph = bg.crop((0, H - 90 * X, W, H)); c.alpha_composite(ph, (0, H - 90 * X))
    ht = M.halftone(bg.crop((0, 0, W, 400 * X)).filter(ImageFilter.GaussianBlur(1)), 7)
    ink = Image.new("RGBA", (W, 400 * X), (60, 56, 50, 255)); c.paste(ink, (0, 0), ImageOps.invert(ht).point(lambda v: int(v * 0.18)))
    d = ImageDraw.Draw(c); spec = font(MED, 8 * X)
    d.text((436 * X, 12 * X), "OPENMCM2  /  SINGLE PLAYER  /  SET-UP", font=spec, fill=INK)
    d.line([12 * X, 34 * X, 628 * X, 34 * X], fill=INK, width=X)
    d.rectangle([0, 396 * X, W, 400 * X], fill=INK)
    save(dat, r"SinglePlayer\SPBackGround.tga", M.grain(c, 5).convert("RGBA"))
    for k, text in enumerate(("EVENT", "BIKE + RIDER", "RACE INFO", ""), 1):
        w = (100, 101, 99, 100)[k - 1]
        save(dat, "SinglePlayer\\Tab%02d.tga" % k, flat_sticker(w, 33, text, INK, BONE, 60 + k))
        save(dat, "SinglePlayer\\TabDown%02d.tga" % k, flat_sticker(w, 33, text, LIME, INK, 60 + k))

def sp_pages():
    for dat, sub, back, title in (("SPEVENT", r"SinglePlayer\1_EventDialog", "SPEBack", "01 / EVENT"),
                                  ("SPBIKER", r"SinglePlayer\2_BikeRiderDlg", "SPBBack", "02 / BIKE + RIDER"),
                                  ("SPRINFO", r"SinglePlayer\3_RaceInfoDlg", "SPRBack", "03 / RACE INFO")):
        save(dat, sub + "\\" + back + ".tga", page_back(610, 355, title, len(dat)))
    e = "SinglePlayer\\1_EventDialog\\"; b = "SinglePlayer\\2_BikeRiderDlg\\"; r_ = "SinglePlayer\\3_RaceInfoDlg\\"
    save("SPEVENT", e + "Screen.tga", screen_frame(456, 224)); save("SPBIKER", b + "BRScreen.tga", screen_frame(456, 224)); save("SPRINFO", r_ + "RIScreen.tga", screen_frame(456, 224))
    for n in ("01", "02", "03"): save("SPEVENT", e + "Plate%s.tga" % n, flat_sticker(130, 57, "", OLIVE, BONE, int(n)))
    save("SPEVENT", e + "Plate04.tga", flat_sticker(315, 52, "", OLIVE, BONE, 4))
    save("SPEVENT", e + "GhostPanel.tga", flat_sticker(415, 28, "", OLIVE, BONE, 5))
    for dat, pre, items in (("SPEVENT", e, [("DropDown01", 108, 21, True), ("DropDown02", 157, 21, True), ("EvDropDown04", 412, 21, True), ("EventTextBox", 412, 21, False),
                                            ("GhostDropDown", 331, 20, True), ("SmalleditBox", 33, 21, False)]),
                            ("SPBIKER", b, [("BRDropDown01", 157, 21, True), ("BRDropDown03", 199, 21, True), ("BRDropDown04", 412, 21, True), ("BREventTextBox", 412, 21, False), ("BRSmalleditBox", 33, 21, False)])):
        for n, w, h, drop in items: save(dat, pre + n + ".tga", field(w, h, drop))
    for dat, pre, items in (("SPEVENT", e, [("DropDownBG01", 108, 77), ("DropDownBG02", 157, 62), ("EvDropDownBG04", 412, 96), ("GhostDropDownBG", 331, 113)]),
                            ("SPBIKER", b, [("BRDropDownBG01", 157, 62), ("BRDropDownBG03", 199, 62), ("BRDropDownBG04", 412, 96)])):
        for n, w, h in items: save(dat, pre + n + ".tga", field(w, h))
    for dat, pre, items in (("SPEVENT", e, [("EvVertScroll04", 21, 96), ("GhostVerticleScroll", 21, 113), ("VerticleScroll01", 21, 77), ("VerticleScroll02", 21, 62)]),
                            ("SPBIKER", b, [("BRVertScroll04", 21, 96), ("BRVerticleScroll", 21, 62), ("BRVerticleScroll02", 21, 62)])):
        for n, w, h in items: save(dat, pre + n + ".tga", track(w, h))
    for dat, pre, p in (("SPEVENT", e, "SPCorner"), ("SPBIKER", b, "BRCorner")):
        for k in ("UL", "UR", "LL", "LR"): save(dat, pre + p + k + ".tga", corner(k))
    sticker_set("SPBIKER", None, {"Up": b + "SnglWrenchUp.tga", "Lit": b + "SnglWrenchLit.tga", "Down": b + "SnglWrenchDown.tga"}, 99 * X, 37 * X, "TUNE", BONE, INK, "wrench", seed=70)
    sticker_set("SPRINFO", None, {"Up": r_ + "SPSaveReplayUp.tga", "Down": r_ + "SPSaveReplayDown.tga", "Lit": r_ + "SPSaveReplayLit.tga"}, 117 * X, 33 * X, "", LIME, INK, seed=71, align="center")
    bar = Image.new("RGBA", (456 * X, 21 * X), LIME + (255,)); save("SPRINFO", r_ + "InfoBar.tga", bar)

def write_layout(dat, rects):
    p = os.path.join(OUT, dat + ".toml")
    with open(p, "w") as f:
        f.write("# Free Pack layout for %s: control rects in 640x480 units\n" % dat)
        for k, v in rects.items():
            f.write('\n[controls.%s]\nrect = [%s]\n' % (k, ", ".join(map(str, v))))
            if k == "FreeDonate":
                f.write('new = true  # not in the stock dialog: a button the Free Pack adds\nkind = "button"\nimages = { up = "01_Main/FreeDonateUp.png", lit = "01_Main/FreeDonateLit.png", down = "01_Main/FreeDonateDown.png", grey = "01_Main/FreeDonateGrey.png" }\ntip = "Support OpenMCM2: opens the donate page"\naction = "open_url:https://ko-fi.com/crspytopgn"\n')
            if k == "FreeBuilder":
                f.write('new = true  # not in the stock dialog: a button the Free Pack adds\nkind = "button"\nimages = { up = "01_Main/FreeBuilderUp.png", lit = "01_Main/FreeBuilderLit.png", down = "01_Main/FreeBuilderDown.png", grey = "01_Main/FreeBuilderGrey.png" }\ntip = "Course Builder: make your own track and ride it"\naction = "course_builder"\n')
    written.append(p)

if __name__ == "__main__":
    photo = sys.argv[1]
    if len(sys.argv) > 2: os.environ["FP_LOGO"] = sys.argv[2]
    os.environ.setdefault("FP_LOGOSTYLE", "asis")
    main_menu(photo); pulldowns(); widgets(); sp_base(photo); sp_pages()
    print(len(written), "files in", OUT)
