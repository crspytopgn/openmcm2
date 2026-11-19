"""Race-day gates for the free tracks: the checkpoint arches riders pass
through on Baja and Enduro courses (`waypnt` and its variants
`fp_waypnt_2` .. `fp_waypnt_4`) and the finish arch (`finish`).

Built like the real thing: inflatable arches swept as one pressurised
tube with sewn panel seams that pinch it, creases where it bends, the
sponsor printed into the fabric, a black base sleeve with ratchet straps
to ground stakes, sandbags, guy ropes and a blower on its duct; and an
aluminium truss gantry with a stretched vinyl banner on bungees. Every
sponsor is made up. Run with Blender's Python (bpy):

    python blender/gates.py [name ...]

Writes build/objects/<name>.glb, each fitted to the stock `waypnt` size
(38.7 ft wide, 20.3 ft tall, the opening kept clear).
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from mathutils import Vector
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from common import reset, flat as _flat, acg, box, cyl, join_all, export, BUILD, FREEPACK

V = Vector
FONTS = os.path.join(FREEPACK, "cache", "fonts")
TEX = os.path.join(BUILD, "tex", "gates")
HALF_W, TOP = 19.33, 20.33          # stock waypnt bounds (x half width, height)

def flat(name, rgb, rough=0.7, metal=0.0):
    """A plain-coloured material. The game shows a glTF colour factor
    about twice as bright as Blender does (it reads the factor's sRGB
    value as linear), so the factor goes in squared-down to match."""
    return _flat(name, tuple(c ** 2.2 for c in rgb), rough, metal)

def font(name, px):
    p = {"anton": "anton/Anton-Regular.ttf", "black": "archivoblack/ArchivoBlack-Regular.ttf",
         "xbold": "barlowcondensed/BarlowCondensed-ExtraBold.ttf", "bold": "barlowcondensed/BarlowCondensed-Bold.ttf",
         "semi": "barlowcondensed/BarlowCondensed-SemiBold.ttf"}[name]
    return ImageFont.truetype(os.path.join(FONTS, p), max(8, int(px)))

# ---------------------------------------------------------------- noise
def value_noise(h, w, cells, rng, octaves=4):
    """Smooth fractal noise in 0..1, (h, w)."""
    out = np.zeros((h, w), np.float32); amp = 1.0; tot = 0.0
    for o in range(octaves):
        cy, cx = max(2, int(cells[0] * 2 ** o)), max(2, int(cells[1] * 2 ** o))
        g = rng.random((cy + 1, cx + 1)).astype(np.float32)
        im = Image.fromarray((g * 255).astype(np.uint8)).resize((w, h), Image.BICUBIC)
        out += amp * np.asarray(im, np.float32) / 255; tot += amp; amp *= 0.5
    return out / tot

def splatter(h, w, rng, n, rmin, rmax, weight):
    """Mud flecks: n soft dots, denser where weight (h, w) is high."""
    m = Image.new("L", (w, h), 0); d = ImageDraw.Draw(m)
    flat_w = weight.ravel() / max(weight.sum(), 1e-6)
    idx = rng.choice(len(flat_w), size=n, p=flat_w)
    for i in idx:
        y, x = divmod(int(i), w); r = rng.uniform(rmin, rmax)
        d.ellipse([x - r, y - r * rng.uniform(0.6, 1.0), x + r, y + r], fill=int(rng.uniform(120, 255)))
    return np.asarray(m.filter(ImageFilter.GaussianBlur(0.8)), np.float32) / 255

def normal_from_height(hm, strength):
    gy, gx = np.gradient(hm.astype(np.float32))
    n = np.dstack((-gx * strength, gy * strength, np.ones_like(hm)))
    n /= np.linalg.norm(n, axis=2, keepdims=True)
    return Image.fromarray(((n * 0.5 + 0.5) * 255).astype(np.uint8))

def mat_tex(name, color_png, normal_png=None, rough=0.6, nscale=1.0, metal=0.0):
    if name in bpy.data.materials: return bpy.data.materials[name]
    m = bpy.data.materials.new(name); m.use_nodes = True; nt = m.node_tree; b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(color_png, check_existing=True)
    nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
    if normal_png:
        n = nt.nodes.new("ShaderNodeTexImage"); n.image = bpy.data.images.load(normal_png, check_existing=True)
        n.image.colorspace_settings.name = "Non-Color"
        nm = nt.nodes.new("ShaderNodeNormalMap"); nm.inputs["Strength"].default_value = nscale
        nt.links.new(n.outputs["Color"], nm.inputs["Color"]); nt.links.new(nm.outputs["Normal"], b.inputs["Normal"])
    b.inputs["Roughness"].default_value = rough; b.inputs["Metallic"].default_value = metal
    b.inputs["Specular IOR Level"].default_value = 0.3
    return m

def mesh(name, verts, faces, mat, uvs=None, smooth=True):
    me = bpy.data.meshes.new(name); me.from_pydata([tuple(v) for v in verts], [], faces); me.update()
    if uvs is not None:
        uv = me.uv_layers.new()
        for p in me.polygons:
            for li in p.loop_indices: uv.data[li].uv = uvs[me.loops[li].vertex_index]
    for p in me.polygons: p.use_smooth = smooth
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat)
    return o

# ---------------------------------------------------------------- paths
def resample(pts, step):
    """Polyline (list of (x, z)) -> evenly spaced points with arc length."""
    pts = [np.array(p, np.float64) for p in pts]
    seg = [np.linalg.norm(b - a) for a, b in zip(pts, pts[1:])]
    L = sum(seg); n = max(2, int(L / step) + 1)
    out = []; acc = [0.0]
    for s in seg: acc.append(acc[-1] + s)
    j = 0
    for k in range(n):
        s = L * k / (n - 1)
        while j < len(seg) - 1 and acc[j + 1] < s: j += 1
        t = (s - acc[j]) / max(seg[j], 1e-9)
        out.append((pts[j] + (pts[j + 1] - pts[j]) * t, s))
    return out, L

def square_path(cx, top, rc, foot, sag, lean, rng):
    """Centre line of a square inflatable arch, left foot -> right foot."""
    pts = []
    for k in range(40):   # left leg, a touch of lean and bow
        z = foot + (top - rc - foot) * k / 40
        f = (z - foot) / (top - foot)
        pts.append((-cx + lean * f + 0.18 * math.sin(math.pi * f), z))
    for k in range(25):   # corner
        a = math.pi - (math.pi / 2) * k / 24
        pts.append((-cx + lean + rc + rc * math.cos(a), top - rc + rc * math.sin(a)))
    xs = -cx + lean + rc; xe = cx - lean - rc
    for k in range(1, 60):
        x = xs + (xe - xs) * k / 60; f = (x - xs) / (xe - xs)
        pts.append((x, top - sag * math.sin(math.pi * f)))
    for k in range(25):
        a = math.pi / 2 - (math.pi / 2) * k / 24
        pts.append((cx - lean - rc + rc * math.cos(a), top - rc + rc * math.sin(a)))
    for k in range(1, 41):
        z = top - rc - (top - rc - foot) * k / 40
        f = (z - foot) / (top - foot)
        pts.append((cx - lean * f - 0.18 * math.sin(math.pi * f), z))
    return pts

def rainbow_path(a, b, foot):
    pts = []
    for k in range(161):
        t = math.pi * (1 - k / 160)
        # a squarer-than-ellipse top, as an arch on its own pressure stands
        c, s = math.cos(t), math.sin(t)
        x = a * math.copysign(abs(c) ** 0.85, c); z = foot + (b - foot) * abs(s) ** 0.75
        pts.append((x, z))
    return pts

# ---------------------------------------------------------------- tube
def sweep(pts, radius, seg, mat, step, seams, gores, corner_s=(), rng=None, name="tube", taper=None):
    """Sweep a fabric tube along pts (x, z) in the y = 0 plane.

    radius(s) is the nominal radius; seams are arc lengths where panels are
    sewn (the tube pinches in), gores the angles of the long seams; within
    3 ft of each corner_s the inner side creases. UV: u = s / L, v = angle.
    Returns (object, L, samples)."""
    samples, L = resample(pts, step)
    verts, uvs, faces = [], [], []
    seams = np.array(seams or [-99.0])
    wob = rng.random(8) * 2 * math.pi
    for i, (p, s) in enumerate(samples):
        a = samples[max(i - 1, 0)][0]; b = samples[min(i + 1, len(samples) - 1)][0]
        T = b - a; T /= np.linalg.norm(T)
        N = np.array([-T[1], T[0]])     # in-plane normal: cross(T, Y) = (-tz, 0, tx)
        r0 = radius(s)
        ds = np.min(np.abs(seams - s))
        pinch = 0.04 * math.exp(-(ds / 0.22) ** 2) - 0.012 * math.cos(math.pi * min(ds / 1.6, 1.0))
        cm = max([max(0.0, 1 - abs(s - c) / 3.0) for c in corner_s] or [0.0])
        # which side of N is the inside of the bend
        if 0 < i < len(samples) - 1:
            T2 = samples[i + 1][0] - p; T1 = p - samples[i - 1][0]
            bend = T1[0] * T2[1] - T1[1] * T2[0]
        else: bend = 0.0
        for j in range(seg + 1):
            th = 2 * math.pi * j / seg
            dg = min(abs(((th - g + math.pi) % (2 * math.pi)) - math.pi) for g in gores) if gores else 9
            r = r0 * (1 - pinch - 0.018 * math.exp(-(dg / 0.11) ** 2))
            # slack: a slow wander so it isn't a perfect pipe
            r *= 1 + 0.012 * math.sin(s * 0.9 + wob[0]) * math.cos(th * 2 + wob[1]) + 0.008 * math.sin(s * 2.3 + th * 3 + wob[2])
            if cm > 0:
                inner = -math.cos(th) * (1 if bend < 0 else -1)   # +1 on the inside of the bend
                if inner > 0:
                    fold = max(0.0, math.sin(s * 9.0 + th * 2.5 + wob[3])) ** 3
                    r *= 1 - cm * inner * (0.05 + 0.07 * fold)
            dirn = np.array([math.cos(th) * N[0], math.sin(th), math.cos(th) * N[1]])
            q = np.array([p[0], 0.0, p[1]]) + r * dirn
            verts.append(V((q[0], q[1], q[2]))); uvs.append((s / L, j / seg))
    n = seg + 1
    for i in range(len(samples) - 1):
        for j in range(seg):
            a = i * n + j; faces.append((a, a + 1, a + n + 1, a + n))
    o = mesh(name, verts, faces, mat, uvs)
    return o, L, samples

# ---------------------------------------------------------------- print
class Sheet:
    """The tube's fabric picture: u along the tube (ft), v round it (ft)."""
    def __init__(self, L, circ, w=4096, h=512, base=(200, 80, 20)):
        self.L, self.circ, self.w, self.h = L, circ, w, h
        self.kx, self.ky = w / L, h / circ
        self.im = Image.new("RGB", (w, h), base); self.d = ImageDraw.Draw(self.im)
        self.hm = np.zeros((h // 2, w // 2), np.float32)   # height for the normal map

    def px(self, u, v): return (u * self.kx, (self.circ - v) * self.ky)

    def rect(self, u0, u1, v0, v1, fill):
        x0, y1 = self.px(u0, v0); x1, y0 = self.px(u1, v1)
        self.d.rectangle([x0, y0, x1, y1], fill=fill)

    def text(self, s, face, u, v, height, fill, rot180=False, track=0.0, max_w=None):
        """Text centred at (u, v) ft, cap height `height` ft, upright in uv."""
        px = int(height * self.ky * 1.38)
        f = font(face, px)
        bb = f.getbbox(s); tw = bb[2] - bb[0] + track * px * len(s); th = bb[3] - bb[1]
        if max_w and tw > max_w * self.kx:
            return self.text(s, face, u, v, height * max_w * self.kx / tw, fill, rot180, track)
        layer = Image.new("RGBA", (int(tw + 20), int(th + 20)), (0, 0, 0, 0)); ld = ImageDraw.Draw(layer)
        x = 10 - bb[0]
        for ch in s:
            ld.text((x, 10 - bb[1]), ch, font=f, fill=fill); x += f.getlength(ch) + track * px
        if rot180: layer = layer.rotate(180)
        cx, cy = self.px(u, v)
        self.im.paste(layer, (int(cx - layer.width / 2), int(cy - layer.height / 2)), layer)
        return tw / self.kx

    def mark(self, style, u, v, size, fill, rot180=False):
        r = size * self.ky / 2; layer = Image.new("RGBA", (int(r * 2.4) + 4,) * 2, (0, 0, 0, 0)); d = ImageDraw.Draw(layer)
        c = layer.width / 2
        if style == "chevron":
            for k in range(2):
                o = (k - 0.5) * r * 0.55
                d.polygon([(c - r * 0.7 + o, c - r), (c - r * 0.25 + o, c - r), (c + r * 0.3 + o, c), (c - r * 0.25 + o, c + r), (c - r * 0.7 + o, c + r), (c - r * 0.15 + o, c)], fill=fill)
        elif style == "wing":
            d.polygon([(c - r, c + r * 0.6), (c + r, c - r), (c + r * 0.2, c + r * 0.1), (c + r * 0.6, c + r * 0.6)], fill=fill)
        elif style == "peak":
            d.polygon([(c - r, c + r * 0.7), (c - r * 0.2, c - r * 0.8), (c + r * 0.15, c - r * 0.1), (c + r * 0.4, c - r * 0.45), (c + r, c + r * 0.7)], fill=fill)
        elif style == "circle":
            d.ellipse([c - r, c - r, c + r, c + r], outline=fill, width=int(r * 0.3)); d.ellipse([c - r * 0.35, c - r * 0.35, c + r * 0.35, c + r * 0.35], fill=fill)
        if rot180: layer = layer.rotate(180)
        cx, cy = self.px(u, v); self.im.paste(layer, (int(cx - c), int(cy - c)), layer)

    def seam_u(self, u):    # a sewn ring round the tube
        x = u * self.kx; hx = x / 2
        a = np.asarray(self.im, np.float32)
        lo, hi = int(max(0, x - 3)), int(min(self.w, x + 3))
        a[:, lo:hi] *= 0.82; self.im = Image.fromarray(a.astype(np.uint8)); self.d = ImageDraw.Draw(self.im)
        xs = np.arange(self.hm.shape[1]); self.hm -= 1.0 * np.exp(-((xs - hx) / 1.6) ** 2)[None, :]

    def seam_v(self, v):    # a long seam down the tube
        y = (self.circ - v) * self.ky; hy = y / 2
        a = np.asarray(self.im, np.float32)
        lo, hi = int(max(0, y - 2)), int(min(self.h, y + 2))
        a[lo:hi, :] *= 0.86; self.im = Image.fromarray(a.astype(np.uint8)); self.d = ImageDraw.Draw(self.im)
        ys = np.arange(self.hm.shape[0]); self.hm -= 0.7 * np.exp(-((ys - hy) / 1.3) ** 2)[:, None]

    def weather(self, rng, foot_u=(), top_v=None, beam=None, dust=(150, 118, 84), fade=0.06):
        a = np.asarray(self.im, np.float32) / 255
        h, w = a.shape[:2]
        uu = (np.arange(w) / self.kx)[None, :]; vv = (self.circ - np.arange(h)[:, None] / self.ky)
        # sun-faded and soiled unevenly
        n = value_noise(h, w, (3, 40), rng)
        a = a * (1 - fade) + fade * 0.85 + (n[..., None] - 0.5) * 0.06
        dust_c = np.array(dust, np.float32) / 255
        mud = np.zeros((h, w), np.float32)
        for fu in foot_u:   # mud thrown up the feet, thickest at the ground
            d = np.abs(uu - fu)
            mud = np.maximum(mud, np.clip(1 - d / 3.2, 0, 1) ** 1.6 * np.ones((h, 1)))
        fine = value_noise(h, w, (8, 120), rng, 3)
        mud_mask = np.clip(mud * (0.55 + 0.9 * (fine - 0.5)), 0, 1) * 0.75
        flecks = splatter(h, w, rng, 2600, 0.8, 3.5, mud + 0.002) * np.clip(mud * 1.6, 0, 1)
        m = np.clip(mud_mask + flecks * 0.8, 0, 0.92)[..., None]
        a = a * (1 - m) + dust_c * 0.75 * m
        if top_v is not None and beam:   # dust settled along the top of the beam
            dv = np.abs(((vv - top_v + self.circ / 2) % self.circ) - self.circ / 2)
            inb = ((uu > beam[0]) & (uu < beam[1])).astype(np.float32)
            dm = np.clip(1 - dv / (self.circ * 0.16), 0, 1) * inb * (0.25 + 0.3 * fine)
            a = a * (1 - dm[..., None]) + dust_c * dm[..., None]
        # grime streaks running down from the top
        st = value_noise(h, w, (1, 260), rng, 2); st = np.clip((st - 0.62) * 3, 0, 1) * 0.08
        a = a * (1 - st[..., None])
        self.im = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8))
        # creases and slack: soft long folds plus the cloth's own grain
        hh, ww = self.hm.shape
        folds = value_noise(hh, ww, (2, 60), rng, 3) * 0.6 + value_noise(hh, ww, (16, 700), rng, 2) * 0.15
        self.hm += folds

    def save(self, stem):
        os.makedirs(TEX, exist_ok=True)
        c = os.path.join(TEX, stem + ".png"); n = os.path.join(TEX, stem + "_n.png")
        self.im.save(c); normal_from_height(self.hm, 4.0).save(n)
        return c, n

