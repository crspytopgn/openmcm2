"""Clean generic paint for a free bike on its new UVs: painted plastics (where the old liveries differed) get a solid main colour
with white number boards, a shroud swoosh and fender tips; the rest is re-toned (rubber, black rims, satin engine, steel) with AO.
Writes 12 liveries + <set>.mask.png/.base.png. usage: python design_fp.py BIKE OUTDIR"""
import numpy as np, sys, os
from PIL import Image
b, OUT = sys.argv[1], sys.argv[2]; D = os.environ.get('BAKE', '.') + f'/{b}/'
os.makedirs(OUT, exist_ok=True)
L = lambda n: np.load(D + n + '.npy').astype(np.float32)
X = L('pos'); names = open(D + 'names.txt').read().split(); NDr = np.rint(L('node')[..., 0]).astype(int); HOT = (NDr >= 200).astype(np.float32); PLATE = ((NDr % 200) >= 100).astype(np.float32); ND = NDr % 100
cov = np.abs(X).sum(-1) > 1e-5
old = {c: np.clip(L(c), 0, 1) for c in ('blue', 'red', 'white', 'black')}
diff = np.maximum(np.abs(old['blue'] - old['red']).max(-1), np.abs(old['white'] - old['black']).max(-1))
ao = np.clip(L('ao')[..., 0], 0, 1)
x, y, z = X[..., 0], X[..., 1], X[..., 2]; ax = np.abs(x)
def ss(a, b_, t):
    t = np.clip((t - a) / (b_ - a), 0, 1); return t * t * (3 - 2 * t)
def band(t, lo, hi, e=0.004): return ss(lo - e, lo + e, t) * (1 - ss(hi - e, hi + e, t))
N = lambda n: (ND == names.index(n)) if n in names else np.zeros_like(cov)
FA = (0.688, 0.343); RA = (-0.792, 0.3323); HPy, HPz = 0.283, 1.103
from scipy import ndimage as nd
painted = HOT * cov
sidep = N('Bike') * band(y, -0.85, -0.25, 0.01) * band(z, 0.58, 0.92, 0.01) * ss(0.075, 0.09, ax)
if b == 'enduro': sidep = sidep * (1 - ss(0.75, 0.77, z) * (1 - ss(-0.44, -0.41, y)))
painted = np.maximum(painted, (sidep > 0.5) * HOT * cov)
# plain white bits left over inside the plastic zones (shrouds, fenders) are plastic too
wl = old['white']; brightfix = ss(0.50, 0.60, wl.mean(-1)) * (1 - ss(0.08, 0.14, wl.max(-1) - wl.min(-1)))
zones = N('Bike') * band(y, -0.08, 0.45, 0.01) * band(z, 0.60, 1.08, 0.01) * ss(0.07, 0.09, ax)
zones = np.maximum(zones, N('Bike') * (1 - ss(-0.86, -0.80, y)) * ss(0.80, 0.84, z) * (1 - ss(0.12, 0.14, ax)))
zones = np.maximum(zones, N('HandleBars') * ss(0.45, 0.50, y))
zones = np.maximum(zones, N('Bike') * band(y, -0.85, -0.25, 0.01) * band(z, 0.58, 0.92, 0.01) * ss(0.075, 0.09, ax) * (sidep > 0.5))
painted = np.maximum(painted, (nd.grey_closing(brightfix * zones, size=3) > 0.5) * cov)
# make it symmetric: a texel is plastic if its mirror image (-x) on the same node is plastic
from scipy.spatial import cKDTree
pm = (painted > 0.5) & cov
for k in range(len(names)):
    src = pm & (ND == k); dst = cov & (ND == k) & ~pm
    if src.sum() < 50 or dst.sum() == 0: continue
    t = cKDTree(X[src]); q = X[dst].copy(); q[:, 0] *= -1
    dd, _ = t.query(q, distance_upper_bound=0.025)
    m = np.zeros_like(pm); m[dst] = np.isfinite(dd)
    pm |= m
painted = nd.gaussian_filter(pm.astype(np.float32), 0.6) * cov
lum = old['white'].mean(-1)
# --- fixed parts
base = old['white'].copy()
g = base.mean(-1, keepdims=True); base = g + (base - g) * 0.25                       # calm the old colours (no khaki blotches)
detail = np.clip(nd.gaussian_filter(lum, 0.8) / np.maximum(nd.gaussian_filter(lum, 5), 0.04), 0.7, 1.3)[..., None]
engine = N('Bike') * band(y, -0.40, 0.42, 0.02) * (1 - ss(0.66, 0.72, z)) * (1 - painted)
base = base * (1 - engine[..., None]) + np.array([0.20, 0.20, 0.21]) * detail * engine[..., None]
for c, n in ((FA, 'TireFront'), (RA, 'TireRear')):
    wh = N(n); dd = np.hypot(y - c[0], z - c[1])
    tyre = wh * ss(0.280, 0.288, dd); rim = wh * band(dd, 0.250, 0.283, 0.002)
    base = base * (1 - tyre[..., None]) + np.array([0.035, 0.035, 0.037]) * detail * tyre[..., None]
    base = base * (1 - rim[..., None]) + np.array([0.05, 0.05, 0.055]) * rim[..., None]
