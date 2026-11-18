"""Classify the TRELLIS rider texture into panel classes, clean them in 3D (removes logos/text), write mask + shade maps."""
import numpy as np, sys
from PIL import Image
from scipy import ndimage as nd
R = 2048
base = np.asarray(Image.open('/home/user/work/trellis/tex_Image_0.png').convert('RGB').resize((R, R), Image.LANCZOS)).astype(np.float32) / 255
P = np.load('Position.npy'); N = np.load('Normal.npy')
r, g, b = base[..., 0], base[..., 1], base[..., 2]
mx = base.max(2); mn = base.min(2); sat = (mx - mn) / (mx + 1e-4); lum = 0.2126 * r + 0.7152 * g + 0.0722 * b
# classes: 0 primary (red), 1 white, 2 black, 3 skin (kept as is)
red = (r > g * 1.6) & (r > b * 1.4) & (sat > 0.45)
skin = (r > g) & (g > b) & (sat > 0.18) & (sat < 0.55) & (mx > 0.55) & ((r - g) < 0.35) & ~red
cls = np.where(red, 0, np.where(lum > 0.42, 1, 2)); cls[skin] = 3
print('class share', [round(float((cls == k).mean()), 3) for k in range(4)])
# 3D voxel histogram per normal direction bin
VOX = 0.0034
lo = P.reshape(-1, 3).min(0) - 4 * VOX
idx = ((P - lo) / VOX).astype(np.int32)
dims = idx.reshape(-1, 3).max(0) + 5
A = np.abs(N); dbin = A.argmax(2) * 2 + (np.take_along_axis(N, A.argmax(2)[..., None], 2)[..., 0] > 0)
out = np.zeros((R, R, 4), np.float32)
SIG = float(sys.argv[1]) if len(sys.argv) > 1 else 2.2       # blur in voxels (~ text stroke killer)
for d in range(6):
    sel = dbin == d
    if not sel.any(): continue
    ii = idx[sel]
    coords = (ii[:, 0] - 0, ii[:, 1], ii[:, 2])
    W = nd.gaussian_filter(np.bincount(np.ravel_multi_index(coords, dims), minlength=int(np.prod(dims))).reshape(dims).astype(np.float32), SIG)
    fc = (P[sel] - lo) / VOX - 0.5
    for k in range(4):
        H = np.bincount(np.ravel_multi_index(coords, dims), weights=(cls[sel] == k).astype(np.float32), minlength=int(np.prod(dims))).reshape(dims)
        H = nd.gaussian_filter(H, SIG)
        out[sel, k] = nd.map_coordinates(H, fc.T, order=1) / np.maximum(nd.map_coordinates(W, fc.T, order=1), 1e-6)
    print('dir', d, int(sel.sum()))
np.save('probs.npy', out)
np.save('cls_raw.npy', cls.astype(np.uint8)); np.save('lum.npy', lum)