# ---------------------------------------------------------------- shared bits
def black_vinyl(): return flat("gate_base_vinyl", (0.022, 0.022, 0.024), 0.55)
def strap_mat(rgb=(0.55, 0.22, 0.01)): return flat(f"gate_strap{rgb}", rgb, 0.8)
def steel_stake(): return flat("gate_stake", (0.18, 0.17, 0.16), 0.5, 0.8)
def rope_mat(): return flat("gate_rope", (0.6, 0.58, 0.52), 0.9)
def alu(): return flat("gate_alu", (0.62, 0.63, 0.64), 0.32, 0.85)

def sandbag_mat():
    return acg("Fabric030", 0.6, ramp=((0.18, 0.14, 0.08), (0.5, 0.42, 0.28)), name="gate_sandbag")

def sandbag(c, rot, mat):
    """A filled sandbag lying on the ground: soft, slumped box."""
    bm = bmesh.new(); bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts: v.co = V((v.co.x * 2.0, v.co.y * 1.1, v.co.z * 0.55))
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=0.22, segments=3, affect="EDGES", profile=0.6)
    for v in bm.verts:
        v.co.z += 0.27 - 0.1 * (abs(v.co.x) / 1.0) ** 2   # slumps at the ends
        v.co.z = max(v.co.z, -0.05)
    from common import _box_uv; _box_uv(bm, 1.5)
    me = bpy.data.meshes.new("sandbag"); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    o = bpy.data.objects.new("sandbag", me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat)
    o.location = c; o.rotation_euler = (0, 0, rot)
    return o

