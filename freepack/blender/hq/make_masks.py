"""Paint masks for runtime colours: <prefix>.mask.png (R main, G trim, A shade) and <prefix>.base.png (everything else),
so paint = (R*main + G*trim) * A + base. Writes beside the pack's HD liveries (PC and web)."""
import numpy as np, os
from PIL import Image
exec(open('/home/user/work/hq/make_liveries.py').read().split('import sys')[0])   # sets, DARK, load
PK = '/home/user/pack'
def out(a, path, size, mode):
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), mode).resize((size, size), Image.LANCZOS).save(path, optimize=True)
for tag, rid, bik in (('pc', 'riders/hq', None), ('mid', None, 'bikes/hq'), ('web', 'riders/hq/web', 'bikes/hq/web')):
    s = sets[tag]
    if rid:
        km, skin = s['km'], s['kmat'][..., 2:3]; ao = (0.35 + 0.65 * s['kao'])[..., None]
        hel = load({'pc': P + 'kit_helmet.png', 'web': L + 'rider_low_helmet.png'}[tag])[..., 0:1]
        m = np.concatenate([km[..., 0:1] * (1 - skin), km[..., 1:2] * (1 - skin), hel * (1 - skin), ao], -1)
        base = km[..., 2:3] * np.array(DARK) * ao * (1 - skin) + s['kbase'] * skin
        out(m, f'{PK}/{rid}/fp_rider_hq.mask.png', s['size'], 'RGBA'); out(base, f'{PK}/{rid}/fp_rider_hq.base.png', s['size'], 'RGB')
    if bik:
        bm, bfx = s['bm'], s['bfx']
        bao = (0.4 + 0.6 * bfx[..., 3:4]) if bfx.shape[-1] == 4 else np.ones_like(bm[..., :1])
        m = np.concatenate([bm[..., 0:1], bm[..., 1:2], np.zeros_like(bao), bao], -1)
        out(m, f'{PK}/{bik}/fp_livery_hq.mask.png', s['size'], 'RGBA'); out(bfx[..., :3], f'{PK}/{bik}/fp_livery_hq.base.png', s['size'], 'RGB')
    print('masks', tag)
