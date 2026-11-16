#!/usr/bin/env python3
"""Free Pack menu art for every other UI DAT (multiplayer, options, high
scores, ghosts and replays, Pro Circuit, VCR bar, loading screens, message
boxes and intro screens). Each image is drawn by kind, guessed from its
name and size, in the same sticker-and-paper look as make_ui.py.

Buttons whose control draws its own caption get a blank face (from
ui_textmap.tsv, made by mm2-formats' ui_textmap example).

Usage: make_ui_rest.py PHOTO.png UI_LIST.tsv UI_TEXTMAP.tsv
"""
import os, re, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageOps
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import make_ui as U
import mock_main as M
from mock_main import font, icon, INK, BONE, OLIVE, LIME
X = U.X
DONE = {"MAINMENU", "GLOBAL", "SPBASE", "SPEVENT", "SPBIKER", "SPRINFO"}
STATE = re.compile(r"(Up|Down|Lit|Grey)$")

TITLES = {"MPBASE": "MULTIPLAYER", "OPTBASE": "OPTIONS", "OPTADVG": "ADVANCED GRAPHICS", "HISCORES": "HIGH SCORES", "GRBASE": "GHOSTS + REPLAYS",
          "PCC": "PRO CIRCUIT", "PCDEAL": "PRO CIRCUIT / DEALS", "PCDONE": "PRO CIRCUIT", "PCFINISH": "PRO CIRCUIT / FINISH", "PCLRACE": "PRO CIRCUIT / LAST RACE",
          "PCSTART": "PRO CIRCUIT / CAREERS", "PCTRANS": "PRO CIRCUIT"}
LOADS = {"LOADBAJA": "BAJA", "LOADEND": "ENDURO", "LOADNAT": "NATIONALS", "LOADQUAR": "STUNT QUARRY", "LOADSX": "SUPERCROSS", "LOADTAG": "MOTO-TAG"}
TABS = {("MPBASE", "Tab00"): "EVENT", ("MPBASE", "Tab01"): "BIKE + RIDER", ("MPBASE", "Tab02"): "RACE INFO", ("MPBASE", "Tab03"): "OPTIONS"}
NAMED = {"MsgBx03Bttn01": "OK", "RdyButton": "READY"}
BAR = {"Back": ("BACK", "back", BONE, INK), "Help": ("HELP", "help", BONE, INK), "Joy": ("PAD", "pad", BONE, INK), "Options": ("OPTIONS", "wrench", OLIVE, BONE),
       "Start": ("START", "flag", LIME, INK), "Practice": ("PRACTICE", "star", BONE, INK)}

def photo_bg(photo):
    return ImageOps.fit(Image.open(photo).convert("RGB"), (640 * X, 480 * X), centering=(0.5, 0.55)).convert("RGBA")

def paper_screen(photo, title):
    """Set-up style screen: halftone paper, title rule, photo strip at the foot."""
    W, H = 640 * X, 480 * X; bg = photo_bg(photo)
    c = Image.new("RGBA", (W, H), BONE + (255,)); c.alpha_composite(bg.crop((0, H - 90 * X, W, H)), (0, H - 90 * X))
    ht = M.halftone(bg.crop((0, 0, W, 400 * X)).filter(ImageFilter.GaussianBlur(1)), 7)
    ink = Image.new("RGBA", (W, 400 * X), (60, 56, 50, 255)); c.paste(ink, (0, 0), ImageOps.invert(ht).point(lambda v: int(v * 0.18)))
    d = ImageDraw.Draw(c); spec = font(U.MED, 8 * X)
    d.text((436 * X, 12 * X), "OPENMCM2  /  " + title, font=spec, fill=INK)
    d.line([12 * X, 34 * X, 628 * X, 34 * X], fill=INK, width=X); d.rectangle([0, 396 * X, W, 400 * X], fill=INK)
    return M.grain(c, 5).convert("RGBA")

