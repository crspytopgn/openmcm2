"""Generic bike paint: P paint slot (plastics), S secondary (plates/graphics), D dark plastic/rubber, metals. Writes bike_mask.png, bike_mat.png, samples."""
import numpy as np, sys
from PIL import Image
from scipy import ndimage as nd
sys.path.insert(0, '/home/user/work/hq')
X = np.load('pos.npy'); ND = np.rint(np.load('node.npy')[..., 0]).astype(int)
pr = np.load('probs.npy'); w = pr ** 4; w /= w.sum(2, keepdims=True) + 1e-6
ao = np.clip(np.load('ao.npy'), 0, 1)
x, y, z = X[..., 0], X[..., 1], X[..., 2]; ax = np.abs(x)
FA = (0.688, 0.343); RA = (-0.792, 0.3323)
def ss(a, b, t):
    t = np.clip((t - a) / (b - a), 0, 1); return t * t * (3 - 2 * t)
def band(t, lo, hi, e=0.004): return ss(lo - e, lo + e, t) * (1 - ss(hi - e, hi + e, t))
# layers: P paint, S white, D dark plastic, M metal (silver), K black metal (frame/engine dark), Rb rubber
L = {k: np.zeros(x.shape, np.float32) for k in ('P', 'S', 'D', 'M', 'K', 'Rb')}
rough = np.full(x.shape, 0.5, np.float32); metal = np.zeros_like(rough)
def put(region, k, r=None, m=None):
    region = region.astype(np.float32)
    for kk in L: L[kk] *= (1 - region)
    L[k] += region
    if r is not None: rough[:] = rough * (1 - region) + r * region
    if m is not None: metal[:] = metal * (1 - region) + m * region
put(np.ones_like(x), 'M', 0.35, 1.0)
put(w[..., 2], 'K', 0.55, 0.3)                                    # dark bits: frame/engine black parts
put(np.clip(w[..., 0] + w[..., 1], 0, 1) * np.isin(ND, [0, 1]), 'P', 0.55, 0.0)   # all plastics in the main colour
bodyzone = (ND == 0) * np.maximum(ss(0.70, 0.73, z), ss(0.0, 0.03, y) * ss(0.60, 0.63, z) * ss(0.11, 0.13, ax)) * (1 - ss(0.0, 0.05, -y - 1.15))
put(bodyzone * np.maximum(1 - ss(0.5, 0.7, w[..., 2]), ss(0.10, 0.12, ax)), 'P', 0.55, 0.0)
put(w[..., 1] * np.isin(ND, [2]), 'S', 0.55, 0.0)
# wheels: tyre rubber outside the rim, black anodised rims, steel spokes/hub
for c, nd_ in ((FA, 3), (RA, 6)):
    wh = (ND == nd_)
    dd = np.hypot(y - c[0], z - c[1])
    put(wh * ss(0.283, 0.290, dd), 'Rb', 0.85, 0.0)
    put(wh * band(dd, 0.250, 0.286, 0.002), 'K', 0.4, 0.8)
    put(wh * (1 - ss(0.250, 0.256, dd)), 'M', 0.3, 1.0)
