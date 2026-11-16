#!/usr/bin/env python3
"""Makes the Steam library pictures in assets/steam/ from the logo and the
site's screenshots, for adding OpenMCM2 to Steam as a non-Steam game.
Needs Pillow and CairoSVG: pip install pillow cairosvg
Usage: python3 tools/make-steam-art.py"""
import io
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "steam"
SHOTS = ROOT / "site" / "img"
LOGO = ROOT / "assets" / "icon" / "openmcm2-dark.svg"
ICON = ROOT / "assets" / "icon" / "app-icon.svg"


def svg(path, width):
    png = cairosvg.svg2png(url=str(path), output_width=width)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def shot(name, box, size):
    """A crop of a screenshot (x, y, w, h on its 1600x900), clear of the
    race display in its corners, scaled to `size`."""
    im = Image.open(SHOTS / name).convert("RGB")
    x, y, w, h = box
    return im.crop((x, y, x + w, y + h)).resize(size, Image.LANCZOS)


def shade(im, top, bottom):
    """Darkens the picture towards its top and bottom edges (0 to 1), so
    the logo reads on it."""
    w, h = im.size
    mask = Image.new("L", (1, h))
    for y in range(h):
        t = y / (h - 1)
        a = max(top * (1 - t / 0.45) if t < 0.45 else 0, bottom * ((t - 0.5) / 0.5) if t > 0.5 else 0)
        mask.putpixel((0, y), int(255 * min(1, a)))
    mask = mask.resize((w, h))
    dark = Image.new("RGB", (w, h), (14, 20, 28))
    return Image.composite(dark, im, mask)


def with_logo(im, width, centre_y):
    logo = svg(LOGO, width)
    glow = Image.new("RGBA", logo.size, (0, 0, 0, 0))
    glow.putalpha(logo.getchannel("A").filter(ImageFilter.GaussianBlur(width / 60)))
    im = im.convert("RGBA")
    at = ((im.width - logo.width) // 2, int(centre_y - logo.height / 2))
    shadow = Image.new("RGBA", logo.size, (0, 0, 0, 0))
    shadow.paste((0, 0, 0, 170), (0, 0), glow)
    im.alpha_composite(shadow, (at[0], at[1] + max(2, width // 200)))
    im.alpha_composite(logo, at)
    return im.convert("RGB")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    # Portrait capsule (the library grid), 600x900.
    cap = shot("enduro-airfield.jpg", (480, 150, 640, 690), (600, 900))
    cap = shade(cap, 0.0, 0.85)
    with_logo(cap, 520, 790).save(OUT / "capsule-600x900.png", optimize=True)
    # Wide capsule (recent games, the Deck's home row), 920x430.
    wide = shot("hero-dusk.jpg", (300, 225, 1000, 467), (920, 430))
    wide = shade(wide, 0.0, 0.8)
    with_logo(wide, 560, 350).save(OUT / "wide-920x430.png", optimize=True)
    # Hero (the banner on the game's page), 1920x620, no logo: Steam
    # puts the logo picture over it.
    hero = shot("nat-butte.jpg", (40, 235, 1400, 452), (1920, 620))
    hero.save(OUT / "hero-1920x620.png", optimize=True)
    # Logo, on a clear background.
    svg(LOGO, 1280).save(OUT / "logo.png", optimize=True)
    # Icon.
    svg(ICON, 512).save(OUT / "icon-512.png", optimize=True)
    for p in sorted(OUT.glob("*.png")):
        print(p.relative_to(ROOT), Image.open(p).size)


if __name__ == "__main__":
    main()
