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
BONE = (232, 226, 210); INK = (24, 22, 20); RUST = (176, 74, 38); OLIVE = (92, 98, 70); SKY = (120, 140, 150)

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

def sticker(text, w, h, bg, fg, f, rot, sub=None):
    pad = 10
    im = Image.new("RGBA", (w + pad * 2, h + pad * 2), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.rectangle([pad, pad, pad + w, pad + h], fill=bg)
    bb = d.textbbox((0, 0), text, font=f); tw, th = bb[2] - bb[0], bb[3] - bb[1]
    d.text((pad + (w - tw) / 2 - bb[0], pad + (h - th) / 2 - bb[1] - (5 if sub else 0)), text, font=f, fill=fg)
    if sub:
        sf = font("barlowcondensed/BarlowCondensed-Medium.ttf", 13)
        d.text((pad + 6, pad + h - 17), sub, font=sf, fill=fg)
        d.line([pad + 6, pad + h - 20, pad + w - 6, pad + h - 20], fill=fg, width=1)
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

def main(photo, out):
    bg = Image.open(photo).convert("RGB")
    bg = ImageOps.fit(bg, (W, H), centering=(0.55, 0.45))
    # muted, warm, slightly faded print
    a = np.asarray(bg, np.float32) / 255
    lum = a @ [0.3, 0.59, 0.11]; a = a * 0.55 + lum[..., None] * 0.45
    a = a * [1.0, 0.97, 0.9]; a = 0.06 + a * 0.88
    bg = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8))
    canvas = bg.convert("RGBA")
    # left band: halftone xerox of the same photo on bone paper
    band_w = 300
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
    big = font("archivoblack/ArchivoBlack-Regular.ttf", 112)
    t1 = Image.new("RGBA", (640, 300), (0, 0, 0, 0)); td = ImageDraw.Draw(t1)
    td.rectangle([0, 18, 560, 150], fill=RUST + (240,))
    td.text((22, 6), "OPEN", font=big, fill=BONE)
    td.text((22, 128), "MCM2", font=big, fill=(245, 242, 232))
    t1 = t1.rotate(-3, Image.BICUBIC, expand=True)
    canvas.alpha_composite(t1, (W - 640, 40))
    tag = font("barlowcondensed/BarlowCondensed-Bold.ttf", 30)
    d.text((W - 600, 360), "RIDE IT LIKE YOU STOLE IT.", font=tag, fill=BONE)
    d.text((W - 600, 396), "No disc. No catch. Just dirt.", font=spec, fill=(220, 214, 200))
    # buttons at stock rects (x2), as stickers / tape labels
    btn = font("barlowcondensed/BarlowCondensed-Bold.ttf", 40)
    items = [("SINGLE", 64, 96, 79, 44, INK, BONE, "01 / RACE"), ("MULTI", 44, 140, 84, 45, BONE, INK, "02 / ONLINE"),
             ("OPTIONS", 35, 195, 86, 50, OLIVE, BONE, "03 / SET-UP"), ("RIDER", 36, 254, 80, 57, INK, BONE, "04 / PROFILE"),
             ("SCORES", 40, 318, 82, 55, RUST, BONE, "05 / RECORDS"), ("REPLAY", 54, 374, 77, 46, BONE, INK, "06 / TAPES"),
             ("EXIT", 73, 419, 70, 42, INK, BONE, None), ("HELP", 528, 339, 71, 47, BONE, INK, None), ("CREDITS", 522, 392, 69, 50, OLIVE, BONE, None)]
    for i, (t, x, y, w, h, bgc, fg, sub) in enumerate(items):
        st = sticker(t, w * S + 30, h * S - 10, bgc + (255,), fg, btn, float(rng.uniform(-4, 4)), sub)
        canvas.alpha_composite(st, (x * S - 10, y * S - 6))
    # lit state: tape across SINGLE
    tp = tape(150, 34).rotate(-12, expand=True)
    canvas.alpha_composite(tp, (64 * S + 120, 96 * S - 16))
    # current profile line
    d.text((218 * S, 454 * S), "RIDER: KAI  ·  250 2T  ·  #47", font=spec, fill=BONE)
    img = grain(canvas, 7)
    img.save(out, quality=92)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