# seat: dark grippy cover on top of the rear bodywork
seat = (ND == 0) * band(y, -0.78, 0.12, 0.01) * ss(0.93, 0.96, z) * (1 - ss(0.17, 0.19, ax))
put(seat * ss(0.3, 0.6, w[..., 2] + w[..., 0] + w[..., 1]), 'D', 0.9, 0.0)
# number plates: front plate (on the bars node, ahead of the fork) and rear side panels -> clean white
HPy, HPz = 0.283, 1.103
u = np.array([0, HPy - FA[0], HPz - FA[1]]); Lf = np.linalg.norm(u); u /= Lf
ry, rz = y - FA[0], z - FA[1]; t = ry * u[1] + rz * u[2]
perp = ry * u[2] - rz * u[1]                                    # >0 = ahead of the fork axis
fork_r = np.hypot(perp, 0) 
front_plate = (ND == 1) * ss(0.06, 0.09, perp) * (1 - ss(0.17, 0.19, perp)) * ss(0.88, 0.91, z) * band(t, 0.48, Lf + 0.10, 0.01) * (1 - ss(0.15, 0.17, ax))
put(front_plate, 'S', 0.55, 0.0)
tubes = np.isin(ND, [1, 2]) * (1 - ss(0.07, 0.09, np.abs(perp))) * band(t, 0.36, Lf - 0.06, 0.01) * band(ax, 0.04, 0.115, 0.004) * (1 - front_plate)
put(tubes, 'M', 0.25, 1.0)
side_panel = (ND == 0) * band(y - 0.35 * (z - 0.62), -0.70, -0.30, 0.006) * band(z, 0.64, 0.92, 0.006) * ss(0.08, 0.11, ax)
put(side_panel, 'S', 0.55, 0.0)
put(side_panel * band(z - 0.25 * (y + 0.5), 0.86, 0.88, 0.003), 'P', 0.35, 0.0)
# generic graphics on the shrouds/tank sides: a swoosh band in S with a D pin line
shroud = (ND == 0) * band(y, -0.05, 0.42, 0.01) * band(z, 0.70, 1.05, 0.01) * ss(0.12, 0.15, ax)
sw = band(z - 0.55 * (y - 0.1), 0.80, 0.86, 0.004)
put(shroud * sw, 'S', 0.55, 0.0)
put(shroud * band(z - 0.55 * (y - 0.1), 0.865, 0.875, 0.002), 'K', 0.4, 0.0)
# fender tips + rear fender stripe in the second colour
fender = (ND == 1) * ss(0.36, 0.40, y) * ss(0.74, 0.77, z) * (1 - front_plate) * (1 - ss(0.12, 0.13, ax))
put(fender, 'P', 0.55, 0.0)
put(fender * ss(0.97, 0.99, y), 'S', 0.55, 0.0)
put((ND == 0) * ss(0.4, 0.6, w[..., 0] + w[..., 1]) * (1 - ss(-1.10, -1.08, y)) * band(ax, 0.0, 0.025, 0.003) * ss(0.9, 0.95, z), 'S', 0.55, 0.0)
# fork guards and lower fork legs
forkz = np.isin(ND, [1, 2]) * (1 - ss(0.13, 0.15, np.abs(perp))) * band(ax, 0.03, 0.17, 0.004) * (1 - front_plate) * (1 - fender)
put(forkz * ss(0.04, 0.07, t) * (1 - ss(0.40, 0.43, t)), 'S', 0.5, 0.0)
put(forkz * ss(0.40, 0.43, t) * (1 - ss(Lf - 0.08, Lf - 0.05, t)), 'M', 0.25, 1.0)
M = np.stack([L[k] for k in ('P', 'S', 'D', 'M', 'K', 'Rb')], -1); M /= np.maximum(M.sum(-1, keepdims=True), 1e-4)
np.save('layers.npy', M.astype(np.float16))
aoc = ao ** 0.5
Image.fromarray((np.stack([rough, metal, aoc], -1) * 255).astype(np.uint8)).save('bike_mat.png')
lum = np.load('lum.npy')
detail = np.clip(nd.gaussian_filter(lum, 0.7) / np.maximum(nd.gaussian_filter(lum, 6), 0.05), 0.75, 1.2)   # keep fine metal detail only
def paint(cp, cs, out):
    cols = [cp, cs, (0.03, 0.03, 0.035), (0.62, 0.62, 0.64), (0.06, 0.06, 0.065), (0.025, 0.025, 0.025)]
    col = sum(M[..., i, None] * np.array(c) for i, c in enumerate(cols))
    col = col * (0.4 + 0.6 * aoc)[..., None]
    col = col * (1 + (detail[..., None] - 1) * (M[..., 3:5].sum(-1, keepdims=True)))
    Image.fromarray((np.clip(col, 0, 1) * 255).astype(np.uint8)).save(out)
paint((0.04, 0.22, 0.72), (0.90, 0.90, 0.88), 'bike_blue.png')
paint((0.95, 0.42, 0.02), (0.06, 0.06, 0.07), 'bike_orange.png')
paint((0.80, 0.06, 0.05), (0.90, 0.90, 0.88), 'bike_red.png')
print('done')
# runtime recolour maps: mask (R main, G second), fixed colour of everything else (AO baked in)
Image.fromarray((np.stack([M[..., 0], M[..., 1], np.zeros_like(M[..., 0])], -1) * 255).astype(np.uint8)).save('bike_mask.png')
fx = (M[..., 2, None] * np.array((0.03, 0.03, 0.035)) + M[..., 3, None] * np.array((0.62, 0.62, 0.64)) + M[..., 4, None] * np.array((0.06, 0.06, 0.065)) + M[..., 5, None] * np.array((0.025, 0.025, 0.025)))
fx = fx * (0.4 + 0.6 * aoc)[..., None] * (1 + (detail[..., None] - 1) * (M[..., 3:5].sum(-1, keepdims=True)))
Image.fromarray((np.clip(np.concatenate([fx, aoc[..., None]], -1), 0, 1) * 255).astype(np.uint8)).save('bike_fixed.png')
print('maps')
