"""Two weathered takes on the chrome logo for the menu:
  tyre  - a muddy knobby-tyre track rolled across it, with roost specks
  worn  - old sticker: faded, chipped edges, scratches and grime
Usage: logo_wear.py logo.png dirt_color.jpg out_prefix"""
import sys, math, numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageChops
rng = np.random.default_rng(11)

def noise(shape, blur, seed):
    r = np.random.default_rng(seed)
    n = Image.fromarray((r.random(shape) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(blur))
    a = np.asarray(n, np.float32); return (a - a.min()) / (np.ptp(a) + 1e-6)

def fade(logo, k=0.45):
    a = np.asarray(logo, np.float32) / 255; rgb = a[..., :3]; lum = rgb @ [0.3, 0.59, 0.11]
    rgb = rgb * (1 - k) + lum[..., None] * k; rgb = rgb * [0.97, 0.95, 0.86] + 0.03
    return Image.fromarray((np.concatenate([np.clip(rgb, 0, 1), a[..., 3:]], -1) * 255).astype(np.uint8))

def tyre(logo, dirt):
    logo = fade(logo, 0.4); W, H = logo.size
    # tread band: chevron lugs along a slightly curved path, drawn on a long strip then warped onto the logo
    band_w = int(H * 0.42); L = int(math.hypot(W, H) * 1.2)
    strip = Image.new("L", (L, band_w), 0); d = ImageDraw.Draw(strip)
    step = band_w * 0.42
    for i in range(int(L / step) + 2):
        x = i * step + rng.uniform(-3, 3)
        for side in (-1, 1):
            y0 = band_w / 2 + side * band_w * 0.06
            y1 = band_w / 2 + side * band_w * 0.46
            dx = band_w * 0.18
            d.polygon([(x, y0), (x + band_w * 0.16, y0), (x + band_w * 0.16 + dx, y1), (x + dx, y1)], fill=255)
    # smear between lugs (tyre carcass contact) and uneven pressure
    smear = Image.new("L", strip.size, 0); ImageDraw.Draw(smear).rectangle([0, band_w * 0.12, L, band_w * 0.88], fill=110)
    m = np.maximum(np.asarray(strip, np.float32), np.asarray(smear, np.float32))
    m = np.where(m > 200, 255, m * 0.75); m *= 0.75 + 0.25 * noise(m.shape, 4, 3)
    m *= np.clip(noise(m.shape, 25, 4) * 2.2 - 0.35, 0, 1)
    m = np.where(noise(m.shape, 1.5, 9) > 0.7, m * 0.3, m)  # broken, crumbly edges  # skips where tyre lifted
    strip = Image.fromarray(np.clip(m, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.2))
    ang = -14
    rot = strip.rotate(ang, Image.BICUBIC, expand=True)
    track = Image.new("L", (W, H), 0); track.paste(rot, ((W - rot.size[0]) // 2 + int(W * 0.05), (H - rot.size[1]) // 2 + int(H * 0.04)))
    tm = np.asarray(track, np.float32) / 255
    # roost: specks flicked off the tread
    sp = Image.new("L", (W, H), 0); sd = ImageDraw.Draw(sp)
    for _ in range(1600):
        x, y = rng.uniform(0, W), rng.uniform(0, H); r = rng.pareto(3) * 1.4 + 0.6
        if rng.random() < 0.7 and tm[int(min(y, H - 1)), int(min(x, W - 1))] < 0.05 and rng.random() < 0.8: continue
        sd.ellipse([x - r, y - r * rng.uniform(0.6, 1), x + r, y + r], fill=int(rng.uniform(150, 255)))
    sm = np.asarray(sp.filter(ImageFilter.GaussianBlur(0.6)), np.float32) / 255
    cover = np.clip(np.maximum(tm * 1.0, sm), 0, 1)
    dirt = np.asarray(dirt.resize((W, H)).convert("RGB"), np.float32) / 255
    dirt = dirt * [0.95, 0.85, 0.72] * 0.75
    # wet mud sheen and depth: darker centre of the lugs
    dirt = dirt * (0.8 + 0.35 * noise((H, W), 2, 12))[..., None]
    a = np.asarray(logo, np.float32) / 255; rgb, al = a[..., :3], a[..., 3]
    rgb = rgb * (1 - cover[..., None]) + dirt * cover[..., None]
    al2 = np.maximum(al, sm * 0.9 * (al < 0.1))  # specks can land beyond the plate
    rgb = np.where((al < 0.1)[..., None], dirt, rgb)
    return Image.fromarray((np.concatenate([rgb, al2[..., None]], -1) * 255).astype(np.uint8))

def worn(logo, dirt):
    W, H = logo.size
    a = np.asarray(logo, np.float32) / 255; rgb, al = a[..., :3], a[..., 3].copy()
    lum = rgb @ [0.3, 0.59, 0.11]
    rgb = rgb * 0.4 + lum[..., None] * 0.6                   # sun fade
    rgb = rgb * [1.0, 0.97, 0.88] + [0.05, 0.04, 0.02]        # yellowed
    # grime collecting toward the bottom and in noise blotches
    g = noise((H, W), 18, 5) * 0.6 + np.linspace(0, 0.5, H)[:, None]
    grime = np.clip(g - 0.38, 0, 1) * 0.9
    d = np.asarray(dirt.resize((W, H)).convert("RGB"), np.float32) / 255 * 0.8
    rgb = rgb * (1 - grime[..., None]) + d * grime[..., None]
    # chipped edges: eat into alpha near the outline
    edge = np.asarray(Image.fromarray((al * 255).astype(np.uint8)).filter(ImageFilter.MinFilter(15)), np.float32) / 255
    rim = (al > 0.5) & (edge < 0.5)
    chip = (noise((H, W), 2.5, 6) > 0.55) | ((noise((H, W), 12, 16) > 0.7) & (noise((H, W), 2, 17) > 0.4))
    al = np.where(rim & chip, 0, al)
    # paint flaked off inside: show a pale primer
    flake = (noise((H, W), 3, 7) > 0.68) & (noise((H, W), 30, 8) > 0.5)
    rgb = np.where(flake[..., None], np.array([0.78, 0.76, 0.7]), rgb)
    # scratches
    sc = Image.new("L", (W, H), 0); sd = ImageDraw.Draw(sc)
    for _ in range(140):
        x, y = rng.uniform(0, W), rng.uniform(0, H); ang = rng.normal(-0.3, 0.5); Ls = rng.pareto(2) * 40 + 20
        pts = [(x + math.cos(ang) * t + rng.normal(0, 1.2), y + math.sin(ang) * t + rng.normal(0, 1.2)) for t in np.linspace(0, Ls, 8)]
        sd.line(pts, fill=int(rng.uniform(90, 230)), width=1)
    s = np.asarray(sc, np.float32)[..., None] / 255
    rgb = rgb * (1 - s * 0.7) + np.array([0.86, 0.84, 0.8]) * s * 0.7
    # creases: a couple of soft light/dark folds
    yy, xx = np.mgrid[0:H, 0:W]
    for _ in range(2):
        a0 = rng.uniform(-0.6, 0.6); c = rng.uniform(0.2, 0.8) * W
        dist = (xx - c) * math.cos(a0) + (yy - H / 2) * math.sin(a0)
        rgb = rgb * (1 - 0.12 * np.exp(-(dist / 3) ** 2))[..., None] + 0.08 * np.exp(-((dist - 3) / 2) ** 2)[..., None]
    out = np.concatenate([np.clip(rgb, 0, 1), al[..., None]], -1)
    return Image.fromarray((out * 255).astype(np.uint8))

if __name__ == "__main__":
    logo = Image.open(sys.argv[1]).convert("RGBA"); dirt = Image.open(sys.argv[2])
    tyre(logo, dirt).save(sys.argv[3] + "_tyre.png"); worn(logo, dirt).save(sys.argv[3] + "_worn.png")
