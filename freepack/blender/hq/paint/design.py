"""Generic kit design painted in 3D (rest pose, metres) onto the TRELLIS rider's UVs.
Writes kit_mask.png (R primary, G secondary, B dark), kit_mat.png (R roughness, G metallic, B skin keep), kit_ao.png, + sample colourways."""
import numpy as np
from PIL import Image
from scipy import ndimage as nd
X = np.load('restp.npy'); NI = np.rint(np.load('nodei.npy')[..., 0]).astype(int)
names = list(np.load('vattr.npz')['names'])
pr = np.load('probs.npy'); w = pr ** 6; w /= w.sum(2, keepdims=True) + 1e-6
ao = np.load('ao.npy')
x, y, z = X[..., 0], X[..., 1], X[..., 2]; ax = np.abs(x)
def ss(a, b, t):
    t = np.clip((t - a) / (b - a), 0, 1); return t * t * (3 - 2 * t)
def band(t, lo, hi, e=0.004): return ss(lo - e, lo + e, t) * (1 - ss(hi - e, hi + e, t))
GW = np.load('grpw.npy')
ARM = ss(0.35, 0.65, GW[..., 0]); LEG = ss(0.35, 0.65, GW[..., 1]); HEAD = ss(0.35, 0.65, GW[..., 2])
BODY = (1 - ARM) * (1 - LEG) * (1 - HEAD)
def part(n):
    s = n[-1]; side = ss(-0.003, 0.003, x) if s == 'R' else 1 - ss(-0.003, 0.003, x)
    if n in ('LwrTorso', 'UprTorso'): return BODY * ss(0.985, 0.995, z)
    if n == 'Pelvis': return BODY * (1 - ss(0.985, 0.995, z))
    if n == 'Head': return HEAD
    if n.startswith('UprArm'): return ARM * side * ss(1.10, 1.12, z)
    if n.startswith('LwrArm'): return ARM * side * (1 - ss(1.10, 1.12, z)) * ss(0.86, 0.875, z)
    if n.startswith('Hand'): return ARM * side * (1 - ss(0.86, 0.875, z))
    if n.startswith('UprLeg'): return LEG * side * ss(0.50, 0.52, z)
    if n.startswith('LwrLeg'): return LEG * side * (1 - ss(0.50, 0.52, z)) * ss(0.10, 0.12, z)
    if n.startswith('Foot'): return LEG * side * (1 - ss(0.10, 0.12, z))
def node(*ns): return np.clip(sum(part(n) for n in ns), 0, 1)
E_ = 0.004
Pm = np.zeros(x.shape, np.float32); Sm = np.zeros_like(Pm); Dm = np.zeros_like(Pm)
rough = np.full(x.shape, 0.78, np.float32); metal = np.zeros_like(Pm)
def put(region, P=0, S=0, D=0, r=None):
    global Pm, Sm, Dm, rough
    region = region.astype(np.float32)
    Pm = Pm * (1 - region) + P * region; Sm = Sm * (1 - region) + S * region; Dm = Dm * (1 - region) + D * region
    if r is not None: rough[:] = rough * (1 - region) + r * region
# ---- jersey (torso + arms): primary body, dark sides + collar, white sleeves, primary elbow band, dark cuffs
tor = node('LwrTorso', 'UprTorso')
put(tor, P=1)
put(tor * ss(0.155, 0.17, ax) * band(y, -0.12, 0.04, 0.01), D=1)           # side panels
put(tor * ss(1.475, 1.485, z) * (1 - ss(0.09, 0.11, ax)), D=1)                  # collar
for s in 'LR':
    arm = node('UprArm' + s, 'LwrArm' + s)
    put(arm, S=1)
    put(arm * band(z, 1.06, 1.15, 0.006), P=1)                                # elbow band
    put(arm * band(z, 1.20, 1.23, 0.004), D=1)                                # pin stripe
    put(node('LwrArm' + s) * (1 - ss(0.89, 0.90, z)), D=1)                   # cuff
    put(node('UprArm' + s) * band(z, 1.27, 1.30, 0.004), P=1)   # upper sleeve stripe
# ---- chest protector (plastic): white front plate w/ primary centre + dark vents; dark back plate w/ primary spine
yc = -0.02
front = tor * ss(yc + 0.03, yc + 0.06, y)
plate = front * band(z, 1.07, 1.42, 0.006) * (1 - ss(0.150, 0.158, ax + 0.25 * np.clip(1.20 - z, 0, 1)))
put(plate, S=1, r=0.42)
put(plate * band(z, 1.15, 1.37, 0.004) * (1 - ss(0.052, 0.058, ax)), P=1, r=0.42)
for k in range(5):
    cx = -0.10 + 0.05 * k
    put(plate * band(z, 1.255, 1.275, 0.003) * (1 - ss(0.014, 0.018, np.abs(x - cx))) * (np.abs(cx) > 0.01), D=1, r=0.6)
back = tor * (1 - ss(yc - 0.07, yc - 0.04, y))
bplate = back * band(z, 1.05, 1.45, 0.006) * (1 - ss(0.13, 0.138, ax))
put(bplate, D=1, r=0.45)
put(bplate * (1 - ss(0.022, 0.028, ax)) * band(z, 1.08, 1.42, 0.004), P=1, r=0.45)
for s in 'LR':                                                                  # shoulder cups
    put(node('UprArm' + s) * ss(1.385, 1.40, z), S=1, r=0.42)
    put(node('UprArm' + s) * band(z, 1.375, 1.395, 0.003), D=1, r=0.5)
