"""Clean generic MX kit for the free rider on its new UVs, painted by rules in 3D (rest pose, metres; +y forward, z up).
Mask: R main, G trim, B helmet (shell takes the helmet colour), A shade. Fixed (base): dark parts, goggles, skin.
usage: python design_rider.py OUTDIR"""
import numpy as np, sys, os
from PIL import Image
from scipy import ndimage as nd
OUT = sys.argv[1]; D = os.environ.get('BAKE', '.') + '/rider/'; os.makedirs(OUT, exist_ok=True)
L = lambda n: np.load(D + n + '.npy').astype(np.float32)
X = L('pos'); names = open(D + 'names.txt').read().split(); ND = np.rint(L('node')[..., 0]).astype(int) % 100
cov = (np.abs(X).sum(-1) > 1e-5).astype(np.float32)
old = {c: np.clip(L(c), 0, 1) for c in ('blue', 'red', 'white', 'black')}
diff = np.maximum(np.abs(old['blue'] - old['red']).max(-1), np.abs(old['white'] - old['black']).max(-1))
ao = nd.gaussian_filter(np.clip(L('ao')[..., 0], 0, 1), 1.2)
NR = L('nrm')
x, y, z = X[..., 0], X[..., 1], X[..., 2]; ax = np.abs(x)
def ss(a, b, t):
    t = np.clip((t - a) / (b - a), 0, 1); return t * t * (3 - 2 * t)
def band(t, lo, hi, e=0.004): return ss(lo - e, lo + e, t) * (1 - ss(hi - e, hi + e, t))
def node(*ns): return np.clip(sum((ND == names.index(n)).astype(np.float32) for n in ns if n in names), 0, 1) * cov
P = np.zeros(x.shape, np.float32); S = np.zeros_like(P); Dk = np.zeros_like(P); H = np.zeros_like(P)
def put(r, p=0, s=0, d=0, h=0):
    global P, S, Dk, H
    r = np.clip(r, 0, 1).astype(np.float32)
    P = P * (1 - r) + p * r; S = S * (1 - r) + s * r; Dk = Dk * (1 - r) + d * r; H = H * (1 - r) + h * r
front = ss(-0.10, -0.06, y)      # chest side of the torso
# ---- jersey
tor = node('UprTorso', 'LwrTorso')
put(tor, p=1)
put(tor * ss(0.135, 0.15, ax) * band(z, 0.98, 1.40, 0.01), d=1)                    # dark side panels
put(tor * ss(1.465, 1.475, z) * (1 - ss(0.10, 0.12, ax)), d=1)                      # collar
put(tor * front * band(z - 0.55 * ax, 1.27, 1.31, 0.003), s=1)                      # chest chevron
put(tor * front * band(z - 0.55 * ax, 1.315, 1.325, 0.002), d=1)
put(tor * ss(1.43, 1.445, z) * ss(0.12, 0.14, ax), s=1)                             # white shoulder caps
for sd, sx in (('L', -1), ('R', 1)):
    ua, la, hd = node('UprArm' + sd), node('LwrArm' + sd), node('Hand' + sd)
    put(ua, p=1); put(ua * ss(1.43, 1.445, z), s=1)
    put(ua * band(z, 1.24, 1.29, 0.004), s=1); put(ua * band(z, 1.225, 1.235, 0.002), d=1)
    put(la, p=1); put(la * band(z, 0.95, 1.00, 0.004), s=1)
    zl = np.percentile(z[la > 0.5], 3) if (la > 0.5).any() else 0.7
    put(la * (1 - ss(zl + 0.05, zl + 0.06, z)), d=1)                                # cuff
    put(hd, d=1); put(hd * band(z, zl - 0.055, zl - 0.03, 0.003), p=1)                  # gloves: dark, kit-colour wrist strap
