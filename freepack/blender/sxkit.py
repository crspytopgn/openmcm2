"""Free Supercross kit: wall pads, the 40-gate start, Tuff Blocks and the
track-side props, with wear, dirt and roost baked into their pictures
(the game draws colour only, so all detail lives in the textures).

    python blender/sxkit.py [name ...]     -> build/objects/<name>.glb

Model space as common.py (X = game x, Y = game z... exported -z, Z = up),
feet, origin at the centre of the footprint on the ground. Long pieces run
along x; "front" (the track side, or where riders face) is Blender -Y,
which is game +z.
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from mathutils import Vector
from common import reset, BUILD, CACHE, image_mat, derived_color, cyl, join_all, export

TEX = os.path.join(BUILD, "tex"); os.makedirs(TEX, exist_ok=True)
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
BRANDS = os.path.join(BUILD, "brands")
DIRT = np.array([0.36, 0.26, 0.17])

# ---------------- picture helpers ----------------
def noise(h, w, cell, seed):
    r = np.random.RandomState(seed).rand(h // cell + 2, w // cell + 2)
    return np.asarray(Image.fromarray((r * 255).astype(np.uint8)).resize((w + 2 * cell, h + 2 * cell), Image.BICUBIC))[cell:cell + h, cell:cell + w] / 255.0

def fbm(h, w, seed, cells=(64, 24, 8, 3)):
    a = sum(noise(h, w, c, seed + i) * (0.5 ** i) for i, c in enumerate(cells))
    return (a - a.min()) / (a.max() - a.min() + 1e-6)

def roost(a, seed, height=0.45, amount=0.75):
    """Dirt thrown up from the track: dense at the bottom edge, specks higher up.
    a is (H, W, 3) linear, v=0 at the top row."""
    h, w = a.shape[:2]; r = np.random.RandomState(seed)
    y = np.linspace(0, 1, h)[:, None]                       # 0 top, 1 bottom
    base = np.clip((y - (1 - height)) / height, 0, 1) ** 1.6
    n = fbm(h, w, seed); film = np.clip(base * (0.6 + 0.8 * n) - 0.1, 0, 1) * amount
    specks = (r.rand(h, w) < 0.04 * base) * r.rand(h, w)
    specks = np.asarray(Image.fromarray((specks * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(3))) / 255 * 0.8
    m = np.clip(film + specks * (base > 0.05), 0, 0.95)[..., None]
    tone = DIRT * (0.75 + 0.5 * n[..., None])
    return a * (1 - m) + tone * m

def scuffs(a, seed, n=60, dark=0.25, light=0.0):
    h, w = a.shape[:2]; im = Image.new("L", (w, h)); d = ImageDraw.Draw(im); r = random.Random(seed)
    for _ in range(n):
        x, y = r.uniform(0, w), r.uniform(0, h); L = r.uniform(0.02, 0.15) * w; ang = r.uniform(-0.4, 0.4)
        d.line([(x, y), (x + L * math.cos(ang), y + L * math.sin(ang))], fill=r.randint(60, 200), width=r.randint(1, 4))
    m = np.asarray(im.filter(ImageFilter.GaussianBlur(1))) / 255.0
    return a * (1 - dark * m[..., None]) + light * m[..., None]

def lin(img): return (np.asarray(img.convert("RGB")).astype(np.float32) / 255) ** 2.2
def save(a, name):
    p = os.path.join(TEX, name + ".png")
    Image.fromarray((np.clip(a, 0, 1) ** (1 / 2.2) * 255 + 0.5).astype(np.uint8)).save(p); return p

def brand_list(): return sorted(f[:-4] for f in os.listdir(BRANDS) if f.endswith(".png") and "_" not in f)
def brand_img(b): return Image.open(os.path.join(BRANDS, b + ".png")).convert("RGB")

# ---------------- mesh helpers ----------------
class Mesh:
    """One bmesh with several materials; quads get explicit UVs."""
    def __init__(self, name): self.bm = bmesh.new(); self.uv = self.bm.loops.layers.uv.new(); self.mats = []; self.name = name
    def mi(self, m):
        if m not in self.mats: self.mats.append(m)
        return self.mats.index(m)
    def quad(self, pts, m, uvs=((0, 0), (1, 0), (1, 1), (0, 1))):
        f = self.bm.faces.new([self.bm.verts.new(Vector(p)) for p in pts]); f.material_index = self.mi(m)
        for l, t in zip(f.loops, uvs): l[self.uv].uv = t
        return f
    def box(self, lo, hi, m, s=1.0, side=None, top=None, H=None):
        """Box with world-size UVs (s ft per repeat). With side/top pictures,
        side faces map v = height / H (dirt at the bottom) and u along the face."""
        (x0, y0, z0), (x1, y1, z1) = lo, hi; H = H or (z1 - z0)
        sm = side or m; tm = top or m
        def vz(z): return (z - z0) / H if side else z / s
        P = lambda x, y, z: (x, y, z)
        self.quad([P(x0, y0, z0), P(x1, y0, z0), P(x1, y0, z1), P(x0, y0, z1)], sm, [(x0 / s, vz(z0)), (x1 / s, vz(z0)), (x1 / s, vz(z1)), (x0 / s, vz(z1))])   # front (-Y)
        self.quad([P(x1, y1, z0), P(x0, y1, z0), P(x0, y1, z1), P(x1, y1, z1)], sm, [(-x1 / s, vz(z0)), (-x0 / s, vz(z0)), (-x0 / s, vz(z1)), (-x1 / s, vz(z1))])  # back
        self.quad([P(x1, y0, z0), P(x1, y1, z0), P(x1, y1, z1), P(x1, y0, z1)], sm, [(y0 / s, vz(z0)), (y1 / s, vz(z0)), (y1 / s, vz(z1)), (y0 / s, vz(z1))])
        self.quad([P(x0, y1, z0), P(x0, y0, z0), P(x0, y0, z1), P(x0, y1, z1)], sm, [(-y1 / s, vz(z0)), (-y0 / s, vz(z0)), (-y0 / s, vz(z1)), (-y1 / s, vz(z1))])
        self.quad([P(x0, y0, z1), P(x1, y0, z1), P(x1, y1, z1), P(x0, y1, z1)], tm, [(x0 / s, y0 / s), (x1 / s, y0 / s), (x1 / s, y1 / s), (x0 / s, y1 / s)])
        self.quad([P(x0, y1, z0), P(x1, y1, z0), P(x1, y0, z0), P(x0, y0, z0)], tm, [(x0 / s, y1 / s), (x1 / s, y1 / s), (x1 / s, y0 / s), (x0 / s, y0 / s)])
    def done(self):
        me = bpy.data.meshes.new(self.name); self.bm.to_mesh(me); self.bm.free()
        for m in self.mats: me.materials.append(m)
        o = bpy.data.objects.new(self.name, me); bpy.context.scene.collection.objects.link(o); return o

def tex_mat(name, a): return image_mat(save(a, name), name, 0.8)

def finish(name):
    o = join_all(name); export(os.path.join(BUILD, "objects", name + ".glb"))
    print(name, "tris", sum(len(p.vertices) - 2 for p in o.data.polygons))

# shared materials
def steel_tex(name="sx_steel", lo=(0.17, 0.18, 0.19), hi=(0.36, 0.37, 0.38), rust=0.0):
    a = lin(Image.open(derived_color("PaintedMetal004", ramp=(lo, hi))).resize((1024, 1024)))
    a = scuffs(a, 5, 80, 0.2, 0.05)
    if rust: a = a * (1 - rust * fbm(1024, 1024, 9)[..., None] ** 3) + np.array([0.25, 0.12, 0.05]) * rust * fbm(1024, 1024, 9)[..., None] ** 3
    return tex_mat(name, a)

def concrete_tex(name="sx_concrete", dirt=0.5):
    a = lin(Image.open(derived_color("Concrete034", ramp=((0.2, 0.2, 0.19), (0.5, 0.49, 0.47)))).resize((1024, 1024)))
    n = fbm(1024, 1024, 21)[..., None]; m = np.clip(n * 1.6 - 0.5, 0, 1) * dirt
    return tex_mat(name, a * (1 - m) + DIRT * 0.8 * m)

# ---------------- fp_wall_pad: 40 ft padded wall with sponsor banners ----------------
def wall_pad(variant=0):
    """Foam pads in a vinyl skin, 40 x 4 x 2.2 ft, rounded top, banner on
    the track side, roost on the lower half."""
    name = "fp_wall_pad" + ("" if variant == 0 else "_" + "abcd"[variant])
    reset(); r = random.Random(30 + variant); brands = brand_list()
    W, Hh = 2048, 256
    bgs = [[(16, 16, 18), (235, 235, 230)], [(170, 18, 22), (14, 14, 16)], [(14, 40, 110), (235, 235, 230)], [(26, 26, 28), (240, 170, 20)]][variant]
    im = Image.new("RGB", (W, Hh), bgs[0]); d = ImageDraw.Draw(im)
    seg = W // 4                                            # 10 ft panels, a seam between each
    for k in range(4):
        x0 = k * seg; bg = bgs[k % 2] if variant in (1, 3) else bgs[0]
        d.rectangle([x0, 0, x0 + seg, Hh], fill=bg)
        b = brand_img(r.choice(brands)); b.thumbnail((seg - 70, Hh - 70)); im.paste(b, (x0 + (seg - b.width) // 2, (Hh - b.height) // 2 - 6))
        d.rectangle([x0, 0, x0 + 3, Hh], fill=(8, 8, 8))
    d.rectangle([0, Hh - 22, W, Hh], fill=(10, 10, 11))                           # black kick strip
    a = lin(im)
    yy = np.linspace(0, 1, Hh)[:, None, None]; a = a * (0.86 + 0.14 * np.cos((yy - 0.45) * 2.6))  # pillowed foam: darker toward the folds
    a = scuffs(a, 40 + variant, 140, 0.35); a = roost(a, 50 + variant, 0.55, 0.85)
    banner = tex_mat(f"fp_wallpad_{variant}", a)
    vinyl = np.full((512, 512, 3), 0.018) * (0.8 + 0.4 * fbm(512, 512, 3)[..., None]); vinyl = roost(scuffs(vinyl, 7, 50, 0, 0.03), 8, 0.6, 0.6)
    back = tex_mat("fp_wallpad_back", vinyl)
    top = tex_mat("fp_wallpad_top", roost(np.full((256, 256, 3), 0.02), 9, 1.0, 0.5))
    m = Mesh(name); L, D, H = 40.0, 2.2, 4.0
    # rounded profile across Y (front -D/2 -> back +D/2), extruded along X
    prof = [(-D / 2, 0)] + [(-D / 2 + 0.35 - 0.35 * math.cos(t), H - 0.35 + 0.35 * math.sin(t)) for t in np.linspace(0, math.pi / 2, 4)]
    prof = [(-D / 2, 0), (-D / 2, H - 0.35)] + [(-(D / 2 - 0.35) - 0.35 * math.cos(t), H - 0.35 + 0.35 * math.sin(t)) for t in np.linspace(0, math.pi / 2, 4)[1:]]
    prof += [(-y, z) for (y, z) in reversed(prof)]
    for i in range(len(prof) - 1):
        (y0, z0), (y1, z1) = prof[i], prof[i + 1]
        front = y0 < 0 and y1 < 0 and z1 <= H - 0.3
        mat = banner if (y0 < 0 and z1 <= H - 0.2) else back if (y0 > 0 and y1 > 0) else top
        v0, v1 = (z0 / H, z1 / H) if mat is banner else (z0 / 4, z1 / 4)
        u = (lambda x: (x + L / 2) / L) if mat is banner else (lambda x: x / 4)
        m.quad([(-L / 2, y0, z0), (L / 2, y0, z0), (L / 2, y1, z1), (-L / 2, y1, z1)], mat,
               [(u(-L / 2), v0), (u(L / 2), v0), (u(L / 2), v1), (u(-L / 2), v1)])
    for x in (-L / 2, L / 2):                                  # end caps
        pts = [(x, y, z) for (y, z) in prof]
        f = m.bm.faces.new([m.bm.verts.new(Vector(p)) for p in (pts[::-1] if x > 0 else pts)]); f.material_index = m.mi(back)
        for l in f.loops: l[m.uv].uv = (l.vert.co.y / 4, l.vert.co.z / 4)
    m.done(); finish(name)

def _fix_normals():
    """Make every face point away from its object's centre (closed solids)."""
    for o in bpy.context.scene.objects:
        if o.type != "MESH": continue
        bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.recalc_face_normals(bm, faces=bm.faces); bm.to_mesh(o.data); bm.free()