def loading(photo, title):
    W, H = 640 * X, 480 * X; c = photo_bg(photo)
    a = np.asarray(c, np.float32) / 255; lum = a[..., :3] @ [0.3, 0.59, 0.11]
    a[..., :3] = (a[..., :3] * 0.5 + lum[..., None] * 0.5) * 0.8; c = Image.fromarray((a * 255).astype(np.uint8))
    band = Image.new("RGBA", (W, 120 * X), INK + (235,)); c.alpha_composite(band, (0, 300 * X))
    d = ImageDraw.Draw(c); f = U.fit_font(title, W - 60 * X, 80 * X); bb = d.textbbox((0, 0), title, font=f)
    d.text((30 * X - bb[0], 310 * X - bb[1]), title, font=f, fill=BONE)
    d.text((32 * X, 398 * X), "LOADING  /  OPENMCM2 FREE PACK", font=font(U.MED, 9 * X), fill=LIME)
    logo = Image.open(os.environ.get("FP_LOGO", os.path.join(os.path.dirname(__file__), "logo.png"))).convert("RGBA")
    lw = 170 * X; logo = logo.resize((lw, int(lw * logo.size[1] / logo.size[0])), Image.LANCZOS)
    c.alpha_composite(logo, (W - lw - 24 * X, 24 * X))
    return M.grain(c, 6).convert("RGBA")

