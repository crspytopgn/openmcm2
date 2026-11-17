#!/usr/bin/env python3
"""Turn the CC0 HDRI skies (sources.toml [sky]) into the six cube faces the
engine draws for each stock sky name, as soft, low-contrast 8-bit PNGs.

Faces follow the engine's sky box (mm2-viewer sky.rs `corner`), in game
coordinates (Y up): 0 +Z, 1 -X, 2 -Z, 3 +X, 4 up, 5 down. Files are named
by the high-res pack's sky keys: sky-<name>-<face>.png.

Usage: python3 tools/make_skies.py [--side 1024] [--preview]
Needs numpy and opencv-python-headless.
"""
import os, sys, tomllib
import numpy as np, cv2

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(HERE, "build", "skies")

def face_dirs(face, side):
    px = (np.arange(side) + 0.5) / side
    u, v = np.meshgrid(2 * px - 1, 1 - 2 * px)
    one = np.ones_like(u)
    d = {0: (u, v, one), 1: (-one, v, u), 2: (-u, v, -one),
         3: (one, v, -u), 4: (u, one, -v), 5: (u, -one, v)}[face]
    d = np.stack(d, -1)
    return d / np.linalg.norm(d, axis=-1, keepdims=True)

def sample(hdr, d, yaw):
    h, w, _ = hdr.shape
    lon = np.arctan2(d[..., 0], d[..., 2]) + yaw
    lat = np.arcsin(np.clip(d[..., 1], -1, 1))
    x = ((lon / (2 * np.pi) + 0.5) % 1.0) * w - 0.5
    y = (0.5 - lat / np.pi) * h - 0.5
    return cv2.remap(hdr, x.astype(np.float32), y.astype(np.float32), cv2.INTER_LINEAR, borderMode=cv2.BORDER_WRAP)

def tone(rgb, key):
    lum = rgb @ np.array([0.2126, 0.7152, 0.0722], np.float32)
    rgb = rgb * key
    # soft shoulder, no hard clip on the sun, low contrast
    # Reinhard with a white point: the sun rolls off instead of glaring
    wp = 6.0
    out = rgb * (1 + rgb / (wp * wp)) / (1 + rgb)
    grey = (out @ np.array([0.2126, 0.7152, 0.0722], np.float32))[..., None]
    out = grey + (out - grey) * 0.9
    out = 0.04 + out * 0.94  # lift blacks a touch
    return np.clip(out, 0, 1) ** (1 / 2.2)

def build(name, hdr_path, side):
    hdr = cv2.imread(hdr_path, cv2.IMREAD_UNCHANGED)[..., ::-1].astype(np.float32)
    h, w, _ = hdr.shape
    # exposure from the sky above the horizon (ignore the sun's top 0.5%)
    upper = hdr[: h // 2 - h // 60]
    lum = upper @ np.array([0.2126, 0.7152, 0.0722], np.float32)
    key = 0.42 / max(np.percentile(lum, 50), 1e-4)
    # below the horizon: the haze colour just above it, darkening a little
    band = hdr[h // 2 - h // 40 : h // 2 - h // 200].reshape(-1, 3)
    haze = np.median(band, 0)
    faces = []
    for f in range(6):
        d = face_dirs(f, side)
        c = sample(hdr, d, 0.0)
        below = np.clip(-d[..., 1] / 0.06, 0, 1)[..., None]
        ground = haze * (1 - 0.25 * np.clip(-d[..., 1], 0, 1)[..., None])
        c = c * (1 - below) + ground * below
        faces.append((tone(c, key) * 255 + 0.5).astype(np.uint8))
    return faces

if __name__ == "__main__":
    side = int(sys.argv[sys.argv.index("--side") + 1]) if "--side" in sys.argv else 1024
    with open(os.path.join(HERE, "sources.toml"), "rb") as f: src = tomllib.load(f)
    os.makedirs(OUT, exist_ok=True)
    for name, asset in src["sky"].items():
        faces = build(name, os.path.join(HERE, "cache", "hdri", f"{asset}_2k.hdr"), side)
        for i, img in enumerate(faces):
            cv2.imwrite(os.path.join(OUT, f"sky-{name.lower()}-{i}.png"), img[..., ::-1])
        if "--preview" in sys.argv:
            s = 256; sm = [cv2.resize(x, (s, s), interpolation=cv2.INTER_AREA) for x in faces]
            strip = np.concatenate([sm[1], sm[0], sm[3], sm[2]], 1)  # -X +Z +X -Z round the horizon
            cv2.imwrite(os.path.join(OUT, f"preview-{name.lower()}.jpg"), strip[..., ::-1])
        print("sky", name)
