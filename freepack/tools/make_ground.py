#!/usr/bin/env python3
"""Write build/ground/NAME.png: 2048 px seamless ground tiles (sRGB) for the
free tracks, made from ambientCG CC0 scans. The scans' ambient occlusion,
cavities (from the height map) and a soft overhead light on the normal map
are baked into the colour, since the game draws ground as colour only.
Contrast is pulled in so the ground reads calm from a riding camera.

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
    "loosedirt":   ("Ground103", (1.0, 1.0, 1.0), 1.0, 0.85, "nogreen"),
    "mud":         ("Ground049A", (1.0, 1.0, 1.0), 0.9, 1.0, "puddles"),
    "grass":       ("Grass004", (0.92, 0.95, 0.85), 0.85, 0.7, None),
    "drygrass":    ("Grass004", (1.0, 1.0, 1.0), 0.6, 0.7, "straw"),
    "desertsand":  ("Ground079S", (1.0, 0.95, 0.88), 0.95, 0.75, None),
    "gravel":      ("Gravel022", (0.95, 0.94, 0.92), 0.8, 0.65, None),
    "rock":        ("Rock030", (0.9, 0.88, 0.85), 0.8, 0.7, None),
    "forestfloor": ("Ground078", (0.85, 0.82, 0.78), 0.85, 0.7, None),
    "sxclay":      ("Ground051", (0.62, 0.5, 0.42), 1.0, 0.7, "ruts"),
    "concrete":    ("Concrete034", (0.78, 0.78, 0.78), 0.5, 0.6, None),
}

def box_wrap(h, r):
    """Mean over a (2r+1) box, wrapping at the edges (summed-area table)."""
    p = np.pad(h, r + 1, mode="wrap").cumsum(0).cumsum(1); k = 2 * r + 1
    s = p[k:, k:] - p[:-k, k:] - p[k:, :-k] + p[:-k, :-k]
    return (s / (k * k))[:h.shape[0], :h.shape[1]]

# Mean colour (sRGB) a tile is pulled to, so the tracks' baked colours stay put.
MATCH = {"loosedirt": (131, 90, 57), "mud": (88, 62, 46)}

def periodic_noise(n, k, seed):
    """1D noise that wraps: a sum of whole-period sines."""
    r = np.random.RandomState(seed); x = np.arange(n) / n; v = np.zeros(n)
    for f in range(1, k + 1): v += r.randn() / f * np.sin(2 * np.pi * (f * x + r.rand()))
    return (v - v.min()) / (np.ptp(v) + 1e-9)

for name, (cid, tint, sat, con, extra) in G.items():
    SIDE = 2048
    def load(kind, mode="RGB"):
        f = os.path.join(SRC, cid, f"{cid}_2K-JPG_{kind}.jpg")
        if not os.path.exists(f): return None
        im = Image.open(f).convert(mode)
        if im.size != (SIDE, SIDE): im = im.resize((SIDE, SIDE), Image.LANCZOS)
        return np.asarray(im).astype(np.float32) / 255.0
    a = load("Color") ** 2.2  # work in linear light
    shade = np.ones(a.shape[:2], np.float32)
    ao = load("AmbientOcclusion", "L")
    if ao is not None: shade *= 0.55 + 0.45 * ao
    h = load("Displacement", "L")
    if h is not None:
        # cavities: below the local mean height reads darker (wraps, as the tile does)
        cav = h - box_wrap(h, 24)
        shade *= np.clip(1.0 + 0.8 * cav, 0.8, 1.08)
    n = load("NormalGL")
    if n is not None:
        n = n * 2 - 1; n /= np.linalg.norm(n, axis=2, keepdims=True) + 1e-6
        L = np.array([0.35, 0.45, 0.82]); L /= np.linalg.norm(L)
        d = np.clip((n * L).sum(2), 0, 1); d0 = float(L[2])
        shade *= 1.0 + 0.3 * (d - d0)
    if extra == "puddles" and h is not None:
        # wet mud: the low ground holds water, darker and smoother; ruts read stronger
        low = np.clip((box_wrap(h, 6) - h.mean()) * -6 + 0.1, 0, 1)
        low = box_wrap(low, 8)
        shade *= 1.0 - 0.38 * low
        if n is not None: shade *= 1.0 + 0.35 * (d - d0)
    shade = shade / shade.mean()
    a = (a * shade[..., None]) ** (1 / 2.2)
    if extra == "nogreen":
        # no plant bits on a race line: greenish pixels become the soil's brown at their own brightness
        g = np.clip((a[..., 1] - 0.84 * a[..., 0]) * 14, 0, 1)[..., None] * np.clip((a[..., 1] - 1.15 * a[..., 2]) * 8, 0, 1)[..., None]
        lum = a.mean(2, keepdims=True); brown = lum * np.array([1.25, 1.0, 0.72])
        a = a * (1 - g) + brown * g
    if extra == "straw":
        # dry summer grass: pull hue toward straw and lift it a little
        lum = a.mean(2, keepdims=True); a = lum * np.array([1.55, 1.25, 0.7]) * 1.2
    lum = a.mean(2, keepdims=True)
    a = lum + (a - lum) * sat
    m = a.mean((0, 1)); a = m + (a - m) * con
    a *= np.array(tint)
    if extra == "ruts":
        # faint tyre lines running along v: darker, smoother bands that wrap
        band = periodic_noise(2048, 24, hash(name) & 0xffff)
        mask = np.clip((band - 0.62) / 0.2, 0, 1)[None, :, None]
        a = a * (1 - 0.12 * mask) + 0.12 * mask * a.mean((0, 1))
    if extra == "wet":
        a = a ** 1.15
    if name in MATCH:
        want = (np.array(MATCH[name]) / 255.0) ** 2.2
        lin_ = np.clip(a, 0, 1) ** 2.2; a = np.clip(lin_ * (want / lin_.reshape(-1, 3).mean(0)), 0, 1) ** (1 / 2.2)
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, name + ".png"))
    print("ground", name, cid)