def splash(photo, kind):
    W, H = 640 * X, 480 * X; c = Image.new("RGBA", (W, H), INK + (255,)); d = ImageDraw.Draw(c)
    logo = Image.open(os.environ.get("FP_LOGO", os.path.join(os.path.dirname(__file__), "logo.png"))).convert("RGBA")
    if kind == 1:
        lw = 360 * X; logo = logo.resize((lw, int(lw * logo.size[1] / logo.size[0])), Image.LANCZOS)
        c.alpha_composite(logo, ((W - lw) // 2, (H - logo.size[1]) // 2))
    elif kind == 2:
        f = font(U.BOLD, 44 * X); t = "FREE PACK"; bb = d.textbbox((0, 0), t, font=f)
        d.text(((W - bb[2]) / 2, 190 * X), t, font=f, fill=BONE)
        s = font(U.MED, 12 * X); t2 = "OPEN ART  ·  OPEN SOUND  ·  OPEN SOURCE"; bb = d.textbbox((0, 0), t2, font=s)
        d.text(((W - bb[2]) / 2, 250 * X), t2, font=s, fill=LIME)
    else:
        s = font(U.MED, 11 * X)
        lines = ["OpenMCM2 is free software under the MIT and Apache-2.0 licences.", "The Free Pack art and sound are CC0, CC-BY or OFL; see CREDITS.md for every source.",
                 "Brands, sponsors and riders shown are made up.", "", "Ride safe. Wear your gear."]
        for i, l in enumerate(lines): d.text((60 * X, (170 + 22 * i) * X), l, font=s, fill=BONE)
    return M.grain(c, 4).convert("RGBA")

def label_from(stem):
    s = re.sub(r"^(PC|PCBR|B_R|MPE|MP|MO|GA|AG|GS|HS|HJ|G_R|SPop|E|I|C|G|S)(?=[A-Z_])", "", stem).strip("_")
    s = re.sub(r"(Bttn|Button|But)\d*$", "", s); s = re.sub(r"\d+$", "", s)
    s = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", s).replace("_", " ").strip()
    return s.upper()

VCR = {"Play": "play", "Pause": "pause", "Rewind": "rew", "FFx2": "ff", "FFx8": "ff8", "Beginning": "start", "End": "end", "Advance1": "step",
       "NextRide": "next", "PrevRide": "prev", "Camera": "cam", "Save": "save", "BackSml": "back"}

def vcr_icon(kind, w, h, state):
    im = U.glyph("none", w, h, state); d = ImageDraw.Draw(im); W, H = im.size
    col = (LIME if state == "Lit" else (BONE if state != "Grey" else (170, 166, 156))) + (255,)
    cx, cy, r = W / 2, H / 2, min(W, H) * 0.26
    tri = lambda x, s=1: d.polygon([(x - r * 0.6 * s, cy - r), (x + r * 0.8 * s, cy), (x - r * 0.6 * s, cy + r)], fill=col)
    if kind == "play": tri(cx)
    elif kind == "pause": d.rectangle([cx - r * 0.7, cy - r, cx - r * 0.2, cy + r], fill=col); d.rectangle([cx + r * 0.2, cy - r, cx + r * 0.7, cy + r], fill=col)
    elif kind in ("ff", "ff8"): tri(cx - r * 0.5); tri(cx + r * 0.6)
    elif kind == "rew": tri(cx + r * 0.5, -1); tri(cx - r * 0.6, -1)
    elif kind == "start": d.rectangle([cx - r * 1.1, cy - r, cx - r * 0.8, cy + r], fill=col); tri(cx + r * 0.2, -1)
    elif kind in ("end", "step"): tri(cx - r * 0.2); d.rectangle([cx + r * 0.8, cy - r, cx + r * 1.1, cy + r], fill=col)
    elif kind == "next": tri(cx - r * 0.3); d.ellipse([cx + r * 0.7, cy - r * 0.3, cx + r * 1.3, cy + r * 0.3], fill=col)
    elif kind == "prev": tri(cx + r * 0.3, -1); d.ellipse([cx - r * 1.3, cy - r * 0.3, cx - r * 0.7, cy + r * 0.3], fill=col)
    elif kind == "cam": d.rectangle([cx - r * 1.1, cy - r * 0.7, cx + r * 0.5, cy + r * 0.7], fill=col); d.polygon([(cx + r * 0.5, cy), (cx + r * 1.2, cy - r * 0.6), (cx + r * 1.2, cy + r * 0.6)], fill=col)
    elif kind == "save": d.rectangle([cx - r, cy - r, cx + r, cy + r], fill=col); d.rectangle([cx - r * 0.55, cy - r, cx + r * 0.55, cy - r * 0.3], fill=INK + (255,))
    elif kind == "back": im.alpha_composite(icon("back", int(min(W, H) * 0.7), col[:3]), (int(W * 0.15), int(H * 0.15)))
    return im

def main(photo, list_tsv, text_tsv):
    has_text = {}
    for l in open(text_tsv):
        dat, path, t, _ = l.rstrip("\n").split("\t"); has_text[(dat, path.lower())] = has_text.get((dat, path.lower()), 0) | int(t)
    rows = []
    for l in open(list_tsv):
        dat, path, size = l.rstrip("\n").split("\t")[:3]; dat = dat[:-4]
        if dat in DONE or "x" not in size: continue
        w, h = map(int, size.split("x")); rows.append((dat, path, w, h))
    r = np.random.default_rng(5); n = 0
    for dat, path, w, h in rows:
        base = path.rsplit("\\", 1)[-1][:-4]; low = base.lower(); m = STATE.search(base); st = m.group(1) if m else None
        stem = STATE.sub("", base) if m else base
        text = has_text.get((dat, path.lower()), 0)
        if (w, h) == (640, 480):
            if dat in LOADS: im = loading(photo, LOADS[dat])
            elif dat.startswith("INTRO"): im = splash(photo, int(dat[-1]))
            else: im = paper_screen(photo, TITLES.get(dat, dat))
        elif (w, h) == (4, 4): im = U.corner({"ll": "LL", "lr": "LR", "ul": "UL", "ur": "UR", "bl": "LL", "br": "LR"}[re.search(r"(ll|lr|ul|ur|bl|br)corner|corner(ll|lr|ul|ur)", low).group(1) or re.search(r"corner(ll|lr|ul|ur)", low).group(1)] if re.search(r"(ll|lr|ul|ur|bl|br)corner|corner(ll|lr|ul|ur)", low) else "UL")
        elif "thumb" in low: im = U.glyph("thumb", w, h, "Up", bg=BONE, fg=INK)
        elif "slider" in low and m: im = U.glyph("thumb", w, h, st, bg=BONE, fg=INK)
        elif "slider" in low: im = U.track(w, h, vertical=h > w)
        elif "scroll" in low: im = U.track(w, h, vertical=h > w)
        elif "bluebar" in low: im = Image.new("RGBA", (w * X, h * X), LIME + (255,))
        elif dat == "VCR" and stem in VCR: im = vcr_icon(VCR[stem], w, h, st or "Up")
        elif dat == "VCR" and stem == "VcrBG": im = U.board(w * X, h * X, seed=8, edge=OLIVE)
        elif re.search(r"(dropdownbg|pulldownbg)", low): im = U.field(w, h)
        elif re.search(r"(dropdown|pulldown)", low): im = U.field(w, h, drop=True)
        elif re.search(r"(edit|namebox|entrybox|textbox)", low): im = U.field(w, h)
        elif re.search(r"tab", low) and not m and not re.search(r"flash", low):
            down = "tabdown" in low; t = "" if text else TABS.get((dat, stem.replace("Down", "")), label_from(stem))
            im = U.flat_sticker(w, h, t, LIME if down else INK, INK if down else BONE, len(stem))
        elif re.search(r"flash\d+", low):
            k = int(re.search(r"(\d+)$", base).group(1)); im = U.flat_sticker(w, h, "", LIME if k % 2 else INK, INK, k)
        elif m and re.search(r"tab", low):
            im = U.flat_sticker(w, h, "" if text else label_from(stem), LIME if st == "Down" else INK, INK if st == "Down" else BONE, len(stem))
        elif m and h == 76:
            key = next((k for k in BAR if stem.endswith(k)), "Back"); t, ic, bgc, fgc = BAR[key]
            fw, fh = int(w * X * 0.86), int(h * X * 0.8); face = U.plate(fw, fh, bgc, len(stem)); d = ImageDraw.Draw(face); isz = int(fh * 0.5)
            face.alpha_composite(icon(ic, isz, fgc), ((fw - isz) // 2, int(fh * 0.08)))
            lf = U.fit_font(t, fw - 8, int(fh * 0.24)); bb = d.textbbox((0, 0), t, font=lf)
            d.text(((fw - (bb[2] - bb[0])) / 2 - bb[0], fh * 0.66 - bb[1]), t, font=lf, fill=fgc)
            im = U.finish(face, w * X, h * X, float(r.uniform(-3, 3)), st)
        elif m:
            if "wrench" in low: t, ic = "TUNE", "wrench"
            else: t, ic = ("" if text else next((v for k, v in NAMED.items() if stem.startswith(k)), label_from(stem))), None
            if re.search(r"(left|right)bttn", low): ic = None; t = "" if text else ("◀" if "left" in low else "▶")
            bgc, fgc = ((LIME, INK) if re.search(r"(accept|start|continue|host|ok|send|rdy)", low) else (BONE, INK))
            fw, fh = int(w * X * 0.92), int(h * X * 0.84)
            face = U.label(U.plate(fw, fh, bgc, len(stem)), t, fgc, ic, None, "center") if (t or ic) else U.plate(fw, fh, bgc, len(stem))
            im = U.finish(face, w * X, h * X, 0.0 if h < 40 else float(r.uniform(-2, 2)), st)
        elif re.search(r"(screen|chatbox)", low): im = U.screen_frame(w, h)
        elif re.search(r"(plate|panel|divider)", low): im = U.flat_sticker(w, h, "", OLIVE, BONE, len(stem))
        else: im = U.taped(U.board(w * X, h * X, tone=(30, 28, 25), seed=len(stem)), len(stem)) if w > 150 and h > 100 else U.board(w * X, h * X, seed=len(stem))
        U.save(dat, path, im); n += 1
    print(n, "images")

if __name__ == "__main__":
    os.environ.setdefault("FP_LOGOSTYLE", "asis")
    main(*sys.argv[1:4])