def strap(a, b, mat, w=0.17):
    """A flat webbing strap from a to b (a ratchet halfway)."""
    a, b = V(a), V(b); d = b - a; L = d.length; t = d.normalized()
    side = t.cross(V((0, 0, 1))); side = side.normalized() if side.length > 1e-4 else V((1, 0, 0))
    up = side.cross(t).normalized()
    vs = []
    for k in range(9):
        p = a + d * (k / 8) - V((0, 0, 0.06 * math.sin(math.pi * k / 8)))
        vs += [p - side * w / 2, p + side * w / 2]
    fs = [(2 * k, 2 * k + 1, 2 * k + 3, 2 * k + 2) for k in range(8)]
    o = mesh("strap", vs, fs, mat, smooth=False); s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.03
    m = a + d * 0.55
    box(m - side * 0.12 - t * 0.2 - up * 0.04, m + side * 0.12 + t * 0.2 + up * 0.07, flat("gate_ratchet", (0.3, 0.3, 0.31), 0.4, 0.9))
    return o

def stake(p):
    cyl(V((p[0], p[1], -0.6)), V((p[0], p[1], 0.35)), 0.06, steel_stake(), 6)
    cyl(V((p[0], p[1], 0.3)), V((p[0], p[1], 0.38)), 0.13, steel_stake(), 8)

