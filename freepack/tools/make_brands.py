#!/usr/bin/env python3
"""Banner art for made-up sponsors (no real brands), used on billboards
and hay-bale banners. Writes build/brands/<slug>.png (2048x512) and
<slug>_sq.png (1024x1024). Needs Pillow."""
import os, math, colorsys, random
from PIL import Image, ImageDraw, ImageFont, ImageFilter
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(HERE, "build", "brands"); FONTS = os.path.join(HERE, "cache", "fonts")
BRANDS = [  # name, tagline, hue, style
    ("RIDGEBACK", "ENGINE OILS", 0.02, "chevron"), ("KESTREL", "HELMETS", 0.58, "wing"),
    ("DUSTLINE", "RIDING GEAR", 0.08, "stripe"), ("NORTHFORK", "TYRES", 0.0, "circle"),
    ("SANDPIPER", "GOGGLES", 0.52, "dot"), ("IRONBARK", "CHAINS & SPROCKETS", 0.1, "chevron"),
    ("COPPERHEAD", "EXHAUSTS", 0.05, "stripe"), ("BLUEGUM", "RACING", 0.6, "circle"),
    ("HIGHMOOR", "BOOTS", 0.3, "wing"), ("THORNFIELD", "GLOVES", 0.95, "dot"),
    ("RIVERBEND", "SUSPENSION", 0.55, "stripe"), ("SLATE PEAK", "ENERGY DRINK", 0.75, "chevron"),
    ("JUNIPER", "HANDLEBARS", 0.38, "circle"), ("CINDER", "BRAKES", 0.01, "wing"),
    ("HALCYON", "AIR FILTERS", 0.48, "dot"), ("REDTAIL", "GRAPHICS", 0.99, "stripe"),
    ("MARLOW", "PLASTICS", 0.62, "chevron"), ("GRANITE", "MX PARTS", 0.12, "circle"),
    ("TALLGRASS", "TRAILERS", 0.25, "wing"), ("OAKMERE", "MOTOR CLUB", 0.09, "dot"),
]
def slug(n): return n.lower().replace(" ", "")
def col(h, s, v): return tuple(int(c * 255) for c in colorsys.hsv_to_rgb(h, s, v))
def font(name, size):
    p = {"bold": "barlowcondensed/BarlowCondensed-Bold.ttf", "black": "archivoblack/ArchivoBlack-Regular.ttf",
         "semi": "barlowcondensed/BarlowCondensed-SemiBold.ttf"}[name]
    return ImageFont.truetype(os.path.join(FONTS, p), size)
def mark(d, x, y, r, style, fg):
    if style == "chevron":
        for k in range(2):
            o = k * r * 0.55
            d.polygon([(x - r + o, y - r), (x - r * 0.45 + o, y - r), (x + r * 0.15 + o, y), (x - r * 0.45 + o, y + r), (x - r + o, y + r), (x - r * 0.4 + o, y)], fill=fg)
    elif style == "circle":
        d.ellipse([x - r, y - r, x + r, y + r], outline=fg, width=int(r * 0.28))
        d.ellipse([x - r * 0.35, y - r * 0.35, x + r * 0.35, y + r * 0.35], fill=fg)
    elif style == "wing":
        d.polygon([(x - r, y + r * 0.6), (x + r, y - r), (x + r * 0.2, y + r * 0.1), (x + r * 0.6, y + r * 0.6)], fill=fg)
    elif style == "dot":
        for i in range(3): d.ellipse([x - r + i * r * 0.75, y - r * 0.3, x - r + i * r * 0.75 + r * 0.6, y + r * 0.3], fill=fg)
    else:
        for i in range(3): d.rectangle([x - r + i * r * 0.7, y - r, x - r + i * r * 0.7 + r * 0.4, y + r], fill=fg)
def banner(name, tag, hue, style, w, h, race=False):
    rnd = random.Random(name)
    dark = rnd.random() < 0.5
    bg = col(hue, 0.45, 0.32) if dark else col(hue, 0.08, 0.86)
    fg = col(hue, 0.08, 0.92) if dark else col(hue, 0.55, 0.42)
    acc = col((hue + 0.5) % 1, 0.4, 0.7) if dark else col(hue, 0.5, 0.6)
    if race:
        # Race-day vinyl, as hung round a real track: the sponsor's own
        # strong colour behind bold white or black lettering.
        bg = col(hue, 0.78, 0.62 if dark else 0.78)
        fg = col(hue, 0.04, 0.97) if dark or hue > 0.55 or hue < 0.04 else col(hue, 0.6, 0.12)
        acc = col(hue, 0.25, 0.97) if fg[0] > 128 else col(hue, 0.9, 0.3)
    im = Image.new("RGB", (w, h), bg); d = ImageDraw.Draw(im)
    if style == "stripe": d.rectangle([0, h * 0.82, w, h * 0.9], fill=acc)
    r = h * 0.28; mx = h * 0.55
    mark(d, mx, h / 2, r, style, acc)
    face = "black" if rnd.random() < 0.5 else "bold"
    tx = mx + r * 1.6; size = int(h * 0.4)
    while True:
        f = font(face, size); tb = d.textbbox((tx, h * 0.44), name, font=f, anchor="lm")
        if tb[2] < w - h * 0.25 or size < 20: break
        size -= 4
    d.text((tx, h * 0.44), name, font=f, fill=fg, anchor="lm")
    ft = font("semi", int(h * 0.12))
    d.text((tb[0] + 4, tb[3] + h * 0.07), tag, font=ft, fill=acc, anchor="lt")
    # print weathering: faint noise and soft edges so it sits in the scene
    noise = Image.effect_noise((w, h), 18).convert("L").filter(ImageFilter.GaussianBlur(1))
    im = Image.blend(im, Image.merge("RGB", (noise, noise, noise)), 0.05)
    return im.filter(ImageFilter.GaussianBlur(0.6))
if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, tag, hue, style in BRANDS:
        banner(name, tag, hue, style, 2048, 512).save(os.path.join(OUT, slug(name) + ".png"))
        banner(name, tag, hue, style, 2048, 1024, race=True).save(os.path.join(OUT, slug(name) + "_race.png"))
    print(len(BRANDS), "banners")
