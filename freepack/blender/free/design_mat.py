"""Material maps for a free bike: metal/roughness (G rough, B metal) and a tangent-space normal map, from the part each
texel belongs to (painted plastic, rubber, rims, spokes, cast engine, frame, chrome, seat) and 3D detail patterns.
usage: BAKE=dir python design_mat.py BIKE OUTDIR   (run after design_fp.py, which wrote OUTDIR/fp_livery_BIKE.mask.png)"""
import numpy as np, sys, os
from PIL import Image
from scipy import ndimage as nd
b, OUT = sys.argv[1], sys.argv[2]; D = os.environ.get('BAKE', '.') + f'/{b}/'
L = lambda n: np.load(D + n + '.npy').astype(np.float32)
X = L('pos'); names = open(D + 'names.txt').read().split(); ND = np.rint(L('node')[..., 0]).astype(int) % 100
cov = np.abs(X).sum(-1) > 1e-5
NR = L('nrm'); x, y, z = X[..., 0], X[..., 1], X[..., 2]; ax = np.abs(x)
M = np.asarray(Image.open(os.path.join(OUT, f'fp_livery_{b}.mask.png'))).astype(np.float32) / 255
base = np.asarray(Image.open(os.path.join(OUT, f'fp_livery_{b}.base.png'))).astype(np.float32) / 255
paint = np.clip(M[..., 0] + M[..., 1], 0, 1)
def ss(a, b_, t):
    t = np.clip((t - a) / (b_ - a), 0, 1); return t * t * (3 - 2 * t)
def band(t, lo, hi, e=0.004): return ss(lo - e, lo + e, t) * (1 - ss(hi - e, hi + e, t))
N = lambda n: (ND == names.index(n)).astype(np.float32) if n in names else np.zeros(x.shape, np.float32)
rng = np.random.default_rng(7); G = rng.random((96, 96, 96)).astype(np.float32)
def noise(f, oct=3):
    out = np.zeros(x.shape, np.float32); a = 1.0; tot = 0
    for o in range(oct):
        q = (X * f * 2 ** o) % 96
        out += a * nd.map_coordinates(G, [q[..., 0].ravel(), q[..., 1].ravel(), q[..., 2].ravel()], order=1, mode='wrap').reshape(x.shape)
        tot += a; a *= 0.5
    return out / tot
lum = (base / np.maximum(M[..., 3:4], 0.2)).mean(-1)                       # unshaded tone of the fixed parts
FA = (0.688, 0.343); RA = (-0.792, 0.3323)
tyre = np.zeros(x.shape, np.float32); rim = np.zeros_like(tyre)
for c, n in ((FA, 'TireFront'), (RA, 'TireRear')):
    w = N(n); dd = np.hypot(y - c[0], z - c[1])
    tyre += w * ss(0.280, 0.288, dd); rim += w * band(dd, 0.250, 0.283, 0.002)
wheel = N('TireFront') + N('TireRear')
seat = N('Bike') * band(y, -0.86, -0.03, 0.01) * ss(0.84, 0.87, z) * (1 - ss(0.15, 0.17, ax)) * ss(0.25, 0.4, NR[..., 2]) * (1 - paint)
fixed = (1 - paint)
seat = seat * (1 - ss(0.10, 0.16, lum))                                      # only the black cover
bright = ss(0.40, 0.60, lum) * fixed; dark = (1 - ss(0.08, 0.16, lum)) * fixed
# ---- roughness and metal per part
R = np.full(x.shape, 0.55, np.float32); Mt = np.zeros_like(R)
def put(r, rough, metal):
    global R, Mt
    R[:] = R * (1 - r) + rough * r; Mt[:] = Mt * (1 - r) + metal * r
put(fixed, 0.5, 0.6)                                                        # bare metal by default (frame, engine, bars)
put(dark, 0.62, 0.15)                                                       # black parts: coated / plastic
put(bright, 0.28, 0.95)                                                     # polished alloy, chrome, silencer
put(N('Bike') * band(y, -0.40, 0.42, 0.02) * (1 - ss(0.66, 0.72, z)) * fixed * (1 - bright), 0.55, 0.85)  # cast engine cases
put(wheel * (1 - tyre) * (1 - rim), 0.25, 1.0)                              # spokes, hubs
put(rim, 0.38, 0.9)                                                         # anodised rims
put(tyre, 0.86, 0.0)                                                        # rubber
put(seat, 0.72, 0.0)                                                        # seat cover
put(paint, 0.30, 0.0)                                                       # glossy plastics
R = R + (noise(40) - 0.5) * 0.08 * (1 - tyre)                               # a little variation everywhere
# ---- height detail (metres-ish units, small): grip seat, rubber, casting grain, plastic grain
H = np.zeros_like(R)
u, v = (y + 0.6 * x) / 0.012, (y - 0.6 * x) / 0.012                          # diamond seat grip
H += seat * ss(0.6, 0.8, NR[..., 2]) * (np.abs(np.sin(np.pi * u)) * np.abs(np.sin(np.pi * v))) ** 0.5 * 0.8
H += tyre * (noise(260, 2) - 0.5) * 0.5
H += fixed * (1 - tyre) * (1 - seat) * (1 - bright) * (noise(500, 2) - 0.5) * 0.35   # casting / coating grain
H += paint * (noise(120, 2) - 0.5) * 0.06
H += (1 - paint) * bright * (noise(900, 1) - 0.5) * 0.10                    # fine brushing
# tangent-space normal from the height's gradient in texture space, cut at island edges
hx = np.zeros_like(H); hy = np.zeros_like(H)
dxp = np.linalg.norm(X[:, 1:] - X[:, :-1], axis=-1); okx = cov[:, 1:] & cov[:, :-1] & (dxp < 0.01)
dyp = np.linalg.norm(X[1:] - X[:-1], axis=-1); oky = cov[1:] & cov[:-1] & (dyp < 0.01)
hx[:, :-1] = np.where(okx, H[:, 1:] - H[:, :-1], 0); hy[:-1] = np.where(oky, H[1:] - H[:-1], 0)
k = 2.2
nrm = np.stack([-hx * k, hy * k, np.ones_like(H)], -1)                     # image rows run down: v = -row
nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True)
nrm[~cov] = (0, 0, 1)
mr = np.stack([np.ones_like(R), np.clip(R, 0.05, 1), np.clip(Mt, 0, 1)], -1)
mr[~cov] = (1, 0.6, 0)
# bleed into the empty texels so mips and filtering stay clean
idx = nd.distance_transform_edt(~cov, return_distances=False, return_indices=True)
mr = mr[idx[0], idx[1]]; nrm = nrm[idx[0], idx[1]]
Image.fromarray((mr * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, f'fp_bike_{b}_mr.png'), optimize=True)
Image.fromarray(((nrm * 0.5 + 0.5) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, f'fp_bike_{b}_nrm.png'), optimize=True)
print('maps', b, 'metal', Mt[cov].mean().round(2), 'rough', R[cov].mean().round(2))