def rope(a, b, sag=0.25):
    a, b = V(a), V(b); pts = []
    for k in range(7):
        t = k / 6; pts.append(a.lerp(b, t) - V((0, 0, sag * math.sin(math.pi * t))))
    for p, q in zip(pts, pts[1:]): cyl(p, q, 0.035, rope_mat(), 5)
    stake(b)

def blower(x, side, y):
    """A cold-air blower beside a foot, its duct into the base sleeve."""
    red = flat("gate_blower_shell", (0.32, 0.03, 0.02), 0.45); grey = flat("gate_blower_grey", (0.05, 0.05, 0.055), 0.6)
    bx = x + side * 4.2
    cyl(V((bx, y - 0.45, 0.95)), V((bx, y + 0.45, 0.95)), 0.8, red, 24)                 # housing
    cyl(V((bx, y - 0.48, 0.95)), V((bx, y - 0.44, 0.95)), 0.7, grey, 24)                 # grille
    for k in range(5):   # guard bars over the grille
        gx = bx - 0.56 + k * 0.28; h = math.sqrt(max(0.0, 0.7 ** 2 - (gx - bx) ** 2))
        cyl(V((gx, y - 0.5, 0.95 - h)), V((gx, y - 0.5, 0.95 + h)), 0.025, grey, 4)
    for dx in (-0.45, 0.45):
        cyl(V((bx + dx, y - 0.3, 0.0)), V((bx + dx, y - 0.3, 0.5)), 0.05, grey, 6); cyl(V((bx + dx, y + 0.3, 0.0)), V((bx + dx, y + 0.3, 0.5)), 0.05, grey, 6)
    box(V((bx - 0.6, y - 0.4, 0.0)), V((bx + 0.6, y + 0.4, 0.12)), grey)
    # corrugated duct curving from the blower's outlet into the sleeve
    duct = flat("gate_duct", (0.03, 0.03, 0.032), 0.7)
    p0 = V((bx - side * 0.8, y, 0.95)); p3 = V((x + side * 2.0, y, 0.9)); prev = p0
    for k in range(1, 13):
        t = k / 12; p = p0.lerp(p3, t) + V((0, 0.35 * math.sin(math.pi * t), -0.25 * math.sin(math.pi * t)))
        cyl(prev, p, 0.42 + 0.03 * (k % 2), duct, 14); prev = p
    # power lead off across the ground
    lead = flat("gate_lead", (0.02, 0.02, 0.02), 0.6)
    q = V((bx + side * 0.3, y + 0.5, 0.05))
    for k in range(6):
        n = q + V((side * 1.6, 1.2 + 0.4 * math.sin(k), 0)); cyl(q, n, 0.04, lead, 5); q = n

