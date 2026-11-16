#!/usr/bin/env python3
"""Write build/ground/NAME.png: 512 px seamless ground tiles (sRGB) for the
free tracks, made from ambientCG CC0 scans. Contrast is pulled in so the
ground reads calm from a riding camera.

    python tools/make_ground.py CACHE_DIR   (folder holding <Id>/<Id>_2K-JPG_Color.jpg)
"""
import os, sys
import numpy as np
from PIL import Image
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(HERE, "build", "ground"); os.makedirs(OUT, exist_ok=True)
SRC = sys.argv[1]
# name: (ambientCG id, multiply tint, saturation, contrast, extra)
G = {
    "trackdirt":   ("Ground067", (1.0, 0.93, 0.86), 0.9, 0.7, "ruts"),
    "loosedirt":   ("Ground082S", (1.05, 0.98, 0.92), 0.9, 0.75, None),
    "mud":         ("Ground048", (1.25, 1.1, 0.95), 0.85, 0.6, None),
    "grass":       ("Grass004", (0.92, 0.95, 0.85), 0.85, 0.7, None),
    "drygrass":    ("Grass004", (1.0, 1.0, 1.0), 0.6, 0.7, "straw"),
    "desertsand":  ("Ground079S", (1.0, 0.95, 0.88), 0.95, 0.75, None),
    "gravel":      ("Gravel022", (0.95, 0.94, 0.92), 0.8, 0.65, None),
    "rock":        ("Rocks011", (0.9, 0.88, 0.85), 0.8, 0.7, None),
    "forestfloor": ("Ground078", (0.85, 0.82, 0.78), 0.85, 0.7, None),
    "sxclay":      ("Ground051", (0.62, 0.5, 0.42), 1.0, 0.7, "ruts"),
    "concrete":    ("Concrete034", (0.78, 0.78, 0.78), 0.5, 0.6, None),
}

def periodic_noise(n, k, seed):
    """1D noise that wraps: a sum of whole-period sines."""
    r = np.random.RandomState(seed); x = np.arange(n) / n; v = np.zeros(n)
    for f in range(1, k + 1): v += r.randn() / f * np.sin(2 * np.pi * (f * x + r.rand()))
    return (v - v.min()) / (np.ptp(v) + 1e-9)

for name, (cid, tint, sat, con, extra) in G.items():
    im = Image.open(os.path.join(SRC, cid, f"{cid}_2K-JPG_Color.jpg")).convert("RGB").resize((512, 512), Image.LANCZOS)
    a = np.asarray(im).astype(np.float32) / 255.0
    if extra == "straw":
        # dry summer grass: pull hue toward straw and lift it a little
        lum = a.mean(2, keepdims=True); a = lum * np.array([1.55, 1.25, 0.7]) * 1.2
    lum = a.mean(2, keepdims=True)
    a = lum + (a - lum) * sat
    m = a.mean((0, 1)); a = m + (a - m) * con
    a *= np.array(tint)
    if extra == "ruts":
        # faint tyre lines running along v: darker, smoother bands that wrap
        band = periodic_noise(512, 24, hash(name) & 0xffff)
        mask = np.clip((band - 0.62) / 0.2, 0, 1)[None, :, None]
        a = a * (1 - 0.12 * mask) + 0.12 * mask * a.mean((0, 1))
    if extra == "wet":
        a = a ** 1.15
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, name + ".png"))
    print("ground", name, cid)