# ---- pants: primary, dark seat/inner leg, white hip flash, plastic knee guards; boots below
pants = node('Pelvis', 'UprLegL', 'UprLegR', 'LwrLegL', 'LwrLegR') * ss(0.40, 0.42, z)
put(pants, P=1)
put(node('Pelvis') * ss(0.985, 0.995, z), D=1)                               # waist band
put(node('Pelvis') * pants * (1 - ss(-0.11, -0.09, y)) * (1 - ss(0.11, 0.13, ax)) * ss(0.80, 0.82, z), D=1)              # seat
for s, sx in (('L', -1), ('R', 1)):
    leg = node('UprLeg' + s, 'LwrLeg' + s, 'Pelvis') * ss(-0.003, 0.003, sx * x)
    inner = leg * (1 - ss(0.055, 0.075, sx * x)) * band(z, 0.45, 0.80, 0.004)
    put(inner * pants, D=1)                                                   # inner leg grip panel
    flash = leg * ss(0.140, 0.146, sx * x) * band(z, 0.62, 0.92, 0.004)
    put(flash * pants, S=1)
    put(flash * pants * band(z, 0.70, 0.73, 0.004), D=1)
    knee = node('UprLeg' + s, 'LwrLeg' + s) * band(z, 0.42, 0.60, 0.006) * ss(-0.06, -0.03, y)
    put(knee, S=1, r=0.45)
    put(knee * band(z, 0.495, 0.515, 0.003), D=1, r=0.5)
# ---- boots: white plastic, dark sole + buckle straps, primary top band
boot = node('LwrLegL', 'LwrLegR', 'FootL', 'FootR') * (1 - ss(0.405, 0.42, z))
put(boot, S=1, r=0.5)
put(boot * (1 - ss(0.03, 0.04, z)), D=1, r=0.9)
for zb in (0.13, 0.22, 0.31):
    put(boot * band(z, zb, zb + 0.022, 0.003), D=1, r=0.6)
put(boot * band(z, 0.37, 0.405, 0.004), P=1, r=0.5)
# ---- gloves: dark palm/fingers, primary back of hand
for s, sx in (('L', -1), ('R', 1)):
    h = node('Hand' + s)
    put(h, D=1, r=0.7)
    put(h * ss(0.30, 0.31, sx * x) * ss(0.76, 0.78, z), P=1, r=0.7)
    put(h * band(z, 0.835, 0.85, 0.003), S=1, r=0.7)
# ---- helmet: keep TRELLIS dark bits (goggles, eye port, strap) + skin; white shell, primary stripes + chin
hd = node('Head')
dark = ss(0.55, 0.7, w[..., 2]) * hd
hx, hy, hz = x, y + 0.11, z - 1.62
shell = hd * (1 - dark)
put(shell, S=1, r=0.25)
put(shell * band(ax, 0.028, 0.050, 0.003) * ss(-0.02, 0.0, hz), P=1, r=0.25)           # twin top stripes
put(shell * band(ax, 0.060, 0.068, 0.002) * ss(0.0, 0.02, hz), D=1, r=0.25)
put(shell * ss(0.10, 0.12, hy) * (1 - ss(-0.05, -0.04, hz)), P=1, r=0.25)               # chin bar
put(shell * band(hz, -0.085, -0.07, 0.003) * (1 - ss(0.08, 0.10, hy)), P=1, r=0.25)     # lower rim stripe
put(dark, D=1, r=0.15)
skin = ss(0.5, 0.7, w[..., 3]) * hd
# ---- output
M = np.stack([Pm, Sm, Dm], -1); M /= np.maximum(M.sum(-1, keepdims=True), 1e-4)
Image.fromarray((M * 255).astype(np.uint8)).save('kit_mask.png')
aoc = np.clip(ao, 0, 1) ** 0.5
Image.fromarray((np.stack([rough, metal, skin], -1) * 255).astype(np.uint8)).save('kit_mat.png')
Image.fromarray((aoc * 255).astype(np.uint8)).save('kit_ao.png')
base = np.asarray(Image.open('/home/user/work/trellis/tex_Image_0.png').convert('RGB').resize((2048, 2048), Image.LANCZOS)).astype(np.float32) / 255
def paint(c0, c1, c2, out):
    col = M[..., 0, None] * np.array(c0) + M[..., 1, None] * np.array(c1) + M[..., 2, None] * np.array(c2)
    col *= (0.35 + 0.65 * aoc)[..., None]
    col = col * (1 - skin[..., None]) + base * skin[..., None]
    Image.fromarray((np.clip(col, 0, 1) * 255).astype(np.uint8)).save(out)
paint((0.06, 0.25, 0.78), (0.90, 0.90, 0.88), (0.03, 0.03, 0.035), 'kit_blue.png')
paint((0.80, 0.07, 0.06), (0.90, 0.90, 0.88), (0.03, 0.03, 0.035), 'kit_red.png')
paint((0.95, 0.45, 0.02), (0.06, 0.06, 0.07), (0.85, 0.85, 0.85), 'kit_orange.png')
print('done')