# ---- pants
pants = node('Pelvis', 'UprLegL', 'UprLegR', 'LwrLegL', 'LwrLegR') * ss(0.42, 0.44, z)
put(pants, p=1)
put(node('Pelvis') * ss(0.99, 1.00, z), d=1)                                        # waist band
for sd, sx in (('L', -1), ('R', 1)):
    leg = node('UprLeg' + sd, 'LwrLeg' + sd, 'Pelvis') * ss(-0.01, 0.01, sx * x) * pants
    outw = ss(0.45, 0.7, sx * NR[..., 0]); inw = ss(0.45, 0.7, -sx * NR[..., 0])
    put(leg * outw * band(z, 0.60, 0.94, 0.006) * band(y, -0.12, -0.03, 0.004), s=1)  # hip flash down the outside
    put(leg * outw * band(z, 0.60, 0.94, 0.006) * band(y, -0.135, -0.125, 0.002), d=1)
    put(leg * inw * band(z, 0.46, 0.80, 0.01), d=1)                                  # inner grip panel
    put(node('UprLeg' + sd, 'LwrLeg' + sd) * band(z, 0.44, 0.58, 0.006) * ss(0.35, 0.6, NR[..., 1]), s=1)  # knee guard
    put(node('UprLeg' + sd, 'LwrLeg' + sd) * band(z, 0.505, 0.515, 0.002) * ss(0.35, 0.6, NR[..., 1]), d=1)
# ---- boots
boot = node('LwrLegL', 'LwrLegR', 'FootL', 'FootR') * (1 - ss(0.42, 0.44, z))
put(boot, s=1)
put(boot * (1 - ss(0.025, 0.035, z)), d=1)                                          # sole
for zb in (0.14, 0.23, 0.32): put(boot * band(z, zb, zb + 0.025, 0.003), d=1)      # buckle straps
put(boot * band(z, 0.38, 0.42, 0.004), p=1)                                         # top band
# ---- helmet: keep goggles/eye port and skin from the old art; shell takes the helmet colour
hdn = node('Head')
lumall = np.min([old[c].mean(-1) for c in old], 0)
o = old['white']; skin = hdn * (diff < 0.08) * ((o[..., 0] - o[..., 2]) > 0.12) * (o[..., 0] > 0.35)
skin = nd.grey_opening(skin, size=3)
dark = hdn * (1 - skin) * (np.max([old[c].mean(-1) for c in old], 0) < 0.18)
dark = nd.grey_closing(dark, size=3)
shell = hdn * (1 - skin) * (1 - dark)
put(shell, p=1, h=1)
put(shell * band(ax, 0.022, 0.040, 0.002) * ss(1.70, 1.72, z), p=1, h=0)           # twin top stripes (kit colour)
put(shell * band(ax, 0.050, 0.058, 0.002) * ss(1.70, 1.72, z), s=1)
put(shell * band(z, 1.555, 1.575, 0.003), s=1)                                     # lower rim stripe
put(dark, d=1); fixed_skin = skin
# ---- output
tot = np.maximum(P + S + Dk, 1e-4); keep = cov * (1 - fixed_skin)
Pm, Sm = P / tot * keep, S / tot * keep; Hm = H * Pm
shade = (0.45 + 0.55 * ao ** 0.6)[..., None]
dark_c = np.array([0.035, 0.035, 0.04])
base = (Dk / tot * keep)[..., None] * dark_c + fixed_skin[..., None] * old['white']
base = np.clip(base * shade, 0, 1)
def save(a, name, mode='RGB'):
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), mode).save(os.path.join(OUT, name), optimize=True)
save(np.stack([Pm, Sm, Hm, shade[..., 0]], -1), 'fp_rider.mask.png', 'RGBA'); save(base, 'fp_rider.base.png')
PAL = {'black': (0.08, 0.08, 0.09), 'blue': (0.06, 0.25, 0.78), 'green': (0.05, 0.45, 0.15), 'grey': (0.42, 0.43, 0.45),
       'lime': (0.55, 0.80, 0.05), 'navy': (0.05, 0.10, 0.32), 'orange': (0.95, 0.42, 0.02), 'purple': (0.38, 0.10, 0.62),
       'red': (0.80, 0.07, 0.06), 'teal': (0.0, 0.50, 0.52), 'white': (0.92, 0.92, 0.90), 'yellow': (0.98, 0.80, 0.05)}
for n, c in PAL.items():
    tr = (0.06, 0.06, 0.07) if n in ('white', 'yellow', 'lime') else (0.90, 0.90, 0.88)
    col = (Pm[..., None] * np.array(c) + Sm[..., None] * np.array(tr)) * shade + base
    save(col, f'fp_rider_{n}.png')
print('done skin', skin.sum(), 'dark', dark.sum())
