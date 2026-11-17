"""Course Builder link for the main menu chain: our own glyph (a tabletop
jump with a shovel stuck in it), drawn in the menu icons' engraved style.
Writes RGBA overlays (up, lit, grey) the game lays over its own roller."""
import sys
from PIL import Image, ImageDraw, ImageFilter, ImageChops
import numpy as np

W, H = 71, 47
SS = 8  # supersampling

def P(pts, dx=0, dy=0):
    return [((x + dx) * SS, (y + dy) * SS) for x, y in pts]

def shear(img):
    # The chain's icons lean a little, as if engraved on the turning roller.
    return img.transform(img.size, Image.AFFINE, (1, 0.18, -0.18 * img.size[1] / 2, 0, 1, 0), resample=Image.BICUBIC)

def parts():
    """The glyph's faces, each a mask, with how bright its fill is."""
    def poly(pts):
        m = Image.new("L", (W * SS, H * SS), 0)
        ImageDraw.Draw(m).polygon(P(pts), fill=255)
        return m
    front = poly([(10, 39), (21, 27), (43, 27), (58, 39)])
    top = poly([(21, 27), (26, 22), (46, 22), (43, 27)])
    side = poly([(43, 27), (46, 22), (60, 35), (58, 39)])
    shovel = Image.new("L", front.size, 0)
    d = ImageDraw.Draw(shovel)
    d.line(P([(40, 19), (49, 5)]), fill=255, width=int(3.4 * SS))
    d.line(P([(46, 4), (53, 7.5)]), fill=255, width=int(3.0 * SS))
    d.polygon(P([(33.5, 15.5), (43.5, 19.5), (41.5, 25.5), (36, 28.5), (31.5, 23)]), fill=255)
    return [(front, 0.80), (side, 0.55), (top, 1.15), (shovel, 1.0)]

def mask():
    m = Image.new("L", (W * SS, H * SS), 0)
    for p, _ in parts():
        m = ImageChops.lighter(m, p)
    return shear(m)

def down(img):
    return img.resize((W, H), Image.LANCZOS)

def build(state):
    m = mask()
    edge_out = m.filter(ImageFilter.MaxFilter(2 * SS + 1))
    inner = m.filter(ImageFilter.MinFilter(int(1.2 * SS) | 1))
    rim = ImageChops.subtract(edge_out, inner)
    # Fill colours, light from the top left, as the engraved icons.
    yy = np.linspace(0, 1, H * SS)[:, None] * np.ones((1, W * SS))
    xx = np.ones((H * SS, 1)) * np.linspace(0, 1, W * SS)[None, :]
    shade = 1.0 - 0.45 * (0.7 * yy + 0.3 * xx)
    if state == "up":
        top, rimc, glow = np.array([70, 70, 102]), np.array([214, 218, 228]), None
    elif state == "lit":
        top, rimc, glow = np.array([40, 130, 245]), np.array([176, 224, 255]), (90, 170, 255)
    else:
        top, rimc, glow = np.array([140, 140, 140]), np.array([222, 222, 222]), None
    bright = np.zeros((H * SS, W * SS))
    seams = np.zeros((H * SS, W * SS))
    for p, b in parts():
        p = shear(p)
        a = np.asarray(p, float) / 255
        bright = np.where(a > 0.5, b, bright)
        e = np.asarray(ImageChops.subtract(p, p.filter(ImageFilter.MinFilter(int(0.8 * SS) | 1))), float) / 255
        seams = np.maximum(seams, e)
    fill = (top[None, None, :] * (shade * bright)[..., None]).clip(0, 255)
    fill = fill * (1 - seams[..., None]) + rimc[None, None, :] * 0.85 * seams[..., None]
    rimshade = (rimc[None, None, :] * (1.05 - 0.35 * yy[..., None])).clip(0, 255)
    a_fill = np.asarray(inner, float) / 255
    a_rim = np.asarray(rim, float) / 255
    rgb = fill * a_fill[..., None] + rimshade * a_rim[..., None] * (1 - a_fill[..., None])
    alpha = np.maximum(a_fill, a_rim)
    out = np.dstack([rgb, alpha[..., None] * 255]).astype(np.uint8)
    icon = Image.fromarray(out, "RGBA")
    # Dark engraving shadow below right.
    sh = edge_out.filter(ImageFilter.GaussianBlur(1.2 * SS))
    shadow = Image.new("RGBA", icon.size, (10, 10, 25, 0))
    shadow.putalpha(sh.point(lambda v: int(v * 0.65)))
    shadow = ImageChops.offset(shadow, int(1.2 * SS), int(1.4 * SS))
    base = Image.new("RGBA", icon.size, (0, 0, 0, 0))
    if glow:
        g = edge_out.filter(ImageFilter.GaussianBlur(2.2 * SS))
        gl = Image.new("RGBA", icon.size, glow + (0,))
        gl.putalpha(g.point(lambda v: int(v * 0.55)))
        base = Image.alpha_composite(base, gl)
    base = Image.alpha_composite(base, shadow)
    base = Image.alpha_composite(base, icon)
    return down(base)

if __name__ == "__main__":
    out = sys.argv[1]
    for s in ["up", "lit", "grey"]:
        build(s).save(f"{out}/builder_{s}.png", optimize=True)
