#!/usr/bin/env python3
"""Second half of blender/surface_bake.py: write each colourway's full atlas
from the baked layers (material detail, old-UV look-up, dust, ambient
occlusion) and that colourway's original livery or outfit picture.

    python tools/compose_atlas.py fp_bike_mx2 | fp_rider

Bikes share flat livery pictures (bikes/fp_livery_*.png) but each model
gets its own baked set in bikes/<model>/; the rider's outfits are
rewritten in place (their flat originals kept as *.src.png).
"""
import os, sys, glob
import numpy as np
from PIL import Image
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODEL = sys.argv[1]; KIND = "rider" if "rider" in MODEL else "bike"
DIR = os.path.join(HERE, "build", "bikes" if KIND == "bike" else "riders")
L = np.load(os.path.join(DIR, f".bake_{MODEL}.npy")).astype(np.float32)
col, uv, dust, ao = L[..., 0:3], L[..., 3:6], L[..., 6], L[..., 7]
S = col.shape[0]
pics = sorted(p for p in glob.glob(os.path.join(DIR, ("fp_livery_*.src.png" if KIND == "bike" else "fp_rider_*.src.png"))))
DUST = np.array([0.40, 0.33, 0.25]) if KIND == "bike" else np.array([0.42, 0.35, 0.27])
DUST_AMT = float(os.environ.get("FP_DUST", "0.45" if KIND == "bike" else "0.35"))

def sample(img, u, v):
    """Bilinear look-up of img (H, W, 3, linear) at glTF-style UVs (v up)."""
    h, w = img.shape[:2]
    x = np.clip(u * w - 0.5, 0, w - 1.001); y = np.clip((1 - v) * h - 0.5, 0, h - 1.001)
    x0 = x.astype(int); y0 = y.astype(int); fx = (x - x0)[..., None]; fy = (y - y0)[..., None]
    a = img[y0, x0]; b = img[y0, x0 + 1]; c = img[y0 + 1, x0]; d = img[y0 + 1, x0 + 1]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy

lin = lambda a: np.power(a, 2.2)

def infill(img, mask):
    """Fill the empty atlas between islands from the islands around it
    (pull-push over a mip pyramid), so the game's smaller mip levels, which
    blend neighbouring texels, never pull in black."""
    levels = [(img * mask[..., None], mask.astype(np.float32))]
    while levels[-1][1].shape[0] > 1:
        c, w = levels[-1]; h = c.shape[0] // 2
        c2 = c.reshape(h, 2, h, 2, 3).sum((1, 3)); w2 = w.reshape(h, 2, h, 2).sum((1, 3))
        levels.append((c2, w2))
    filled = levels[-1][0] / np.maximum(levels[-1][1], 1e-6)[..., None]
    for c, w in reversed(levels[:-1]):
        up = filled.repeat(2, 0).repeat(2, 1)
        own = c / np.maximum(w, 1e-6)[..., None]
        filled = np.where((w > 0)[..., None], own, up)
    return filled
tinted = uv[..., 2] > 0.5
covered = (ao > 0.004) | (col.sum(2) > 0.004)
occl = 0.42 + 0.58 * np.clip(ao, 0, 1) ** 0.8          # soft: the game's sun still shades the shape
d = np.clip(dust * DUST_AMT, 0, 0.8)[..., None]
for p in pics:
    src = lin(np.asarray(Image.open(p).convert("RGB")).astype(np.float32) / 255)
    pic = sample(src, uv[..., 0], uv[..., 1])
    # the bake's colour is sRGB-ish grey detail around 1 for tinted faces, a full colour for the rest
    base = np.where(tinted[..., None], pic * col, np.clip(col, 0, 1))
    out = base * (1 - d) + lin(DUST) * d
    out = out * occl[..., None]
    out = infill(out, covered)
    img = Image.fromarray((np.clip(out, 0, 1) ** (1 / 2.2) * 255 + 0.5).astype(np.uint8))
    if KIND == "bike":
        os.makedirs(os.path.join(DIR, MODEL[len("fp_bike_"):]), exist_ok=True)
        dst = os.path.join(DIR, MODEL[len("fp_bike_"):], f"fp_livery_{MODEL[len('fp_bike_'):]}_" + os.path.basename(p)[len("fp_livery_"):-len(".src.png")] + ".png")
    else:
        dst = p.replace(".src.png", ".png")
    img.save(dst, optimize=True)
    print("wrote", os.path.basename(dst))