def base_sleeve(x, R, foot_top):
    """The black base each leg stands in: wider, flared, with D-rings."""
    m = black_vinyl()
    cyl(V((x, 0, -3.0)), V((x, 0, 0.12)), R * 1.28, m, 28)
    o = cyl(V((x, 0, 0.12)), V((x, 0, foot_top)), R * 1.26, m, 28, r2=R * 1.12)
    cyl(V((x, 0, foot_top - 0.02)), V((x, 0, foot_top + 0.1)), R * 1.14, flat("gate_hem", (0.06, 0.06, 0.06), 0.6), 28)
    return o

def foot_dressing(x, R, rng, strap_rgb, sand):
    """Straps to stakes front, back and out; sandbags on the skirt."""
    out = -1 if x < 0 else 1
    for ang in (math.radians(a) for a in (-70, 0, 70, 180 - 35, 180 + 35)):
        dx, dy = math.cos(ang) * out, math.sin(ang)
        if ang > math.pi / 2 and abs(dy) < 0.6: continue
        a = V((x + dx * R * 1.2, dy * R * 1.2, 0.85)); b = V((x + dx * (R * 1.2 + 1.7), dy * (R * 1.2 + 1.7), 0.32))
        strap(a, b, strap_mat(strap_rgb)); stake(b)
    for k, ang in enumerate((math.radians(110 + rng.uniform(-10, 10)), math.radians(250 + rng.uniform(-10, 10)))):
        dx, dy = math.cos(ang) * out, math.sin(ang)
        sandbag(V((x + dx * (R * 1.35 + 0.75), dy * (R * 1.35 + 0.75), 0.0)), ang + math.pi / 2 + rng.uniform(-0.2, 0.2), sand)

def pennant_line(a, b, colors, n=12):
    """A string of small triangular pennants between a and b."""
    a, b = V(a), V(b)
    rope_ = rope_mat()
    pts = [a.lerp(b, k / n) - V((0, 0, 0.6 * math.sin(math.pi * k / n))) for k in range(n + 1)]
    for p, q in zip(pts, pts[1:]): cyl(p, q, 0.02, rope_, 4)
    for k, (p, q) in enumerate(zip(pts, pts[1:])):
        m = flat(f"gate_pennant{colors[k % len(colors)]}", colors[k % len(colors)], 0.75)
        tip = (p + q) / 2 - V((0, 0, 1.1))
        bm = bmesh.new(); vs = [bm.verts.new(v) for v in (p, q, tip + V((0, 0.12, 0)))]
        bm.faces.new(vs); me = bpy.data.meshes.new("pennant"); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new("pennant", me); bpy.context.scene.collection.objects.link(o); me.materials.append(m)
        s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.02

# ---------------------------------------------------------------- inflatables
STYLES = {
    # name: tube colour, print band colour, print ink, accent, brand, tagline, mark, leg words, strap
    "waypnt": dict(base=(214, 84, 18), band=(22, 22, 24), ink=(238, 236, 228), acc=(214, 84, 18),
                   brand="RIDGEBACK", tag="ENGINE OILS", mark="chevron", leg="CHECKPOINT", legink=(24, 24, 26), strap=(0.02, 0.02, 0.02), shape="square"),
    "fp_waypnt_2": dict(base=(232, 230, 222), alt=(178, 28, 24), band=(178, 28, 24), ink=(244, 242, 236), acc=(24, 24, 26),
                   brand="NORTHFORK", tag="TYRES", mark="circle", leg="CHECKPOINT", legink=(178, 28, 24), strap=(0.4, 0.3, 0.0), shape="rainbow"),
    "fp_waypnt_4": dict(base=(26, 64, 128), band=(236, 234, 226), ink=(26, 52, 104), acc=(230, 170, 20),
                   brand="KESTREL", tag="HELMETS", mark="wing", leg="CHECKPOINT", legink=(236, 234, 226), strap=(0.45, 0.32, 0.0), shape="square"),
    "finish": dict(base=(20, 20, 22), band=(238, 236, 228), ink=(20, 20, 22), acc=(238, 236, 228),
                   brand="FINISH", tag="", mark=None, leg="SLATE PEAK", legink=(238, 236, 228), strap=(0.02, 0.02, 0.02), shape="square", checker=True),
}

