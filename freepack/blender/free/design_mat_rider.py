"""Material maps for a rider from its paint masks and nodes (works in any pose): fabric kit, plastic boots and
armour (trim on the lower legs and feet), gloves, glossy helmet shell, goggles. Writes OUTDIR/<set>_mr.png, _nrm.png.
usage: BAKE=dir python design_mat_rider.py BAKENAME SET OUTDIR"""
import numpy as np, sys, os
from PIL import Image
from scipy import ndimage as nd
bk, SET, OUT = sys.argv[1:4]; D = os.environ.get('BAKE', '.') + f'/{bk}/'
L = lambda n: np.load(D + n + '.npy').astype(np.float32)
X = L('pos'); names = open(D + 'names.txt').read().split(); ND = np.rint(L('node')[..., 0]).astype(int) % 100
cov = np.abs(X).sum(-1) > 1e-5; x = X[..., 0]
Mk = np.asarray(Image.open(os.path.join(OUT, f'{SET}.mask.png')).convert('RGBA').resize(cov.shape[::-1])).astype(np.float32) / 255
base = np.asarray(Image.open(os.path.join(OUT, f'{SET}.base.png')).convert('RGB').resize(cov.shape[::-1])).astype(np.float32) / 255
P, S, Hm = Mk[..., 0], Mk[..., 1], Mk[..., 2]
def node(*ns): return np.clip(sum((ND == names.index(n)).astype(np.float32) for n in ns if n in names), 0, 1)
def ss(a, b, t):
    t = np.clip((t - a) / (b - a), 0, 1); return t * t * (3 - 2 * t)
rng = np.random.default_rng(5); G3 = rng.random((96, 96, 96)).astype(np.float32)
def noise(f, oct=3):
    out = np.zeros(x.shape, np.float32); a = 1.0; tot = 0
    for o in range(oct):
        q = (X * f * 2 ** o) % 96
        out += a * nd.map_coordinates(G3, [q[..., 0].ravel(), q[..., 1].ravel(), q[..., 2].ravel()], order=1, mode='wrap').reshape(x.shape)
        tot += a; a *= 0.5
    return out / tot
head = node('Head'); hands = node('HandL', 'HandR'); low = node('LwrLegL', 'LwrLegR', 'FootL', 'FootR')
lum = base.mean(-1); paint = np.clip(P + S, 0, 1)
shell = head * paint; goggle = head * (1 - paint) * (1 - ss(0.05, 0.15, lum))
skin = head * (1 - paint) * ss(0.15, 0.3, lum)
boots = low * ss(0.4, 0.6, S); fabric = (1 - head) * (1 - hands) * (1 - boots)
R = np.full(x.shape, 0.85, np.float32); Mt = np.zeros_like(R)
def pm(r, rough, metal):
    global R, Mt
    R[:] = R * (1 - r) + rough * r; Mt[:] = Mt * (1 - r) + metal * r
pm(fabric, 0.86, 0); pm(boots, 0.45, 0); pm(hands, 0.78, 0); pm(shell, 0.18, 0); pm(goggle, 0.22, 0.25); pm(skin, 0.6, 0)
R = R + (noise(30) - 0.5) * 0.06
H = fabric * (noise(45, 3) - 0.5) * 0.5 + fabric * (noise(700, 1) - 0.5) * 0.12 + hands * (noise(400, 2) - 0.5) * 0.5 + boots * (noise(90, 2) - 0.5) * 0.15
dxp = np.linalg.norm(X[:, 1:] - X[:, :-1], axis=-1); okx = cov[:, 1:] & cov[:, :-1] & (dxp < 0.01)
dyp = np.linalg.norm(X[1:] - X[:-1], axis=-1); oky = cov[1:] & cov[:-1] & (dyp < 0.01)
hx = np.zeros_like(H); hy = np.zeros_like(H)
hx[:, :-1] = np.where(okx, H[:, 1:] - H[:, :-1], 0); hy[:-1] = np.where(oky, H[1:] - H[:-1], 0)
nrm = np.stack([-hx * 2.2, hy * 2.2, np.ones_like(H)], -1); nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True); nrm[~cov] = (0, 0, 1)
mr = np.stack([np.ones_like(R), np.clip(R, 0.05, 1), np.clip(Mt, 0, 1)], -1); mr[~cov] = (1, 0.8, 0)
idx = nd.distance_transform_edt(~cov, return_distances=False, return_indices=True)
mr = mr[idx[0], idx[1]]; nrm = nrm[idx[0], idx[1]]
Image.fromarray((mr * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, f'{SET}_mr.png'), optimize=True)
Image.fromarray(((nrm * 0.5 + 0.5) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, f'{SET}_nrm.png'), optimize=True)
print('rider maps', SET)
