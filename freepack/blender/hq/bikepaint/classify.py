import numpy as np
from PIL import Image
from scipy import ndimage as nd
R = 2048
base = np.asarray(Image.open('tex_Image_0.png').convert('RGB').resize((R, R), Image.LANCZOS)).astype(np.float32) / 255
P = np.load('pos.npy'); N = np.load('nrm.npy')
r, g, b = base[..., 0], base[..., 1], base[..., 2]
mx = base.max(2); mn = base.min(2); sat = (mx - mn) / (mx + 1e-4); lum = 0.2126 * r + 0.7152 * g + 0.0722 * b
# 0 paint (blue/purple), 1 white, 2 dark, 3 metal grey
blue = (b > r * 1.25) & (b > g * 1.2) & (sat > 0.3)
cls = np.where(blue, 0, np.where(lum > 0.62, 1, np.where(lum < 0.12, 2, 3)))
print('share', [round(float((cls == k).mean()), 3) for k in range(4)])
VOX = 0.006
lo = P.reshape(-1, 3).min(0) - 4 * VOX
idx = ((P - lo) / VOX).astype(np.int32); dims = idx.reshape(-1, 3).max(0) + 5
A = np.abs(N); dbin = A.argmax(2) * 2 + (np.take_along_axis(N, A.argmax(2)[..., None], 2)[..., 0] > 0)
out = np.zeros((R, R, 4), np.float32)
for d in range(6):
    sel = dbin == d
    ii = idx[sel]; flat = np.ravel_multi_index((ii[:, 0], ii[:, 1], ii[:, 2]), dims)
    W = nd.gaussian_filter(np.bincount(flat, minlength=int(np.prod(dims))).reshape(dims).astype(np.float32), 1.5)
    fc = ((P[sel] - lo) / VOX - 0.5).T
    wn = np.maximum(nd.map_coordinates(W, fc, order=1), 1e-6)
    for k in range(4):
        H = nd.gaussian_filter(np.bincount(flat, weights=(cls[sel] == k).astype(np.float32), minlength=int(np.prod(dims))).reshape(dims), 1.5)
        out[sel, k] = nd.map_coordinates(H, fc, order=1) / wn
np.save('probs.npy', out); np.save('cls.npy', cls.astype(np.uint8)); np.save('lum.npy', lum)