def inflatable(name, st, rng):
    R = 1.62; seg = 28; foot = -3.0
    if st["shape"] == "square":
        cx = HALF_W - R * 1.3 - 0.05; top = TOP - R - 0.02; rc = 2.3
        pts = square_path(cx, top, rc, foot, 0.22, rng.uniform(0.1, 0.25), rng)
        leg_len = (top - rc - foot); corner = leg_len + rc * math.pi / 4
    else:
        cx = HALF_W - R * 1.3 - 0.05; top = TOP - R - 0.02
        pts = rainbow_path(cx, top, foot)
        leg_len = None
    samples, L = resample(pts, 0.4)
    circ = 2 * math.pi * R
    if st["shape"] == "square":
        corners = [corner, L - corner]; beam = (corners[0] + 1.6, corners[1] - 1.6)
        seams = [foot + 3.0 + 3.2 * k - foot for k in range(int(leg_len / 3.2))]
        seams = [s for s in seams if s < leg_len - 0.5]
        seams += [corner - 1.4, corner + 1.4]
        nb = max(3, round((beam[1] - beam[0]) / 4.2))
        seams += [beam[0] + (beam[1] - beam[0]) * k / nb for k in range(nb + 1)]
        seams += [L - s for s in seams if s < corner + 2]
    else:
        corners = []; beam = (L * 0.2, L * 0.8)
        n = 13; seams = [3.0 + (L - 6.0) * k / n for k in range(n + 1)]
    seams = sorted(set(round(s, 3) for s in seams))
    # ---- the fabric picture
    alt = st.get("alt")
    sh = Sheet(L, circ, base=st["base"])
    if alt:   # rainbow arch: alternate panel colours, the classic look
        for k, (s0, s1) in enumerate(zip(seams, seams[1:])):
            if k % 2: sh.rect(s0, s1, 0, circ, alt)
    vf, vb = circ * 0.75, circ * 0.25          # front / back faces (v ft)
    band_h = circ * 0.25 * 0.92
    if st.get("checker"):
        for vc, r180 in ((vf, False), (vb, True)):
            sq = band_h / 4
            u = beam[0] - 1.0
            while u < beam[1] + 1.0:
                for row in range(4):
                    k = int((u - beam[0]) / sq) + row
                    sh.rect(u, u + sq, vc - band_h / 2 + row * sq, vc - band_h / 2 + (row + 1) * sq, (236, 234, 226) if k % 2 else (18, 18, 20))
                u += sq
            mid = (beam[0] + beam[1]) / 2
            sh.rect(mid - 7.5, mid + 7.5, vc - band_h / 2, vc + band_h / 2, (18, 18, 20))
            sh.text("FINISH", "anton", mid, vc, band_h * 0.62, (238, 236, 228), r180, track=0.08, max_w=14)
    else:
        for vc, r180 in ((vf, False), (vb, True)):
            mid = (beam[0] + beam[1]) / 2
            bw = min(26.0, (beam[1] - beam[0]) * 0.92) if st["shape"] == "square" else (beam[1] - beam[0])
            sh.rect(mid - bw / 2, mid + bw / 2, vc - band_h / 2, vc + band_h / 2, st["band"])
            # thin accent rule above and below the band
            for e in (-1, 1):
                sh.rect(mid - bw / 2, mid + bw / 2, vc + e * band_h / 2 - 0.05, vc + e * band_h / 2 + 0.05, st["acc"])
            sgn = -1 if r180 else 1
            tw = sh.text(st["brand"], "anton", mid + sgn * 1.4, vc + sgn * 0.08, band_h * 0.6, st["ink"], r180, track=0.04, max_w=bw * 0.5)
            if st["mark"]: sh.mark(st["mark"], mid + sgn * (1.4 - tw / 2 - 1.5), vc, band_h * 0.8, st["acc"], r180)
            if st["shape"] == "rainbow":
                for e in (-1, 1):
                    sh.text(st["tag"], "xbold", mid + e * 8.5, vc, band_h * 0.34, st["ink"], r180, track=0.12)
                    sh.text(st["leg"], "xbold", mid + e * (bw / 2 - 3.6), vc, band_h * 0.42, st["ink"], r180, track=0.12)
            elif st["tag"]:
                for e in (-1, 1):
                    sh.text(st["tag"], "xbold", mid + e * (bw / 2 - 3.2), vc, band_h * 0.32, st["ink"], r180, track=0.12)
    # legs: words running up (left) and down (right), on both faces
    if st["shape"] == "square":
        for vc, r180 in ((vf, False), (vb, True)):
            for u in (leg_len * 0.55, L - leg_len * 0.55):
                sh.text(st["leg"], "xbold", u, vc, band_h * 0.5, st["legink"], r180, track=0.14, max_w=leg_len * 0.62)
    for s in seams: sh.seam_u(s)
    gores = [math.pi / 4, 3 * math.pi / 4, 5 * math.pi / 4, 7 * math.pi / 4]
    for g in gores: sh.seam_v(g / (2 * math.pi) * circ)
    sh.weather(rng, foot_u=(-foot + 0.0, L + foot), top_v=0.0, beam=beam)
    c, n = sh.save(name)
    m = mat_tex(f"gate_fabric_{name}", c, n, rough=0.58, nscale=0.7)
    def radius(s):
        if leg_len is None:
            d = min(s, L - s); return R * (1 + 0.1 * max(0, 1 - d / 9))
        d = min(s, L - s)
        return R * (1 + 0.09 * max(0.0, 1 - d / leg_len))   # legs a touch fatter low down
    sweep(pts, radius, seg, m, 0.4, seams, gores, corners, rng, name="tube")
    # ---- base, straps, sandbags, ropes, blower
    sand = sandbag_mat()
    feet_x = [samples[0][0][0], samples[-1][0][0]]
    for x in feet_x:
        base_sleeve(x, radius(0.0) / 1.0, 1.45)
        foot_dressing(x, R, rng, st["strap"], sand)
    ty = top + R * 0.4 if st["shape"] == "square" else top
    for x in feet_x:
        out = -1 if x < 0 else 1
        anchor_x = (x - out * (rc if st["shape"] == "square" else 6.0))
        for dy in (-1, 1):
            rope(V((anchor_x, dy * R * 0.7, ty - (0.6 if st["shape"] == "square" else 1.4))), V((x + out * 7.5, dy * 4.0, 0.3)))
    blower(feet_x[1], 1, 0.0)