NRM = L('nrm')
seat = N('Bike') * band(y, -0.86, -0.03, 0.01) * ss(0.84, 0.87, z) * (1 - ss(0.15, 0.17, ax)) * ss(0.25, 0.4, NRM[..., 2])
seat = (nd.grey_closing(seat, size=5) > 0.5).astype(np.float32)
painted = painted * (1 - seat)
base = base * (1 - seat[..., None]) + np.array([0.035, 0.035, 0.04]) * seat[..., None]
# --- paint layers inside the painted plastics: main P, white boards S
S = np.zeros_like(lum)
u = np.array([0, HPy - FA[0], HPz - FA[1]]); Lf = np.linalg.norm(u); u /= Lf
ry, rz = y - FA[0], z - FA[1]; t = ry * u[1] + rz * u[2]; perp = ry * u[2] - rz * u[1]
front_plate = N('HandleBars') * ss(0.05, 0.08, perp) * ss(0.85, 0.89, z)
side = N('Bike') * band(y - 0.35 * (z - 0.62), -0.72, -0.30, 0.008) * band(z, 0.62, 0.90, 0.008) * ss(0.07, 0.10, ax)
BOARD = np.maximum(front_plate, side)
shroud = N('Bike') * band(y, -0.05, 0.42, 0.01) * band(z, 0.68, 1.05, 0.01) * ss(0.10, 0.13, ax)
S = np.maximum(S, shroud * band(z - 0.55 * (y - 0.1), 0.80, 0.86, 0.004) * ss(0.35, 0.55, np.abs(L('nrm')[..., 0])))
fender_tip = N('HandleBars') * ss(0.97, 1.0, y)
rear_tip = N('Bike') * (1 - ss(-1.12, -1.08, y))
S = np.maximum(S, np.maximum(fender_tip, rear_tip))
fork_guard = N('Shocks') * band(t, 0.04, 0.40, 0.01)
S = np.maximum(S, fork_guard)
BRD = dict(mx2=(-0.68, -0.34, 0.64, 0.84), enduro=(-0.66, -0.42, 0.68, 0.79))[b]
board = N('Bike') * band(y, BRD[0], BRD[1], 0.006) * band(z - 0.18 * (y - BRD[1]), BRD[2], BRD[3], 0.006) * ss(0.075, 0.09, ax)
S = np.maximum(S, np.maximum(PLATE * N('HandleBars'), board)) * painted; P = painted * (1 - S)
aos = nd.gaussian_filter(ao, 1.5)
shade = (0.42 + 0.58 * aos ** 0.6)[..., None]                               # fixed parts
shade_p = (0.72 + 0.28 * aos ** 0.8)[..., None]                             # glossy paint shows little AO
fixed = np.clip(base * shade, 0, 1) * (1 - painted[..., None])
# ---- a ridden bike, kept subtle: slight tone drift in the paint, boot scuffs on the side plastics, dust low down
rng = np.random.default_rng(11); G3 = rng.random((96, 96, 96)).astype(np.float32)
def noise(f, oct=3, stretch=(1, 1, 1)):
    out = np.zeros(x.shape, np.float32); a = 1.0; tot = 0
    for o in range(oct):
        q = (X * np.array(stretch, np.float32) * f * 2 ** o) % 96
        out += a * nd.map_coordinates(G3, [q[..., 0].ravel(), q[..., 1].ravel(), q[..., 2].ravel()], order=1, mode='wrap').reshape(x.shape)
        tot += a; a *= 0.5
    return out / tot
drift = 1 + (noise(6) - 0.5) * 0.06
boot = N('Bike') * ss(0.08, 0.11, ax) * band(z, 0.48, 0.80, 0.03) * band(y, -0.55, 0.30, 0.05)
streak = ss(0.62, 0.80, noise(260, 2, (0.15, 1.0, 0.15)))                    # fine scratches running fore and aft
wear = painted * boot * streak * 0.22
dust = (1 - ss(0.20, 0.60, z)) * ss(0.35, 0.75, noise(30)) * 0.14
dustc = np.array([0.42, 0.36, 0.29])
shade_p = shade_p * drift[..., None] * (1 - wear - dust)[..., None]
paint_add = (wear * 0.70)[..., None] + dust[..., None] * dustc
fixed = fixed * (1 - dust[..., None]) + (dust[..., None] * dustc * shade) * (1 - painted[..., None])
fixed = fixed + paint_add * painted[..., None]
mask = np.stack([P, S, np.zeros_like(P), shade_p[..., 0]], -1)
def save(a, name, mode='RGB'):
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), mode).save(os.path.join(OUT, name), optimize=True)
save(mask, f'fp_livery_{b}.mask.png', 'RGBA'); save(fixed, f'fp_livery_{b}.base.png')
PAL = {'black': (0.08, 0.08, 0.09), 'blue': (0.06, 0.25, 0.78), 'green': (0.05, 0.45, 0.15), 'grey': (0.42, 0.43, 0.45),
       'lime': (0.55, 0.80, 0.05), 'navy': (0.05, 0.10, 0.32), 'orange': (0.95, 0.42, 0.02), 'purple': (0.38, 0.10, 0.62),
       'red': (0.80, 0.07, 0.06), 'teal': (0.0, 0.50, 0.52), 'white': (0.92, 0.92, 0.90), 'yellow': (0.98, 0.80, 0.05)}
WHITE, BLACK = (0.90, 0.90, 0.88), (0.06, 0.06, 0.07)
for n, c in PAL.items():
    tr = BLACK if n in ('white', 'yellow', 'lime') else WHITE
    col = (P[..., None] * np.array(c) + S[..., None] * np.array(tr)) * shade_p + fixed
    save(col, f'fp_livery_{b}_{n}.png')
print('done', b, 'painted', painted[cov].mean().round(3))
