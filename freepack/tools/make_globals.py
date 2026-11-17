#!/usr/bin/env python3
"""New art for the small stock textures in GLOBAL.RES that tracks name:
the terrain detail noise (GrayNoise, whitenoise and per-track noise names)
and the dirt particle sheets (TrackNNP, Sand, Snow, BajaDirt, DirtPart).

Particle sheets keep the stock cell layout (mm2-viewer particles.rs
cell_uv): 14 clods in a 4x4 grid of quarter cells, four 1/8 puffs, then
16 chunks and 16 sparks in 1/16 cells along the bottom row. Drawn at 256
pixels (the cells are fractions, so any size works). Writes build/global/.
"""
import os, numpy as np
from PIL import Image, ImageFilter
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__))); OUT = os.path.join(HERE, "build", "global")
rng = np.random.default_rng(7)

def tile_noise(n, octaves, seed):
    r = np.random.default_rng(seed); acc = np.zeros((n, n)); amp = 1.0; tot = 0
    for o in range(octaves):
        f = 4 * 2 ** o
        g = r.standard_normal((f, f))
        img = Image.fromarray(((g - g.min()) / (np.ptp(g) + 1e-9) * 255).astype(np.uint8)).resize((n, n), Image.BICUBIC)
        # wrap-safe: blend with a rolled copy so edges tile
        a = np.asarray(img, np.float32) / 255
        acc += amp * a; tot += amp; amp *= 0.55
    acc /= tot
    acc = (acc + np.roll(acc, n // 2, 0) + np.roll(acc, n // 2, 1) + np.roll(np.roll(acc, n // 2, 0), n // 2, 1)) / 4
    return (acc - acc.mean()) / (acc.std() + 1e-9)

def detail(name, contrast, seed):
    z = tile_noise(256, 6, seed)
    v = np.clip(128 + z * contrast, 0, 255).astype(np.uint8)
    Image.fromarray(v).convert("RGB").save(os.path.join(OUT, name + ".png"))

def blob(size, rgb, soft=0.35, seed=0):
    r = np.random.default_rng(seed); y, x = np.mgrid[0:size, 0:size] / (size - 1) * 2 - 1
    ang = np.arctan2(y, x); rad = np.hypot(x, y)
    edge = 0.72 + sum(r.uniform(-1, 1) * 0.12 / k * np.cos(k * ang + r.uniform(0, 6.3)) for k in range(1, 7))
    a = np.clip((edge - rad) / soft, 0, 1)
    shade = 0.8 + 0.3 * (-(x * 0.4 + y * 0.6)) + 0.18 * r.standard_normal((size, size))
    c = np.clip(np.array(rgb)[None, None, :] * shade[..., None], 0, 1)
    return np.concatenate([c, a[..., None]], -1)

def sheet(name, rgb, puff_rgb, spark=(1.0, 0.72, 0.35), S=256):
    im = np.zeros((S, S, 4), np.float32)
    q, e, s = S // 4, S // 8, S // 16
    for c in range(14):
        tint = np.array(rgb) * rng.uniform(0.8, 1.1)
        im[(c // 4) * q:(c // 4 + 1) * q, (c % 4) * q:(c % 4 + 1) * q] = blob(q, tint, 0.15, c)
    for k in range(4):
        y0, x0 = int(0.75 * S) + (k // 2) * e, int(0.25 * S) + (k % 2) * e
        b = blob(e, puff_rgb, 0.9, 50 + k); b[..., 3] *= 0.35
        im[y0:y0 + e, x0:x0 + e] = b
    for k in range(16):
        y0, x0 = int(0.75 * S) + (k // 4) * s, int(0.5 * S) + (k % 4) * s
        im[y0:y0 + s, x0:x0 + s] = blob(s, np.array(rgb) * 0.8, 0.3, 100 + k)
        y0, x0 = int(0.75 * S) + (k // 4) * s, int(0.75 * S) + (k % 4) * s
        im[y0:y0 + s, x0:x0 + s] = blob(s, spark, 0.6, 200 + k)
    Image.fromarray((im * 255 + 0.5).astype(np.uint8), "RGBA").save(os.path.join(OUT, name + ".png"))

if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    detail("graynoise", 22, 1); detail("whitenoise", 14, 2)
    for i, n in enumerate(["mxunat03noise", "mxunat13noise", "mxusx16noise"]): detail(n, 20, 10 + i)
    dirt = {"track01p": (0.36, 0.26, 0.17), "track02p": (0.33, 0.24, 0.16), "track03p": (0.40, 0.30, 0.20), "track04p": (0.30, 0.22, 0.15),
            "track05p": (0.38, 0.28, 0.18), "track06p": (0.35, 0.27, 0.19), "track07p": (0.32, 0.25, 0.18), "track11p": (0.42, 0.32, 0.22),
            "track13p": (0.29, 0.21, 0.14), "track14p": (0.37, 0.29, 0.21), "track15p": (0.34, 0.25, 0.17), "track16p": (0.39, 0.29, 0.19),
            "dirtpart": (0.35, 0.26, 0.18), "bajadirt": (0.55, 0.42, 0.28), "sand": (0.66, 0.56, 0.40), "snow": (0.88, 0.9, 0.93)}
    for n, rgb in dirt.items():
        sheet(n, rgb, tuple(min(1, c * 1.5 + 0.15) for c in rgb))
    print(len(dirt) + 5, "textures")