# ---------------------------------------------------------------- truss gantry
def truss_mast(a, b, w, m, step=1.6):
    """Square aluminium box truss from a to b (vertical or horizontal)."""
    a, b = V(a), V(b); d = b - a; L = d.length; t = d.normalized()
    u = V((0, 1, 0)) if abs(t.y) < 0.9 else V((1, 0, 0))
    v = t.cross(u).normalized(); u = v.cross(t).normalized()
    cs = [(-1, -1), (1, -1), (1, 1), (-1, 1)]
    corners = [(u * cu + v * cv) * (w / 2) for cu, cv in cs]
    for c in corners: cyl(a + c, b + c, 0.085, m, 8)
    n = max(1, int(L / step))
    for k in range(n):
        p0 = a + t * (L * k / n); p1 = a + t * (L * (k + 1) / n)
        for i in range(4):
            c0, c1 = corners[i], corners[(i + 1) % 4]
            cyl(p0 + c0, p1 + c1, 0.045, m, 5)   # diagonal brace
            cyl(p0 + c0, p0 + c1, 0.04, m, 5)
    for c0, c1 in zip(corners, corners[1:] + corners[:1]): cyl(b + c0, b + c1, 0.04, m, 5)

def banner_sheet(name, st, w_ft, h_ft):
    W, H = 2048, int(2048 * h_ft / w_ft)
    im = Image.new("RGB", (W, H), st["band"]); d = ImageDraw.Draw(im); k = W / w_ft
    # a fat sponsor panel left, CHECKPOINT right, a hazard rule along the foot
    d.rectangle([0, 0, W * 0.46, H], fill=st["acc"])
    f = font("anton", H * 0.62); s = st["brand"]
    tw = d.textlength(s, font=f)
    if tw > W * 0.42: f = font("anton", H * 0.62 * W * 0.42 / tw); tw = d.textlength(s, font=f)
    d.text((W * 0.23, H * 0.45), s, font=f, fill=st["ink2"], anchor="mm")
    f2 = font("xbold", H * 0.2); d.text((W * 0.23, H * 0.82), st["tag"], font=f2, fill=st["ink2"], anchor="mm")
    f3 = font("anton", H * 0.5); d.text((W * 0.73, H * 0.42), "CHECKPOINT", font=f3, fill=st["ink"], anchor="mm")
    f4 = font("xbold", H * 0.16); d.text((W * 0.73, H * 0.8), "TIMING  •  ENDURO  •  BAJA", font=f4, fill=st["ink"], anchor="mm")
    # grommets along the hem
    for i in range(14):
        x = 24 + (W - 48) * i / 13
        for y in (20, H - 20): d.ellipse([x - 9, y - 9, x + 9, y + 9], fill=(150, 150, 150)); d.ellipse([x - 4, y - 4, x + 4, y + 4], fill=(30, 30, 30))
    rng = np.random.default_rng(7)
    a = np.asarray(im, np.float32) / 255
    n = value_noise(H, W, (3, 12), rng); a = a * 0.95 + 0.04 + (n[..., None] - 0.5) * 0.05
    dust = np.array((150, 120, 88), np.float32) / 255
    g = (np.arange(H) / H)[:, None] ** 3 * (0.15 + 0.25 * value_noise(H, W, (4, 30), rng))
    a = a * (1 - g[..., None]) + dust * g[..., None]
    im = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8))
    os.makedirs(TEX, exist_ok=True); p = os.path.join(TEX, name + "_banner.png"); im.save(p)
    hm = value_noise(H // 2, W // 2, (2, 6), rng) * 1.5 + value_noise(H // 2, W // 2, (10, 40), rng) * 0.2
    pn = os.path.join(TEX, name + "_banner_n.png"); normal_from_height(hm, 3.0).save(pn)
    return p, pn

def gantry(name, st, rng):
    m = alu(); w = 1.0
    xs = (-HALF_W + 1.2, HALF_W - 1.2); top = TOP - 0.6
    for x in xs:
        truss_mast((x, 0, 0.3), (x, 0, top - w / 2), w, m)
        box(V((x - 1.3, -1.3, 0)), V((x + 1.3, 1.3, 0.3)), flat("gate_baseplate", (0.12, 0.12, 0.12), 0.6, 0.7))
        sand = sandbag_mat()
        for k, (dx, dy, r) in enumerate(((0, -1.9, 0), (0, 1.9, 0), (-1.0 if x < 0 else 1.0, 0, math.pi / 2))):
            sandbag(V((x + dx * 1.0 + (-0.9 if x < 0 else 0.9) * (k == 2), dy, 0.0)), r + rng.uniform(-0.15, 0.15), sand)
    truss_mast((xs[0] - w / 2, 0, top), (xs[1] + w / 2, 0, top), w, m)
    # the banner, tensioned under the top truss on its front and back
    bw, bh = (xs[1] - xs[0]) - 3.2, 4.4
    p, pn = banner_sheet(name, st, bw, bh)
    bm_ = mat_tex(f"gate_banner_{name}", p, pn, rough=0.7, nscale=0.6)
    z1 = top - w / 2 - 0.35; z0 = z1 - bh
    for y, side in ((-w / 2 - 0.25, -1), (w / 2 + 0.25, 1)):
        nx, nz = 30, 6; vs, uvs, fs = [], [], []
        for j in range(nz + 1):
            for i in range(nx + 1):
                u, v = i / nx, j / nz; x = -bw / 2 + bw * u
                sag = 0.28 * math.sin(math.pi * u) * (1 - v) + 0.06 * math.sin(u * 37 + v * 3) * math.sin(math.pi * u)
                vs.append(V((x, y + side * (0.12 * math.sin(math.pi * u) * math.sin(math.pi * v) + 0.04 * math.sin(u * 23)), z0 + bh * v - sag * (1 - v * 0.6))))
                uvs.append((u, v) if side < 0 else (1 - u, v))
        for j in range(nz):
            for i in range(nx):
                a = j * (nx + 1) + i; f = (a, a + 1, a + nx + 2, a + nx + 1); fs.append(f if side < 0 else f[::-1])
        o = mesh("banner", vs, fs, bm_, uvs); s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.02
        # bungees from the grommets up to the truss
        bung = flat("gate_bungee", (0.02, 0.02, 0.2), 0.7)
        for i in range(14):
            x = -bw / 2 + 0.3 + (bw - 0.6) * i / 13
            cyl(V((x, y, z1 - 0.15)), V((x, y * 0.6, top - w / 2)), 0.03, bung, 4)
        for x in (-bw / 2 - 0.05, bw / 2 + 0.05):
            for z in (z0 + 0.2, z1 - 0.2):
                cyl(V((x, y, z)), V((xs[0] if x < 0 else xs[1], y * 0.5, z)), 0.03, bung, 4)
    # timing box and a beacon on the left mast, a flag on the right
    box(V((xs[0] + 0.55, -0.7, 4.0)), V((xs[0] + 1.35, 0.1, 5.2)), flat("gate_timer", (0.04, 0.04, 0.045), 0.5))
    box(V((xs[0] + 0.7, -0.74, 4.6)), V((xs[0] + 1.2, -0.7, 5.0)), flat("gate_timer_lcd", (0.05, 0.12, 0.06), 0.3))
    cyl(V((xs[0], 0, top + w / 2)), V((xs[0], 0, top + w / 2 + 0.5)), 0.22, flat("gate_beacon", (0.6, 0.18, 0.0), 0.3), 12)
    cyl(V((xs[1], 0, top + w / 2)), V((xs[1], 0, top + w / 2 + 6)), 0.05, alu(), 6)
    flagm = flat("gate_flag_check", (0.55, 0.36, 0.0), 0.75)
    vs = [V((xs[1], 0, top + w / 2 + 6)), V((xs[1], 0, top + w / 2 + 4)), V((xs[1] + 2.8, 0.3, top + w / 2 + 5.3)), V((xs[1] + 2.6, 0.2, top + w / 2 + 6.4))]
    o = mesh("flag", vs, [(0, 1, 2, 3)], flagm, smooth=False); s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.02
    # pennant strings off the masts' tops to stakes outside
    for x in xs:
        out = -1 if x < 0 else 1
        rope(V((x, 0.0, top)), V((x + out * 8.0, 0.0, 0.3)), sag=0.4)
        pennant_line(V((x, 0.0, top - 0.3)), V((x + out * 7.6, 0.0, 0.9)), [(0.6, 0.05, 0.02), (0.75, 0.72, 0.68), (0.03, 0.06, 0.25), (0.6, 0.4, 0.0)], n=8)

GANTRY = {"fp_waypnt_3": dict(band=(236, 234, 228), acc=(36, 120, 64), ink=(24, 24, 26), ink2=(244, 242, 236), brand="BLUEGUM", tag="RACING  •  SINCE 1978")}

def build(name):
    reset(); rng = np.random.default_rng(sum(map(ord, name)))
    random.seed(name)
    if name in GANTRY: gantry(name, GANTRY[name], rng)
    else: inflatable(name, STYLES[name], rng)
    # one UV layer name on every part, or joining drops the odd ones out
    for o in bpy.context.scene.objects:
        if o.type != "MESH": continue
        if not o.data.uv_layers: o.data.uv_layers.new(name="UVMap")
        o.data.uv_layers[0].name = "UVMap"
    join_all(name)
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    o = bpy.context.scene.objects[name]
    print("built", name, len(o.data.vertices), "verts", os.path.getsize(path) // 1024, "KB", flush=True)

ALL = ["waypnt", "fp_waypnt_2", "fp_waypnt_3", "fp_waypnt_4", "finish"]
if __name__ == "__main__":
    names = [a for a in sys.argv[1:] if not a.startswith("-")] or ALL
    for n in names: build(n)
