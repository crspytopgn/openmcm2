"""Mock-up of a Free Pack main menu at 2x (1280x960), using the stock
main-menu button rectangles. Look: 2000s skate-mag cut-and-paste (halftone
photo, tape, stickers, heavy condensed caps) with an outdoor-catalogue
cleanliness (thin rules, small spec text, muted earth palette)."""
import sys, os, numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageOps, ImageChops
F = os.path.join(os.path.dirname(__file__), "..", "cache", "fonts")
def font(rel, size, var=None):
    f = ImageFont.truetype(os.path.join(F, rel), size)
    if var:
        try: f.set_variation_by_axes(var)
        except Exception: pass
    return f
S = 2; W, H = 640 * S, 480 * S
rng = np.random.default_rng(3)
LIME = (163, 194, 74); BONE = (232, 226, 210); INK = (24, 22, 20); RUST = (176, 74, 38); OLIVE = (92, 98, 70); SKY = (120, 140, 150)

def grain(img, amt=10):
    a = np.asarray(img.convert("RGB"), np.float32)
    a += rng.normal(0, amt, a.shape[:2])[..., None]
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))

def halftone(img, cell=6):
    g = np.asarray(ImageOps.grayscale(img), np.float32) / 255
    out = Image.new("L", img.size, 255); d = ImageDraw.Draw(out)
    for y in range(0, img.size[1], cell):
        for x in range(0 + (y // cell % 2) * cell // 2, img.size[0], cell):
            v = 1 - g[min(y, g.shape[0]-1), min(x, g.shape[1]-1)]
            r = cell * 0.62 * v ** 0.8
            if r > 0.3: d.ellipse([x - r, y - r, x + r, y + r], fill=0)
    return out

def tape(w, h, col=(214, 206, 180), alpha=215):
    t = Image.new("RGBA", (w, h), col + (alpha,)); a = np.asarray(t).copy()
    for side in (0, w - 1):  # torn ends
        for y in range(h):
            k = int(rng.integers(0, 6))
            if side == 0: a[y, :k, 3] = 0
            else: a[y, w - k:, 3] = 0
    a[..., :3] = np.clip(a[..., :3].astype(int) + rng.integers(-8, 8, a.shape[:2])[..., None], 0, 255)
    return Image.fromarray(a.astype(np.uint8))


def icon(kind, sz, col):
    im = Image.new("RGBA", (sz, sz), (0, 0, 0, 0)); d = ImageDraw.Draw(im); w = max(3, sz // 11); c = col
    m = sz * 0.12; a, b = m, sz - m
    if kind == "flag":
        d.line([a, a, a, b], fill=c, width=w)
        q = (b - a) / 4
        for i in range(4):
            for j in range(3):
                if (i + j) % 2 == 0: d.rectangle([a + w / 2 + i * q * 0.85, a + j * q * 0.8, a + w / 2 + (i + 1) * q * 0.85, a + (j + 1) * q * 0.8], fill=c)
        d.rectangle([a + w / 2, a, a + w / 2 + 4 * q * 0.85, a + 3 * q * 0.8], outline=c, width=max(2, w // 2))
    elif kind == "helmet" or kind == "helmets":
        def hel(ox, oy, r):
            d.chord([ox - r, oy - r, ox + r, oy + r], 180, 360, fill=c)
            d.rectangle([ox - r, oy - 1, ox + r * 0.2, oy + r * 0.55], fill=c)
            d.polygon([(ox + r * 0.3, oy - r * 0.15), (ox + r * 1.35, oy - r * 0.05), (ox + r * 0.3, oy + r * 0.2)], fill=c)
        if kind == "helmet": hel(sz * 0.45, sz * 0.55, sz * 0.33)
        else: hel(sz * 0.33, sz * 0.62, sz * 0.22); hel(sz * 0.62, sz * 0.42, sz * 0.22)
    elif kind == "wrench":
        d.line([a + sz * 0.15, b - sz * 0.15, b - sz * 0.22, a + sz * 0.22], fill=c, width=w + 3)
        d.ellipse([b - sz * 0.38, a, b, a + sz * 0.38], outline=c, width=w)
        d.ellipse([a, b - sz * 0.25, a + sz * 0.25, b], fill=c)
    elif kind == "trophy":
        d.pieslice([a + sz * 0.1, a - sz * 0.25, b - sz * 0.1, a + sz * 0.45], 0, 180, fill=c)
        d.arc([a - sz * 0.02, a + sz * 0.02, a + sz * 0.3, a + sz * 0.3], 90, 270, fill=c, width=w // 2 + 1)
        d.arc([b - sz * 0.3, a + sz * 0.02, b + sz * 0.02, a + sz * 0.3], 270, 90, fill=c, width=w // 2 + 1)
        d.rectangle([sz * 0.45, a + sz * 0.45, sz * 0.55, b - sz * 0.15], fill=c)
        d.rectangle([a + sz * 0.15, b - sz * 0.15, b - sz * 0.15, b], fill=c)
    elif kind == "replay":
        d.rounded_rectangle([a, a + sz * 0.12, b, b - sz * 0.12], radius=sz // 12, outline=c, width=w)
        for cx in (0.34, 0.66): d.ellipse([sz * cx - sz * 0.1, sz * 0.4, sz * cx + sz * 0.1, sz * 0.6], outline=c, width=max(2, w // 2))
    elif kind == "exit":
        d.line([b - sz * 0.25, a, a, a, a, b, b - sz * 0.25, b], fill=c, width=w, joint="curve")
        d.line([sz * 0.35, sz / 2, b, sz / 2], fill=c, width=w)
        d.polygon([(b + 2, sz / 2), (b - sz * 0.2, sz * 0.3), (b - sz * 0.2, sz * 0.7)], fill=c)
    elif kind == "help":
        d.ellipse([a, a, b, b], outline=c, width=w)
        f = font("archivoblack/ArchivoBlack-Regular.ttf", int(sz * 0.55)); bb = d.textbbox((0, 0), "?", font=f)
        d.text(((sz - bb[2] - bb[0]) / 2, (sz - bb[3] - bb[1]) / 2), "?", font=f, fill=c)
    elif kind == "pad":
        d.rounded_rectangle([a, sz * 0.3, b, sz * 0.72], radius=sz * 0.18, fill=c)
        d.ellipse([a - sz * 0.02, sz * 0.45, a + sz * 0.3, b], fill=c); d.ellipse([b - sz * 0.3, sz * 0.45, b + sz * 0.02, b], fill=c)
        bg = (0, 0, 0, 0); cx, cy = sz * 0.32, sz * 0.5; t = sz * 0.05
        d.rectangle([cx - t * 2.5, cy - t, cx + t * 2.5, cy + t], fill=bg); d.rectangle([cx - t, cy - t * 2.5, cx + t, cy + t * 2.5], fill=bg)
        for dx, dy in ((0.66, 0.42), (0.74, 0.52), (0.66, 0.6), (0.58, 0.52)): d.ellipse([sz * dx - t, sz * dy - t, sz * dx + t, sz * dy + t], fill=bg)
    elif kind == "back":
        d.line([b, sz / 2, a + sz * 0.15, sz / 2], fill=c, width=w)
        d.polygon([(a - 2, sz / 2), (a + sz * 0.3, sz * 0.26), (a + sz * 0.3, sz * 0.74)], fill=c)
    elif kind == "heart":
        r = sz * 0.2; d.ellipse([sz * 0.5 - 2 * r, sz * 0.3 - r * 0.2, sz * 0.5, sz * 0.3 + r * 1.8], fill=c)
        d.ellipse([sz * 0.5, sz * 0.3 - r * 0.2, sz * 0.5 + 2 * r, sz * 0.3 + r * 1.8], fill=c)
        d.polygon([(sz * 0.5 - 2 * r + 1, sz * 0.3 + r * 1.05), (sz * 0.5 + 2 * r - 1, sz * 0.3 + r * 1.05), (sz * 0.5, b)], fill=c)
    elif kind == "star":
        import math
        pts = [(sz / 2 + (sz * 0.42 if i % 2 == 0 else sz * 0.18) * math.sin(i * math.pi / 5), sz / 2 - (sz * 0.42 if i % 2 == 0 else sz * 0.18) * math.cos(i * math.pi / 5)) for i in range(10)]
        d.polygon(pts, fill=c)
    return im

def sticker(text, w, h, bg, fg, f, rot, sub=None, ico=None):
    pad = 10
    im = Image.new("RGBA", (w + pad * 2, h + pad * 2), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.rectangle([pad, pad, pad + w, pad + h], fill=bg)
    bb = d.textbbox((0, 0), text, font=f); tw, th = bb[2] - bb[0], bb[3] - bb[1]
    isz = int(h * 0.62) if ico else 0
    if ico: im.alpha_composite(icon(ico, isz, fg), (pad + 12, pad + int((h - isz) / 2) - (6 if sub else 0)))
    tx = pad + isz + 22 if ico else pad + (w - tw) / 2
    d.text((tx - bb[0], pad + (h - th) / 2 - bb[1] - (7 if sub else 0)), text, font=f, fill=fg)
    if sub:
        sf = font("barlowcondensed/BarlowCondensed-Medium.ttf", 16)
        d.text((pad + 8, pad + h - 21), sub, font=sf, fill=fg)
        d.line([pad + 8, pad + h - 24, pad + w - 8, pad + h - 24], fill=fg, width=1)
    # scuffs
    a = np.asarray(im).copy(); m = a[..., 3] > 0
    scuff = rng.random(a.shape[:2]) < 0.012
    a[scuff & m, :3] = np.clip(a[scuff & m, :3].astype(int) + 60, 0, 255)
    im = Image.fromarray(a)
    sh = Image.new("RGBA", im.size, (0, 0, 0, 0)); sh.paste((0, 0, 0, 110), (0, 0), im.split()[3])
    sh = sh.filter(ImageFilter.GaussianBlur(3))
    out = Image.new("RGBA", (im.size[0] + 8, im.size[1] + 8), (0, 0, 0, 0))
    out.alpha_composite(sh, (5, 6)); out.alpha_composite(im, (0, 0))
    return out.rotate(rot, Image.BICUBIC, expand=True)

def wear(im, amount=0.06, seed=5):
    """Scuffed print: speckle holes and a slightly worn edge."""
    r = np.random.default_rng(seed); a = np.asarray(im).astype(np.float32)
    n = np.asarray(Image.fromarray((r.random(a.shape[:2]) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.2)), np.float32) / 255
    hole = n > (1 - amount * 0.5) * 0.62 + 0.38 * (1 - amount)
    a[..., 3] = np.where(hole, a[..., 3] * 0.15, a[..., 3])
    return Image.fromarray(a.astype(np.uint8))

def logo_style(logo, style):
    a = np.asarray(logo).astype(np.float32) / 255; rgb, al = a[..., :3], a[..., 3:]
    lum = rgb @ [0.3, 0.59, 0.11]
    if style == "faded":
        # sun-faded decal: half the saturation, lime knocked back to the olive family, warm paper tint
        rgb = rgb * 0.55 + lum[..., None] * 0.45
        rgb = rgb * [0.97, 0.95, 0.86] + 0.04
        out = Image.fromarray((np.concatenate([np.clip(rgb, 0, 1), al], -1) * 255).astype(np.uint8))
        return wear(out, 0.05)
    if style == "ink":
        # two-colour screen print on a bone sticker: dark plate becomes ink, chrome and lime become paper and olive
        ink = np.array(INK) / 255; bone = np.array(BONE) / 255; ol = np.array(OLIVE) / 255
        lime = (rgb[..., 1] > 0.6) & (rgb[..., 0] < 0.85) & (rgb[..., 2] < 0.4)
        base = np.where(lum[..., None] < 0.25, ink, bone)
        base = np.where(lime[..., None], ol, base)
        out = Image.fromarray((np.concatenate([base, al], -1) * 255).astype(np.uint8))
        return wear(out, 0.08)
    if style == "print": return wear(logo, 0.04)
    return logo

def compose_bg(photo):
    bg = Image.open(photo).convert("RGB")
    bg = ImageOps.fit(bg, (W, H), centering=(0.55, 0.45))
    # muted, warm, slightly faded print
    a = np.asarray(bg, np.float32) / 255
    lum = a @ [0.3, 0.59, 0.11]; a = a * 0.55 + lum[..., None] * 0.45
    a = a * [1.0, 0.97, 0.9]; a = 0.06 + a * 0.88
    bg = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8))
    canvas = bg.convert("RGBA")
    # left band: halftone xerox of the same photo on bone paper
    band_w = 430
    ht = halftone(bg.crop((0, 0, band_w, H)).filter(ImageFilter.GaussianBlur(1)), 7)
    paper = Image.new("RGBA", (band_w, H), BONE + (255,))
    ink = Image.new("RGBA", (band_w, H), (60, 56, 50, 255)); paper.paste(ink, (0, 0), ImageOps.invert(ht).point(lambda v: int(v * 0.35)))
    mask = Image.new("L", (band_w + 40, H), 0); md = ImageDraw.Draw(mask)
    edge = [(band_w - 10 + int(rng.integers(-8, 9)), y) for y in range(0, H + 24, 24)]
    md.polygon([(0, 0)] + edge + [(0, H)], fill=255)
    canvas.paste(paper, (0, 0), mask.crop((0, 0, band_w, H)))
    d = ImageDraw.Draw(canvas)
    # spec text, catalogue style
    spec = font("barlowcondensed/BarlowCondensed-Medium.ttf", 15)
    d.text((24, 22), "OPENMCM2  /  FREE PACK  /  VOL. 01", font=spec, fill=INK)
    d.line([24, 42, band_w - 40, 42], fill=INK, width=1)
    d.text((24, H - 40), "ALL-TERRAIN  ·  DIRT  ·  SAND  ·  SNOW", font=spec, fill=INK)
    # big title, right side, knocked out of a rust block
    logo = Image.open(os.environ.get("FP_LOGO", os.path.join(os.path.dirname(__file__), "logo.png"))).convert("RGBA")
    style = os.environ.get("FP_LOGOSTYLE", "full"); lw = {"full": 700, "asis": 640}.get(style, 560)
    if logo.size[1] / logo.size[0] > 0.45: lw = int(lw * 0.72)  # two-line plate
    logo = logo_style(logo.resize((lw, int(lw * logo.size[1] / logo.size[0])), Image.LANCZOS), style).rotate(2, Image.BICUBIC, expand=True)
    sh = Image.new("RGBA", logo.size, (0, 0, 0, 0)); sh.paste((0, 0, 0, 120), (0, 0), logo.split()[3]); sh = sh.filter(ImageFilter.GaussianBlur(8))
    lx = W - lw - 60; canvas.alpha_composite(sh, (lx + 8, 70 + 12)); canvas.alpha_composite(logo, (lx, 70))
    if style != "full":
        for tx, ty, rot in ((lx - 20, 60, 35), (lx + lw - 100, 70 + logo.size[1] - 75, 30)):
            canvas.alpha_composite(tape(110, 30).rotate(rot, expand=True), (tx, ty))
    tag = font("barlowcondensed/BarlowCondensed-Bold.ttf", 30)
    ty = 70 + logo.size[1] + 18; tx = lx + 10
    d.text((tx, ty), "RIDE IT LIKE YOU STOLE IT.", font=tag, fill=BONE)
    d.text((tx, ty + 36), "No disc. No catch. Just dirt.", font=spec, fill=(220, 214, 200))
    return canvas, spec

def main(photo, out):
    canvas, spec = compose_bg(photo); d = ImageDraw.Draw(canvas)
    # buttons at stock rects (x2), as stickers / tape labels
    btn = font("barlowcondensed/BarlowCondensed-Bold.ttf", 58)
    left = [("SINGLE", "flag", INK, BONE, "01 / RACE"), ("MULTI", "helmets", BONE, INK, "02 / ONLINE"),
            ("OPTIONS", "wrench", OLIVE, BONE, "03 / SET-UP"), ("RIDER", "helmet", INK, BONE, "04 / PROFILE"),
            ("SCORES", "trophy", LIME, INK, "05 / RECORDS"), ("REPLAY", "replay", BONE, INK, "06 / TAPES"),
            ("EXIT", "exit", INK, BONE, None)]
    for i, (t, ico, bgc, fg, sub) in enumerate(left):
        st = sticker(t, 330, 96, bgc + (255,), fg, btn, float(rng.uniform(-3.5, 3.5)), sub, ico)
        canvas.alpha_composite(st, (40 + (i % 2) * 22, 56 + i * 121))
    for j, (t, ico, bgc, fg) in enumerate([("HELP", "help", BONE, INK), ("CREDITS", "star", OLIVE, BONE)]):
        st = sticker(t, 300, 100, bgc + (255,), fg, btn, float(rng.uniform(-3, 3)), None, ico)
        canvas.alpha_composite(st, (W - 380, 640 + j * 132))
    d = ImageDraw.Draw(canvas)
    d.text((24, H - 40), "ALL-TERRAIN  ·  DIRT  ·  SAND  ·  SNOW", font=spec, fill=INK)
    # lit state: tape across SINGLE
    tp = tape(150, 34).rotate(-12, expand=True)
    canvas.alpha_composite(tp, (300, 52))
    # current profile line
    d.text((218 * S, 454 * S), "RIDER: KAI  ·  250 2T  ·  #47", font=spec, fill=BONE)
    img = grain(canvas, 7)
    img.save(out, quality=92)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