# ---------------- fp_startgate: 40 gates, drop bars, concrete pad ----------------
def startgate():
    """40 drop gates, 3 ft each (120 ft), the bars standing up; the
    concrete start pad behind them (+Y, riders face -Y toward the track)."""
    reset(); N, P = 40, 3.0; L = N * P
    # numbered gate plates: an 8 x 5 atlas
    W, H = 2048, 1024; cw, ch = W // 8, H // 5; im = Image.new("RGB", (W, H)); d = ImageDraw.Draw(im); f = ImageFont.truetype(FONT, 120)
    brands = brand_list(); r = random.Random(3)
    for k in range(N):
        x0, y0 = (k % 8) * cw, (k // 8) * ch
        d.rectangle([x0, y0, x0 + cw, y0 + ch], fill=(205, 205, 200))
        d.rectangle([x0 + 6, y0 + 6, x0 + cw - 6, y0 + ch * 0.36], fill=(24, 24, 26))
        b = brand_img(brands[k % len(brands)]); b.thumbnail((cw - 30, int(ch * 0.3) - 12)); im.paste(b, (x0 + (cw - b.width) // 2, y0 + 12))
        t = str(k + 1); tw = d.textlength(t, font=f); d.text((x0 + (cw - tw) / 2, y0 + ch * 0.38), t, font=f, fill=(20, 20, 22))
    a = lin(im)
    a = roost(np.concatenate([roost(a[k * ch:(k + 1) * ch], 70 + k, 0.7, 0.8) for k in range(5)], 0), 1, 0.01, 0.0)
    a = scuffs(a, 71, 300, 0.4)
    plates = tex_mat("fp_startgate_plates", a)
    steel = steel_tex("fp_startgate_steel", (0.1, 0.1, 0.11), (0.3, 0.3, 0.31))
    galv = steel_tex("fp_startgate_galv", (0.3, 0.31, 0.32), (0.58, 0.59, 0.6))
    # pad: grooved concrete with dirt tracked over it, darker in each lane
    pa = lin(Image.open(derived_color("Concrete034", ramp=((0.22, 0.22, 0.21), (0.52, 0.51, 0.49)))).resize((2048, 256)))
    lane = (np.cos(np.linspace(0, N * 2 * math.pi, 2048)) * 0.5 + 0.5)[None, :, None]
    m_ = np.clip(0.35 + 0.45 * fbm(256, 2048, 4)[..., None] - 0.3 * lane, 0, 0.9)
    pa = pa * (1 - m_) + DIRT * 0.75 * m_
    pa[:, ::int(2048 / N)] *= 0.5
    pad = tex_mat("fp_startgate_pad", pa)
    m = Mesh("fp_startgate")
    m.quad([(-L / 2 - 4, 0.6, 0.0), (L / 2 + 4, 0.6, 0.0), (L / 2 + 4, 14.0, 0.0), (-L / 2 - 4, 14.0, 0.0)], pad)                               # pad, gate line to 14 ft back
    m.box((-L / 2 - 1, -0.6, 0), (L / 2 + 1, 0.6, 0.45), steel, s=4)           # base beam with the hinge line
    for k in range(N):                                                    # drop bars, slightly uneven
        x0 = -L / 2 + k * P + 0.12; x1 = x0 + P - 0.24; tilt = random.Random(k).uniform(-0.04, 0.04)
        u0, v0 = (k % 8) / 8, 1 - (k // 8 + 1) / 5; u1, v1 = u0 + 1 / 8, v0 + 1 / 5
        y0, z0, z1 = -0.25, 0.45, 2.3; dy = tilt * (z1 - z0)
        m.quad([(x0, y0, z0), (x1, y0, z0), (x1, y0 + dy, z1), (x0, y0 + dy, z1)], plates, [(u0, v0), (u1, v0), (u1, v1), (u0, v1)])
        m.quad([(x1, y0 + 0.1, z0), (x0, y0 + 0.1, z0), (x0, y0 + 0.1 + dy, z1), (x1, y0 + 0.1 + dy, z1)], steel, [(0, 0), (1, 0), (1, 0.5), (0, 0.5)])
        m.box((x0, y0 + dy, z1), (x1, y0 + dy + 0.1, z1 + 0.08), galv, s=3)
    for k in range(0, N + 1, 5):                                          # hydraulic stands
        x = -L / 2 + k * P
        m.box((x - 0.35, 0.6, 0), (x + 0.35, 1.6, 0.9), galv, s=2)
    m.done()
    for k in range(0, N + 1, 5):
        x = -L / 2 + k * P; cyl((x, 1.1, 0.9), (x, -0.1, 0.4), 0.12, steel, 8)
    finish("fp_startgate")

# ---------------- fp_tuffblock: rotomoulded dividers ----------------
def tuffblock(variant=0):
    """5 x 2 x 2.6 ft tapered plastic block; white/blue, all-yellow or red,
    with made-up stickers, scuffs and roost."""
    name = "fp_tuffblock" + ("" if variant == 0 else "_" + "abc"[variant])
    reset(); r = random.Random(80 + variant)
    body = [(0.78, 0.78, 0.75), (0.72, 0.55, 0.05), (0.62, 0.08, 0.06)][variant]
    band = [(0.06, 0.13, 0.4), (0.06, 0.06, 0.07), (0.85, 0.85, 0.82)][variant]
    S = 1024; a = np.ones((S, S, 3)) * np.array(body) ** 1.0
    yy = np.linspace(0, 1, S)[:, None]
    a[(yy[:, 0] > 0.58) & (yy[:, 0] < 0.80)] = band                           # moulded band low on the side
    a *= (0.92 + 0.08 * fbm(S, S, 5)[..., None])                               # rotomoulded blotch
    im = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)); brands = brand_list()
    for k in range(2):                                                        # stickers
        b = brand_img(r.choice(brands)); b.thumbnail((360, 110)); im.paste(b, (90 + k * 470, 200 + r.randint(-20, 30)))
    a = (np.asarray(im).astype(np.float32) / 255)
    a = scuffs(a, 90 + variant, 220, 0.35); a = roost(a, 95 + variant, 0.65, 0.9)
    side = tex_mat(f"fp_tuff_side_{variant}", a ** 1.0)
    top = tex_mat(f"fp_tuff_top_{variant}", roost(np.ones((256, 256, 3)) * np.array(body) * 0.9, 96, 1.0, 0.35))
    m = Mesh(name); L, Wd, H = 5.0, 2.0, 2.6
    # taper: bottom Wd, top 0.72 Wd, chamfered top edges
    prof = [(-Wd / 2, 0), (-Wd * 0.36, H - 0.18), (-Wd * 0.3, H), (Wd * 0.3, H), (Wd * 0.36, H - 0.18), (Wd / 2, 0)]
    for i in range(len(prof) - 1):
        (y0, z0), (y1, z1) = prof[i], prof[i + 1]
        mat = top if (z0 >= H - 0.01 and z1 >= H - 0.01) else side
        uv = [(0.0, z0 / H), (1.0, z0 / H), (1.0, z1 / H), (0.0, z1 / H)] if mat is side else [(0, 0), (1, 0), (1, 1), (0, 1)]
        if y0 > 0 or y1 > 0: uv = [(1 - u, v) for (u, v) in uv]
        m.quad([(-L / 2, y0, z0), (L / 2, y0, z0), (L / 2, y1, z1), (-L / 2, y1, z1)], mat, uv)
    for x in (-L / 2, L / 2):
        pts = [(x, y, z) for (y, z) in prof]
        f = m.bm.faces.new([m.bm.verts.new(Vector(p)) for p in (pts[::-1] if x > 0 else pts)]); f.material_index = m.mi(side)
        for l in f.loops: l[m.uv].uv = (0.5 + l.vert.co.y / Wd * 0.4, l.vert.co.z / H)
    # grip slot on top
    slot = tex_mat("fp_tuff_slot", np.full((8, 8, 3), 0.02))
    m.quad([(-0.6, -0.18, H + 0.01), (0.6, -0.18, H + 0.01), (0.6, 0.18, H + 0.01), (-0.6, 0.18, H + 0.01)], slot)
    m.done(); finish(name)

# ---------------- props ----------------
def finish_arch():
    """Checkered truss arch over the finish line, 40 ft span, 22 ft high."""
    reset(); S = 512; yy, xx = np.mgrid[0:S, 0:S]
    chk = (((xx // 64) + (yy // 64)) % 2).astype(np.float32)[..., None]
    a = np.repeat(chk * 0.82 + (1 - chk) * 0.02, 3, 2); a = scuffs(a, 3, 40, 0.2)
    board = tex_mat("fp_arch_check", a)
    galv = steel_tex("fp_arch_truss", (0.42, 0.43, 0.44), (0.72, 0.72, 0.72))
    m = Mesh("fp_finish_arch"); SPAN, HT = 40.0, 22.0
    for x in (-SPAN / 2, SPAN / 2): m.box((x - 1, -1, 0), (x + 1, 1, HT), board, s=2.0)   # legs wrapped in checks
    m.box((-SPAN / 2 - 1, -1, HT - 4), (SPAN / 2 + 1, 1, HT), board, s=2.0)              # header
    m.box((-SPAN / 2 - 1.5, -1.5, 0), (-SPAN / 2 + 1.5, 1.5, 0.4), galv, s=2); m.box((SPAN / 2 - 1.5, -1.5, 0), (SPAN / 2 + 1.5, 1.5, 0.4), galv, s=2)
    m.done(); finish("fp_finish_arch")

def tv_camera():
    """Broadcast camera on a tripod with a long lens, operator-less."""
    reset(); body = steel_tex("fp_cam_body", (0.03, 0.03, 0.035), (0.12, 0.12, 0.13))
    grey = steel_tex("fp_cam_grey", (0.25, 0.25, 0.26), (0.45, 0.45, 0.46))
    m = Mesh("fp_tv_camera")
    m.box((-0.35, -0.6, 4.4), (0.35, 0.9, 5.2), body, s=1); m.box((-0.25, 0.9, 4.95), (0.25, 1.3, 5.35), grey, s=1)   # body, viewfinder
    m.box((-0.3, -0.4, 5.2), (0.3, 0.2, 5.3), grey, s=1)
    m.done()
    cyl((0, -0.6, 4.8), (0, -2.6, 4.8), 0.28, body, 16); cyl((0, -2.6, 4.8), (0, -2.75, 4.8), 0.33, grey, 16)        # lens
    for a in (0, 2.1, 4.2): cyl((0, 0, 4.4), (1.3 * math.cos(a), 1.3 * math.sin(a), 0), 0.06, grey, 6)
    cyl((0, 0, 4.4), (0, 0, 3.4), 0.1, body, 8)
    finish("fp_tv_camera")

def camera_platform():
    """Scaffold tower, 8 x 8 ft deck at 12 ft, kick boards, ladder, sponsor skirt."""
    reset(); galv = steel_tex("fp_plat_galv", (0.3, 0.31, 0.32), (0.6, 0.61, 0.62), rust=0.15)
    ply = lin(Image.open(os.path.join(CACHE, "ambientcg", "Wood049", "Wood049_2K-JPG_Color.jpg")).resize((512, 512)) if os.path.exists(os.path.join(CACHE, "ambientcg", "Wood049", "Wood049_2K-JPG_Color.jpg")) else Image.new("RGB", (512, 512), (120, 90, 60)))
    deck = tex_mat("fp_plat_deck", roost(ply, 2, 1.0, 0.3))
    brands = brand_list(); im = Image.new("RGB", (1024, 256), (14, 14, 16)); b = brand_img(brands[3]); b.thumbnail((900, 200)); im.paste(b, ((1024 - b.width) // 2, (256 - b.height) // 2))
    skirt = tex_mat("fp_plat_skirt", roost(scuffs(lin(im), 4, 60, 0.3), 5, 0.4, 0.6))
    m = Mesh("fp_camera_platform"); S, Z = 4.0, 12.0
    m.box((-S, -S, Z - 0.3), (S, S, Z), deck, s=4)
    m.quad([(-S, -S - 0.05, Z - 3.3), (S, -S - 0.05, Z - 3.3), (S, -S - 0.05, Z - 0.3), (-S, -S - 0.05, Z - 0.3)], skirt)   # skirt toward the track
    m.done()
    for x in (-S + 0.2, S - 0.2):
        for y in (-S + 0.2, S - 0.2): cyl((x, y, 0), (x, y, Z + 3.4), 0.1, galv, 8)
    for z in (3.0, 6.5, 10.0, Z + 1.7, Z + 3.4):
        for (a, b) in (((-S, -S), (S, -S)), ((S, -S), (S, S)), ((S, S), (-S, S)), ((-S, S), (-S, -S))):
            if z > Z and a == (-S, S): continue
            cyl((a[0], a[1], z), (b[0], b[1], z), 0.07, galv, 6)
    for k in range(int(Z)): cyl((-S + 0.2, S + 0.15, 0.6 + k), (-S + 1.8, S + 0.15, 0.6 + k), 0.05, galv, 6)
    finish("fp_camera_platform")

def mechanics_canopy():
    """10 x 10 ft pop-up canopy with a made-up team print, legs weighted."""
    reset(); brands = brand_list(); r = random.Random(12)
    im = Image.new("RGB", (1024, 1024), (24, 24, 28)); d = ImageDraw.Draw(im)
    b = brand_img(r.choice(brands)); b.thumbnail((700, 260)); im.paste(b, ((1024 - b.width) // 2, 380))
    d.rectangle([0, 0, 1024, 60], fill=(190, 25, 25)); d.rectangle([0, 964, 1024, 1024], fill=(190, 25, 25))
    roof = tex_mat("fp_canopy_roof", scuffs(lin(im) * (0.9 + 0.1 * fbm(1024, 1024, 2)[..., None]), 6, 30, 0.15))
    val = Image.new("RGB", (1024, 128), (190, 25, 25)); b2 = b.copy(); b2.thumbnail((600, 100)); val.paste(b2, ((1024 - b2.width) // 2, 14))
    valance = tex_mat("fp_canopy_valance", lin(val))
    alu = steel_tex("fp_canopy_alu", (0.4, 0.4, 0.41), (0.7, 0.7, 0.7))
    m = Mesh("fp_mechanics_canopy"); S, Z, P = 5.0, 7.0, 9.5
    for (a, b_), uv in ((((-S, -S), (S, -S)), None), (((S, -S), (S, S)), None), (((S, S), (-S, S)), None), (((-S, S), (-S, -S)), None)):
        m.quad([(a[0], a[1], Z), (b_[0], b_[1], Z), (0, 0, P)] + [], roof, [(0.5 + a[0] / S / 2, 0.5 + a[1] / S / 2), (0.5 + b_[0] / S / 2, 0.5 + b_[1] / S / 2), (0.5, 0.5)]) if False else None
        f = m.bm.faces.new([m.bm.verts.new(Vector(p)) for p in ((a[0], a[1], Z), (b_[0], b_[1], Z), (0, 0, P))]); f.material_index = m.mi(roof)
        for l in f.loops: l[m.uv].uv = (0.5 + l.vert.co.x / S / 2, 0.5 - l.vert.co.y / S / 2)
        m.quad([(a[0], a[1], Z - 0.9), (b_[0], b_[1], Z - 0.9), (b_[0], b_[1], Z), (a[0], a[1], Z)], valance, [(0, 0), (1, 0), (1, 1), (0, 1)])
    m.done()
    for x in (-S, S):
        for y in (-S, S): cyl((x, y, 0), (x, y, Z), 0.09, alu, 8); cyl((x, y, 0), (x, y, 0.35), 0.35, steel_tex("fp_canopy_weight", (0.03, 0.03, 0.03), (0.1, 0.1, 0.1)), 12)
    finish("fp_mechanics_canopy")

def toolbox():
    """Roll-away tool chest, red, drawers, scuffed."""
    reset(); S = 512; im = Image.new("RGB", (S, S), (150, 18, 20)); d = ImageDraw.Draw(im)
    for k in range(7):
        y = 20 + k * 68; d.rectangle([14, y, S - 14, y + 60], fill=(165, 22, 24)); d.rectangle([60, y + 8, S - 60, y + 16], fill=(170, 170, 170))
    front = tex_mat("fp_toolbox_front", scuffs(lin(im), 8, 120, 0.4, 0.04))
    side = tex_mat("fp_toolbox_side", scuffs(np.ones((256, 256, 3)) * np.array([0.3, 0.012, 0.012]), 9, 40, 0.3))
    m = Mesh("fp_toolbox"); m.box((-1.4, -0.9, 0.4), (1.4, 0.9, 3.6), side, s=3, side=None)
    m.quad([(-1.4, -0.91, 0.4), (1.4, -0.91, 0.4), (1.4, -0.91, 3.6), (-1.4, -0.91, 3.6)], front)
    m.done()
    blk = steel_tex("fp_toolbox_wheel", (0.02, 0.02, 0.02), (0.08, 0.08, 0.08))
    for x in (-1.2, 1.2):
        for y in (-0.7, 0.7): cyl((x - 0.08, y, 0.2), (x + 0.08, y, 0.2), 0.2, blk, 12)
    finish("fp_toolbox")

def lightrig():
    """Portable light tower: trailer, mast and 4 lamp heads."""
    reset(); y_ = steel_tex("fp_lightrig_body", (0.45, 0.34, 0.02), (0.75, 0.6, 0.08), rust=0.1)
    galv = steel_tex("fp_lightrig_mast", (0.3, 0.31, 0.32), (0.58, 0.59, 0.6))
    lamp = Image.new("RGB", (128, 128), (255, 250, 235)); d = ImageDraw.Draw(lamp); d.rectangle([0, 0, 127, 127], outline=(40, 40, 40), width=10)
    lm = tex_mat("fp_lightrig_lamp", lin(lamp))
    m = Mesh("fp_lightrig"); m.box((-3, -2, 1.2), (3, 2, 4.2), y_, s=3)
    for k, (dx, dz) in enumerate(((-1.6, 0), (1.6, 0), (-1.6, 2.2), (1.6, 2.2))):
        c = Vector((dx, -0.6, 27 + dz))
        m.box((c.x - 0.9, c.y, c.z - 0.9), (c.x + 0.9, c.y + 0.6, c.z + 0.9), galv, s=1)
        m.quad([(c.x - 0.8, c.y - 0.01, c.z - 0.8), (c.x + 0.8, c.y - 0.01, c.z - 0.8), (c.x + 0.8, c.y - 0.01, c.z + 0.8), (c.x - 0.8, c.y - 0.01, c.z + 0.8)], lm)
    m.done()
    cyl((0, 0, 4.2), (0, 0, 28.5), 0.3, galv, 10, r2=0.18); cyl((-3, -0.6, 27.5), (3, -0.6, 27.5), 0.12, galv, 6)
    for x in (-2.4, 2.4): cyl((x - 0.2, 0, 0.9), (x + 0.2, 0, 0.9), 0.9, steel_tex("fp_lightrig_tyre", (0.02, 0.02, 0.02), (0.07, 0.07, 0.07)), 16)
    finish("fp_lightrig")

def number_board():
    """Sign board on a pole held up by the flagger: lap boards 1 to 'white flag'."""
    reset(); im = Image.new("RGB", (512, 512), (235, 235, 230)); d = ImageDraw.Draw(im); f = ImageFont.truetype(FONT, 360)
    d.text((140, 40), "2", font=f, fill=(15, 15, 15)); d.rectangle([0, 0, 511, 511], outline=(20, 20, 20), width=14)
    face = tex_mat("fp_numberboard", scuffs(lin(im), 2, 50, 0.3))
    alu = steel_tex("fp_canopy_alu", (0.4, 0.4, 0.41), (0.7, 0.7, 0.7))
    m = Mesh("fp_number_board"); m.quad([(-1.2, -0.05, 4.8), (1.2, -0.05, 4.8), (1.2, -0.05, 7.2), (-1.2, -0.05, 7.2)], face)
    m.quad([(1.2, 0.05, 4.8), (-1.2, 0.05, 4.8), (-1.2, 0.05, 7.2), (1.2, 0.05, 7.2)], face)
    m.done(); cyl((0, 0.1, 0), (0, 0.1, 7.2), 0.08, alu, 8); finish("fp_number_board")

ALL = {"fp_wall_pad": lambda: [wall_pad(v) for v in range(4)], "fp_startgate": startgate,
       "fp_tuffblock": lambda: [tuffblock(v) for v in range(3)], "fp_finish_arch": finish_arch, "fp_tv_camera": tv_camera,
       "fp_camera_platform": camera_platform, "fp_mechanics_canopy": mechanics_canopy, "fp_toolbox": toolbox,
       "fp_lightrig": lightrig, "fp_number_board": number_board}

if __name__ == "__main__":
    for n in (sys.argv[1:] or ALL): ALL[n]()
