"""12 colourways for the HQ bike + rider, for the PC (TRELLIS UV, 2048) and web (own UV, 1024) models."""
import numpy as np, os
from PIL import Image
OUT = '/home/user/work/hq/liv'
PAL = {'black': (0.08, 0.08, 0.09), 'blue': (0.06, 0.25, 0.78), 'green': (0.05, 0.45, 0.15), 'grey': (0.42, 0.43, 0.45),
       'lime': (0.55, 0.80, 0.05), 'navy': (0.05, 0.10, 0.32), 'orange': (0.95, 0.42, 0.02), 'purple': (0.38, 0.10, 0.62),
       'red': (0.80, 0.07, 0.06), 'teal': (0.0, 0.50, 0.52), 'white': (0.92, 0.92, 0.90), 'yellow': (0.98, 0.80, 0.05)}
WHITE, BLACK, DARK = (0.90, 0.90, 0.88), (0.06, 0.06, 0.07), (0.03, 0.03, 0.035)
def second(c): return BLACK if c in ('white', 'yellow', 'lime') else WHITE
def load(p, size=None):
    im = Image.open(p).convert('RGB')
    if size: im = im.resize((size, size), Image.LANCZOS)
    return np.asarray(im).astype(np.float32) / 255
def save(a, name, size):
    Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).resize((size, size), Image.LANCZOS).save(os.path.join(OUT, name), optimize=True)
P = '/home/user/work/trellis/paint/'; B = '/home/user/work/trellis/bpaint/'; L = '/home/user/work/hq/out/lod/'
sets = {
  'pc': dict(km=load(P + 'kit_mask.png'), kmat=load(P + 'kit_mat.png'), kao=load(P + 'kit_ao.png')[..., 0], kbase=load('/home/user/work/trellis/tex_Image_0.png', 2048),
             bm=load(B + 'bike_mask.png'), bfx=np.asarray(Image.open(B + 'bike_fixed.png').convert('RGBA')).astype(np.float32) / 255, size=2048),
  'mid': dict(km=load(L + 'rider_mid_mask.png'), kmat=load(L + 'rider_mid_mat.png'), kao=load(L + 'rider_mid_ao.png')[..., 0], kbase=load(L + 'rider_mid_blue.png'),
              bm=load(L + 'bike_mid_mask.png'), bfx=load(L + 'bike_mid_fixed.png'), size=2048),
  'web': dict(km=load(L + 'rider_low_mask.png'), kmat=load(L + 'rider_low_mat.png'), kao=load(L + 'rider_low_ao.png')[..., 0], kbase=load(L + 'rider_low_blue.png'),
              bm=load(L + 'bike_low_mask.png'), bfx=load(L + 'bike_low_fixed.png'), size=1024)}
import sys
for tag, s in sets.items():
    if len(sys.argv) > 1 and tag not in sys.argv[1:]: continue
    km, skin = s['km'], s['kmat'][..., 2:3]
    ao = (0.35 + 0.65 * s['kao'])[..., None]
    bm, bfx = s['bm'], s['bfx']
    for name, c in PAL.items():
        kit = (km[..., 0:1] * np.array(c) + km[..., 1:2] * np.array(second(name) if name != 'black' else WHITE) + km[..., 2:3] * np.array(DARK)) * ao
        kit = kit * (1 - skin) + s['kbase'] * skin
        save(kit, f'{tag}_rider_{name}.png', s['size'])
        bao = (0.4 + 0.6 * bfx[..., 3:4]) if bfx.shape[-1] == 4 else 1.0
        bike = bm[..., 0:1] * np.array(c) * bao + bm[..., 1:2] * np.array(second(name)) * bao + bfx[..., :3] * (1 - bm[..., 0:1] - bm[..., 1:2]).clip(0, 1) / np.maximum(1 - bm[..., 0:1] - bm[..., 1:2], 1e-3).clip(1e-3, 1) * (1 - bm[..., 0:1] - bm[..., 1:2]).clip(0, 1) ** 0
        save(bike, f'{tag}_bike_{name}.png', s['size'])
    print('done', tag)
