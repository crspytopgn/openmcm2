"""Free Pack stand-ins for the bigger and odder stock objects: silos, bridges,
towers, wires, barns, lodges, track pieces and small yard props. Our own
procedural models on CC0 ambientCG materials, fitted to the stock bounds.
objects.py dispatches here through STRUCT (name regex -> builder).

Model space as in common.py: X = game x, Y = game z, Z = game y (up), feet.
"""
import math, random, re
import bpy, bmesh
from mathutils import Vector, Matrix
from common import acg, flat, box, cyl, _box_uv
from objects import wood, steel, galv, concrete, siding, roofing, glass, rubber, plywood, brand_mat, brand_for, plastic, paint_for, tyre_mat

V = Vector
def T(mid, tile, **kw): return acg(mid, 1.0 / tile, name=kw.pop("name", f"{mid}_{tile}_{kw.get('ramp')}_{kw.get('tint')}"), **kw)
def bricks(): return T("Bricks059", 6)
def stone(): return T("Rock035", 8)
def rock(): return T("Rock030", 12)
def planks(): return T("Planks023A", 6)
def oldwood(): return T("Planks012", 6)
def sidingw(): return T("WoodSiding008", 8)
def tiles(): return T("RoofingTiles006", 6)
def rust(): return T("Rust004", 6, metal=0.4)
def plaster(): return T("PaintedPlaster017", 8)
def conc(): return T("Concrete031", 10)
def asphalt(): return T("Asphalt012", 12)
def plates(): return T("MetalPlates013", 6, metal=0.8, rough=0.35)
def corr(rgb): return acg("CorrugatedSteel007A", 0.2, ramp=(tuple(c * 0.45 for c in rgb), tuple(rgb)), metal=0.4, name=f"corr{rgb}")
def paintm(rgb, name=None, rough=0.45, metal=0.2): return flat(name or f"paint{rgb}", rgb, rough, metal)
def emit(name, rgb):
    m = flat(name, rgb, 0.3); b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Emission Color"].default_value = (*rgb, 1); b.inputs["Emission Strength"].default_value = 3.0; return m

def mesh(name, verts, faces, mat, uv_s=1.0, smooth=False):
    bm = bmesh.new(); vs = [bm.verts.new(v) for v in verts]
    for f in faces:
        try: bm.faces.new([vs[i] for i in f])
        except ValueError: pass
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces); _box_uv(bm, uv_s)
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = smooth
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat); return o

def prism(pts2, y0, y1, mat, axis="y", uv_s=1.0):
    """Extrude a 2D outline [(a, z)] between y0 and y1 along an axis (x or y)."""
    n = len(pts2)
    mk = (lambda a, z, y: V((a, y, z))) if axis == "y" else (lambda a, z, y: V((y, a, z)))
    vs = [mk(a, z, y0) for a, z in pts2] + [mk(a, z, y1) for a, z in pts2]
    fs = [list(range(n)), list(range(2 * n - 1, n - 1, -1))] + [[i, (i + 1) % n, n + (i + 1) % n, n + i] for i in range(n)]
    return mesh("prism", vs, fs, mat, uv_s)

def lathe(prof, c, mat, seg=48, smooth=True, uv_s=1.0):
    """Revolve [(r, z)] about the vertical through c."""
    vs, fs = [], []
    for r, z in prof:
        for j in range(seg):
            a = 2 * math.pi * j / seg; vs.append(c + V((r * math.cos(a), r * math.sin(a), z)))
    for i in range(len(prof) - 1):
        for j in range(seg): fs.append([i * seg + j, i * seg + (j + 1) % seg, (i + 1) * seg + (j + 1) % seg, (i + 1) * seg + j])
    return mesh("lathe", vs, fs, mat, uv_s, smooth)

def beam(a, b, w, mat, h=None):
    """Square-section member from a to b."""
    a, b = V(a), V(b); d = b - a; L = d.length; h = h or w
    o = box(V((-w / 2, -h / 2, 0)), V((w / 2, h / 2, L)), mat)
    o.location = a; o.rotation_euler = d.to_track_quat("Z", "Y").to_euler(); return o

def catenary(a, b, sag, r, mat, n=20):
    a, b = V(a), V(b); pts = []
    for i in range(n + 1):
        t = i / n; p = a.lerp(b, t); p.z -= sag * 4 * t * (1 - t); pts.append(p)
    for p, q in zip(pts, pts[1:]): cyl(p, q, r, mat, 6, cap=False)

def lattice(c, z0, z1, w0, w1, mat, panels=6, leg=0.6, brace=0.3):
    """Four-legged tapered lattice tower with X bracing."""
    def corner(z, sx, sy):
        t = (z - z0) / (z1 - z0); w = w0 + (w1 - w0) * t; return c + V((sx * w / 2, sy * w / 2, z))
    cs = [(-1, -1), (1, -1), (1, 1), (-1, 1)]
    for sx, sy in cs: cyl(corner(z0, sx, sy), corner(z1, sx, sy), leg, mat, 6)
    for k in range(panels):
        za = z0 + (z1 - z0) * k / panels; zb = z0 + (z1 - z0) * (k + 1) / panels
        for i in range(4):
            p, q = cs[i], cs[(i + 1) % 4]
            cyl(corner(za, *p), corner(zb, *q), brace, mat, 4); cyl(corner(za, *q), corner(zb, *p), brace, mat, 4)
            cyl(corner(zb, *p), corner(zb, *q), brace, mat, 4)

def long_axis(lo, hi): return "x" if (hi.x - lo.x) >= (hi.y - lo.y) else "y"

def windows(lo, hi, z0, z1, rows, mat_frame, side_inset=0.0, every=10.0, ww=4.0, wh=5.0):
    """Window grid on all four faces of a box lo..hi between heights z0..z1."""
    gl = glass()
    for r in range(rows):
        zc = z0 + (z1 - z0) * (r + 0.5) / rows
        for ax in ("x", "y"):
            a0, a1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y)
            n = max(1, int((a1 - a0) / every))
            for k in range(n):
                ac = a0 + (a1 - a0) * (k + 0.5) / n
                for side in (0, 1):
                    if ax == "x":
                        y = (lo.y - 0.15) if side == 0 else (hi.y + 0.15)
                        box(V((ac - ww / 2, y - 0.1, zc - wh / 2)), V((ac + ww / 2, y + 0.1, zc + wh / 2)), gl)
                        box(V((ac - ww / 2 - 0.3, y - 0.2, zc - wh / 2 - 0.4)), V((ac + ww / 2 + 0.3, y + 0.2, zc - wh / 2)), mat_frame)
                    else:
                        x = (lo.x - 0.15) if side == 0 else (hi.x + 0.15)
                        box(V((x - 0.1, ac - ww / 2, zc - wh / 2)), V((x + 0.1, ac + ww / 2, zc + wh / 2)), gl)
                        box(V((x - 0.2, ac - ww / 2 - 0.3, zc - wh / 2 - 0.4)), V((x + 0.2, ac + ww / 2 + 0.3, zc - wh / 2)), mat_frame)

def gable_roof(lo, hi, z0, z1, mat, over=1.5, along=None):
    along = along or long_axis(lo, hi)
    if along == "x":
        ym = (lo.y + hi.y) / 2
        pts = [(lo.y - over, z0 - over * 0.5), (ym, z1), (hi.y + over, z0 - over * 0.5), (hi.y + over, z0 - over * 0.5 + 0.6), (ym, z1 + 0.6), (lo.y - over, z0 - over * 0.5 + 0.6)]
        o = prism([(p[0], p[1]) for p in pts], lo.x - over, hi.x + over, mat, axis="x", uv_s=1.0)
    else:
        xm = (lo.x + hi.x) / 2
        pts = [(lo.x - over, z0 - over * 0.5), (xm, z1), (hi.x + over, z0 - over * 0.5), (hi.x + over, z0 - over * 0.5 + 0.6), (xm, z1 + 0.6), (lo.x - over, z0 - over * 0.5 + 0.6)]
        o = prism(pts, lo.y - over, hi.y + over, mat, axis="y", uv_s=1.0)
    return o

def gable_ends(lo, hi, z0, z1, mat, along=None):
    along = along or long_axis(lo, hi)
    if along == "x":
        ym = (lo.y + hi.y) / 2
        for x in (lo.x, hi.x): prism([(lo.y, z0), (hi.y, z0), (ym, z1)], x - 0.2, x + 0.2, mat, axis="x")
    else:
        xm = (lo.x + hi.x) / 2
        for y in (lo.y, hi.y): prism([(lo.x, z0), (hi.x, z0), (xm, z1)], y - 0.2, y + 0.2, mat, axis="y")

def house(lo, hi, wall, roof, base=None, base_h=0.0, wall_frac=0.62, rows=None, trim=None, door=True, flat_roof=False):
    """General building: optional stone base, walls with windows, gable or flat roof."""
    trim = trim or paintm((0.85, 0.84, 0.8), "trim")
    z0 = lo.z; H = hi.z - z0
    if base:
        box(V((lo.x, lo.y, z0)), V((hi.x, hi.y, z0 + base_h)), base); z0 += base_h; H -= base_h
    zt = z0 + H * (1.0 if flat_roof else wall_frac)
    box(V((lo.x + 0.3, lo.y + 0.3, z0)), V((hi.x - 0.3, hi.y - 0.3, zt)), wall)
    floors = rows or max(1, int((zt - z0) / 11))
    windows(V((lo.x + 0.3, lo.y + 0.3, 0)), V((hi.x - 0.3, hi.y - 0.3, 0)), z0 + 1.5, zt - 1.0, floors, trim, every=max(9.0, (hi.x - lo.x) / 18))
    if flat_roof:
        box(V((lo.x, lo.y, zt)), V((hi.x, hi.y, zt + 0.6)), conc())
        for (a, b) in ((V((lo.x, lo.y, zt)), V((hi.x, lo.y + 1, zt + 3))), (V((lo.x, hi.y - 1, zt)), V((hi.x, hi.y, zt + 3))),
                       (V((lo.x, lo.y, zt)), V((lo.x + 1, hi.y, zt + 3))), (V((hi.x - 1, lo.y, zt)), V((hi.x, hi.y, zt + 3)))):
            box(a, b, wall)
    else:
        gable_ends(V((lo.x + 0.3, lo.y + 0.3, 0)), V((hi.x - 0.3, hi.y - 0.3, 0)), zt, hi.z - 0.6, wall)
        gable_roof(V((lo.x + 1.5, lo.y + 1.5, 0)), V((hi.x - 1.5, hi.y - 1.5, 0)), zt, hi.z - 0.6, roof, over=1.5)
    if door:
        dx = (lo.x + hi.x) / 2
        box(V((dx - 2.2, lo.y + 0.05, z0)), V((dx + 2.2, lo.y + 0.35, z0 + min(7.5, (zt - z0) * 0.8))), paintm((0.3, 0.18, 0.1), "door", 0.6, 0.0))

# ---------------- buildings ----------------
def b_lodge(lo, hi, name):
    house(lo, hi, sidingw(), tiles(), base=stone(), base_h=max(0.0, -lo.z) + 4, wall_frac=0.6)
    # chimneys
    for f in (0.2, 0.8):
        x = lo.x + (hi.x - lo.x) * f; y = (lo.y + hi.y) / 2
        box(V((x - 3, y - 3, hi.z - 12)), V((x + 3, y + 3, hi.z + 3)), stone())

def b_brick(lo, hi, name):
    house(lo, hi, bricks(), roofing(), flat_roof=True)
    box(V((lo.x - 0.3, lo.y - 0.3, lo.z)), V((hi.x + 0.3, hi.y + 0.3, lo.z + 3)), conc())

def b_shed_metal(lo, hi, name):
    """Big steel shed with roller doors along the front."""
    H = hi.z - lo.z; zt = lo.z + H * 0.78
    box(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, zt)), corr((0.62, 0.63, 0.62)))
    gable_roof(lo, hi, zt, hi.z, corr((0.3, 0.32, 0.36)), over=1.0)
    gable_ends(lo, hi, zt, hi.z, corr((0.62, 0.63, 0.62)))
    n = max(2, int((hi.x - lo.x) / 45))
    for k in range(n):
        xc = lo.x + (hi.x - lo.x) * (k + 0.5) / n; dw = min(30, (hi.x - lo.x) / n * 0.7)
        box(V((xc - dw / 2, lo.y - 0.3, lo.z)), V((xc + dw / 2, lo.y + 0.1, lo.z + min(22, (zt - lo.z) * 0.8))), corr((0.22, 0.32, 0.45)))
    box(V((lo.x - 1, lo.y - 1, lo.z - 0.5)), V((hi.x + 1, hi.y + 1, lo.z + 0.3)), conc())

def b_shack(lo, hi, name):
    """Small timber shack; on stilts when the bounds go well below ground."""
    W = hi.x - lo.x; D = hi.y - lo.y; c = (lo + hi) / 2
    sw, sd = min(W, 34), min(D, 30); top = hi.z; sh = min(18.0, (hi.z - lo.z) * 0.6)
    zf = top - sh
    s_lo = V((c.x - sw / 2, c.y - sd / 2, zf)); s_hi = V((c.x + sw / 2, c.y + sd / 2, top))
    house(s_lo, s_hi, oldwood(), corr((0.45, 0.3, 0.2)), wall_frac=0.7, rows=1)
    if zf - lo.z > 3:   # stilts and bracing down the slope
        box(V((s_lo.x - 1, s_lo.y - 1, zf - 1)), V((s_hi.x + 1, s_hi.y + 1, zf)), oldwood())
        legs = [(s_lo.x, s_lo.y), (s_hi.x, s_lo.y), (s_hi.x, s_hi.y), (s_lo.x, s_hi.y), (c.x, s_lo.y), (c.x, s_hi.y)]
        for x, y in legs: beam(V((x, y, lo.z)), V((x, y, zf)), 1.0, oldwood())
        for k in range(int((zf - lo.z) / 14)):
            z = lo.z + 7 + k * 14
            beam(V((s_lo.x, s_lo.y, z)), V((s_hi.x, s_lo.y, z + 10)), 0.6, oldwood()); beam(V((s_lo.x, s_hi.y, z)), V((s_hi.x, s_hi.y, z + 10)), 0.6, oldwood())

def b_silo(lo, hi, name):
    """Concrete grain elevator: a bank of silos, a headhouse and a gallery."""
    W, D, H = hi.x - lo.x, hi.y - lo.y, hi.z - lo.z
    along_x = W >= D; L, S = (W, D) if along_x else (D, W)
    rows = 2 if S > 0.45 * L else 1; r = min(S / rows, L / 4) / 2 * 0.96
    cols = int((L * 0.78) // (2 * r)); hs = H * 0.78
    for i in range(cols):
        for j in range(rows):
            a = (lo.x if along_x else lo.y) + r + 2 * r * i; b = (lo.y if along_x else lo.x) + S / 2 + (j - (rows - 1) / 2) * 2 * r
            p = V((a, b, lo.z)) if along_x else V((b, a, lo.z))
            cyl(p, p + V((0, 0, hs)), r, conc(), 40, uv_scale=1.0)
            cyl(p + V((0, 0, hs)), p + V((0, 0, hs + r * 0.25)), r, conc(), 40, r2=r * 0.25)
    # headhouse at the far end
    a0 = (lo.x if along_x else lo.y) + 2 * r * cols + 2; a1 = (hi.x if along_x else hi.y)
    hl = V((a0, lo.y + S * 0.2, lo.z)) if along_x else V((lo.x + S * 0.2, a0, lo.z))
    hh = V((a1, lo.y + S * 0.8, hi.z)) if along_x else V((lo.x + S * 0.8, a1, hi.z))
    house(hl, hh, conc(), roofing(), flat_roof=True, door=True)
    # gallery along the tops
    g0 = V(((lo.x, lo.y + S / 2 - 3, lo.z + hs + r * 0.25)) if along_x else (lo.x + S / 2 - 3, lo.y, lo.z + hs + r * 0.25))
    g1 = V(((a0, lo.y + S / 2 + 3, lo.z + hs + r * 0.25 + 8)) if along_x else (lo.x + S / 2 + 3, a0, lo.z + hs + r * 0.25 + 8))
    box(g0, g1, corr((0.55, 0.56, 0.55)))

def b_observatory(lo, hi, name):
    c = (lo + hi) / 2; R = min(hi.x - lo.x, hi.y - lo.y) / 2; H = hi.z - lo.z
    drum = H - R * 0.95
    box(V((c.x - R, c.y - R, lo.z)), V((c.x + R, c.y + R, lo.z + 2)), conc())
    cyl(V((c.x, c.y, lo.z + 2)), V((c.x, c.y, lo.z + drum)), R * 0.92, plaster(), 48)
    dome_shell(V((c.x, c.y, lo.z + drum)), R * 0.95, (hi.z - lo.z - drum) / (R * 0.95))
    windows(V((c.x - R * 0.65, c.y - R * 0.65, 0)), V((c.x + R * 0.65, c.y + R * 0.65, 0)), lo.z + 6, lo.z + drum - 4, max(1, int(drum / 14)), paintm((0.8, 0.8, 0.78), "trim"), every=R * 0.4)

def dome_shell(c, R, kz=1.0, slit=True):
    prof = [(R * math.cos(t), R * kz * math.sin(t)) for t in [math.pi / 2 * i / 16 for i in range(17)]]
    prof[-1] = (0.01, R * kz)
    lathe(prof, c, acg("MetalPlates006", 0.08, tint=(0.82, 0.83, 0.85), rough=0.35, metal=0.9, name="domemetal"), 64)
    if slit:   # observing slit: a dark strip over the top
        w = R * 0.12
        pts = []
        for i in range(17):
            t = math.pi / 2 * i / 16; pts.append((R * 1.01 * math.cos(t), R * kz * 1.01 * math.sin(t)))
        prism([(x, z) for x, z in pts] + [(x * 0.97, z * 0.97) for x, z in reversed(pts)], -w, w, paintm((0.03, 0.03, 0.035), "slit"), axis="x").location = (c.x, c.y, c.z)

def b_dome(lo, hi, name):
    c = (lo + hi) / 2; R = min(hi.x - lo.x, hi.y - lo.y) / 2
    dome_shell(V((c.x, c.y, lo.z)), R, (hi.z - lo.z) / R)

def b_pyramid(lo, hi, name):
    c = (lo + hi) / 2; W = min(hi.x - lo.x, hi.y - lo.y); H = hi.z - lo.z; n = max(12, int(H / 3.2))
    m = acg("Rock035", 1 / 10.0, tint=(1.2, 1.02, 0.76), name="sandstone")
    for k in range(n):
        w = W * (1 - k / n); z0 = lo.z + H * k / n
        box(V((c.x - w / 2, c.y - w / 2, z0)), V((c.x + w / 2, c.y + w / 2, z0 + H / n)), m)

def b_ziggurat(lo, hi, name):
    c = (lo + hi) / 2; W, D = hi.x - lo.x, hi.y - lo.y; H = hi.z - lo.z; n = 4
    for k in range(n):
        f = 1 - k * 0.2
        box(V((c.x - W * f / 2, c.y - D * f / 2, lo.z + H * k / n)), V((c.x + W * f / 2, c.y + D * f / 2, lo.z + H * (k + 1) / n)), stone())

def b_ruins(lo, hi, name):
    """Broken stone walls along the long side, ragged tops."""
    rnd = random.Random(name); along = long_axis(lo, hi); H = hi.z - lo.z
    L0, L1 = (lo.x, hi.x) if along == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if along == "x" else (lo.x, hi.x)
    for s in (S0 + 1, S1 - 3, (S0 + S1) / 2 - 1):
        a = L0
        while a < L1 - 2:
            l = rnd.uniform(4, 14); h = H * rnd.uniform(0.25, 1.0)
            if rnd.random() < 0.75:
                p0, p1 = (V((a, s, lo.z)), V((min(a + l, L1), s + 2.2, lo.z + h))) if along == "x" else (V((s, a, lo.z)), V((s + 2.2, min(a + l, L1), lo.z + h)))
                box(p0, p1, stone(), bevel=0.25)
            a += l
    for _ in range(14):   # fallen blocks
        x = rnd.uniform(lo.x, hi.x); y = rnd.uniform(lo.y, hi.y); s = rnd.uniform(1, 2.5)
        o = box(V((x - s, y - s * 0.7, lo.z)), V((x + s, y + s * 0.7, lo.z + s)), stone(), bevel=0.2); o.rotation_euler.z = rnd.uniform(0, 3)

def b_ufo(lo, hi, name):
    c = (lo + hi) / 2; R = min(hi.x - lo.x, hi.y - lo.y) / 2; H = hi.z - lo.z; z = lo.z
    prof = [(0.01, z), (R * 0.35, z + H * 0.05), (R * 0.8, z + H * 0.25), (R, z + H * 0.42), (R * 0.82, z + H * 0.55), (R * 0.45, z + H * 0.62)]
    lathe(prof, V((c.x, c.y, 0)), acg("MetalPlates013", 0.12, tint=(0.75, 0.78, 0.82), rough=0.25, metal=1.0, name="ufometal"), 64)
    dome = [(R * 0.45 * math.cos(t), z + H * 0.62 + H * 0.38 * math.sin(t)) for t in [math.pi / 2 * i / 10 for i in range(11)]]; dome[-1] = (0.01, dome[-1][1])
    lathe(dome, V((c.x, c.y, 0)), flat("ufoglass", (0.1, 0.35, 0.3), 0.08, 0.3), 48)
    for k in range(16):
        a = 2 * math.pi * k / 16
        p = V((c.x + R * 0.93 * math.cos(a), c.y + R * 0.93 * math.sin(a), z + H * 0.42))
        bpy.ops.mesh.primitive_uv_sphere_add(radius=R * 0.04, location=p, segments=12, ring_count=6); bpy.context.object.data.materials.append(emit("ufolight", (0.9, 0.85, 0.4)))

def b_blimp(lo, hi, name):
    c = (lo + hi) / 2; L = hi.y - lo.y; R = (hi.x - lo.x) / 2; H = hi.z - lo.z
    zc = hi.z - R
    prof = []
    for i in range(25):   # envelope as a lathe along Y: build around Z then rotate
        t = i / 24; r = R * math.sin(math.pi * t) ** 0.65 * (1.0 if t < 0.5 else (1 - 0.25 * (t - 0.5)))
        prof.append((max(r, 0.01), -L / 2 + L * t))
    env = lathe(prof, V((0, 0, 0)), flat("blimpskin", (0.78, 0.79, 0.8), 0.45, 0.2), 48)
    env.rotation_euler.x = -math.pi / 2; env.location = (c.x, c.y, zc)
    # fins at the tail
    for a in (0, math.pi / 2, math.pi, 3 * math.pi / 2):
        f = prism([(0, 0), (R * 0.95, -L * 0.06), (R * 0.95, -L * 0.2), (0, -L * 0.28)], -0.4, 0.4, paintm((0.12, 0.2, 0.42), "blimpfin"), axis="x")
        f.location = (c.x, c.y - L * 0.22, zc); f.rotation_euler.y = a
        mw = Matrix.Translation(V((c.x, lo.y + L * 0.28, zc))) @ Matrix.Rotation(a, 4, "Y") @ Matrix.Rotation(math.pi / 2, 4, "Z")
        f.matrix_world = mw
    g = box(V((c.x - R * 0.18, c.y - L * 0.08, lo.z)), V((c.x + R * 0.18, c.y + L * 0.1, lo.z + max(5, zc - R - lo.z + 2))), paintm((0.85, 0.85, 0.83), "gondola"), bevel=0.6)
    box(V((c.x - R * 0.185, c.y - L * 0.06, lo.z + 1.2)), V((c.x + R * 0.185, c.y + L * 0.09, lo.z + 3.0)), glass())
    # side banners
    bm = brand_mat(brand_for(name))
    for sx in (-1, 1):
        x = c.x + sx * R * 1.002
        mesh("ad", [V((x, c.y - L * 0.22, zc - R * 0.3)), V((x, c.y + L * 0.22, zc - R * 0.3)), V((x, c.y + L * 0.22, zc + R * 0.3)), V((x, c.y - L * 0.22, zc + R * 0.3))], [[0, 1, 2, 3]], bm)
        me = bpy.context.scene.objects[-1].data if False else None

def b_missilesilo(lo, hi, name):
    c = (lo + hi) / 2
    box(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, 0.2)), conc())
    R = min(hi.x - lo.x, hi.y - lo.y) * 0.18
    cyl(V((c.x - R * 1.6, c.y, 0)), V((c.x - R * 1.6, c.y, 2.2)), R * 1.15, conc(), 48)
    box(V((c.x - R * 1.6 - R, c.y - R, 2.2)), V((c.x - R * 1.6 + R, c.y + R, min(hi.z, 4.5))), acg("MetalPlates006", 0.1, tint=(0.5, 0.52, 0.5), metal=0.8, name="blastdoor"), bevel=0.3)
    for k in range(3):
        box(V((c.x + R * (0.6 + k * 1.1), c.y - 3, 0)), V((c.x + R * (0.6 + k * 1.1) + 5, c.y + 3, 2.5)), conc())

def b_culvert(lo, hi, name):
    along = "y" if (hi.y - lo.y) >= (hi.x - lo.x) else "x"; H = hi.z - lo.z; r = H * 0.42; c = (lo + hi) / 2
    a, b = (V((c.x, lo.y + 2, lo.z + r + 0.5)), V((c.x, hi.y - 2, lo.z + r + 0.5))) if along == "y" else (V((lo.x + 2, c.y, lo.z + r + 0.5)), V((hi.x - 2, c.y, lo.z + r + 0.5)))
    o = cyl(a, b, r, conc(), 32, cap=False); s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.9
    cyl(a, b, r * 0.92, paintm((0.03, 0.03, 0.03), "dark"), 24)
    for p in (a, b):   # headwalls
        if along == "y": box(V((lo.x, p.y - 1.2, lo.z)), V((hi.x, p.y + 1.2, hi.z)), conc())
        else: box(V((p.x - 1.2, lo.y, lo.z)), V((p.x + 1.2, hi.y, hi.z)), conc())

def b_ramp(lo, hi, name):
    along = long_axis(lo, hi)
    if along == "x":
        prism([(lo.x, lo.z), (hi.x, lo.z), (hi.x, hi.z)], lo.y, hi.y, conc(), axis="y")
    else:
        prism([(lo.y, lo.z), (hi.y, lo.z), (hi.y, hi.z)], lo.x, hi.x, conc(), axis="x")

def b_deck(lo, hi, name):
    """Timber patio deck with posts and rails."""
    zt = hi.z if hi.z - lo.z < 4 else lo.z + 3
    box(V((lo.x, lo.y, zt - 0.4)), V((hi.x, hi.y, zt)), planks())
    for x in (lo.x, hi.x - 0.5):
        for y in (lo.y, hi.y - 0.5):
            box(V((x, y, lo.z)), V((x + 0.5, y + 0.5, zt + 3.4)), wood())
    for (a, b) in ((V((lo.x, lo.y, zt + 3)), V((hi.x, lo.y + 0.4, zt + 3.4))), (V((lo.x, hi.y - 0.4, zt + 3)), V((hi.x, hi.y, zt + 3.4))),
                   (V((lo.x, lo.y, zt + 3)), V((lo.x + 0.4, hi.y, zt + 3.4))), (V((hi.x - 0.4, lo.y, zt + 3)), V((hi.x, hi.y, zt + 3.4)))):
        box(a, b, wood())
    n = int((hi.x - lo.x) / 6)
    for k in range(n + 1):
        x = lo.x + (hi.x - lo.x) * k / max(n, 1)
        for y in (lo.y, hi.y - 0.3): box(V((x - 0.15, y, zt)), V((x + 0.15, y + 0.3, zt + 3)), wood())

# ---------------- bridges and rail ----------------
def _span_axis(lo, hi):
    return "x" if (hi.x - lo.x) >= (hi.y - lo.y) else "y"

def b_girder_bridge(lo, hi, name, deck_frac=0.85, rails=True, mat=None):
    ax = _span_axis(lo, hi); mat = mat or conc(); H = hi.z - lo.z; zd = lo.z + H * deck_frac
    box(V((lo.x, lo.y, zd - 3)), V((hi.x, hi.y, zd)), mat)                     # deck slab
    box(V((lo.x, lo.y, zd)), V((hi.x, hi.y, zd + 0.15)), asphalt())
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x)
    n = max(1, int((L1 - L0) / 60))
    for k in range(n + 1):   # piers
        a = L0 + (L1 - L0) * k / n; a = min(max(a, L0 + 2), L1 - 2)
        for s in (S0 + (S1 - S0) * 0.25, S0 + (S1 - S0) * 0.75):
            p = V((a, s, lo.z)) if ax == "x" else V((s, a, lo.z)); cyl(p, p + V((0, 0, zd - 3 - lo.z)), min(2.5, (S1 - S0) * 0.08), mat, 20)
        cb = (V((a - 2, S0 + 1, zd - 5)), V((a + 2, S1 - 1, zd - 3))) if ax == "x" else (V((S0 + 1, a - 2, zd - 5)), V((S1 - 1, a + 2, zd - 3)))
        box(*cb, mat)
    if rails:
        for s in (S0, S1 - 0.8):
            r = (V((L0, s, zd)), V((L1, s + 0.8, zd + 3.2))) if ax == "x" else (V((s, L0, zd)), V((s + 0.8, L1, zd + 3.2)))
            box(*r, mat)

def b_truss_bridge(lo, hi, name, timber=False):
    """Through-truss footbridge: deck, two side trusses, end piers."""
    ax = _span_axis(lo, hi); H = hi.z - lo.z; m = wood() if timber else paintm((0.2, 0.32, 0.25), "bridgegreen", 0.5, 0.5)
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x)
    sc = (S0 + S1) / 2; w = min(S1 - S0, 10); zd = lo.z + H * 0.55; th = min(9.0, hi.z - zd)
    P = (lambda a, s, z: V((a, s, z))) if ax == "x" else (lambda a, s, z: V((s, a, z)))
    d0, d1 = P(L0, sc - w / 2, zd - 0.8), P(L1, sc + w / 2, zd)
    box(V(map(min, d0, d1)), V(map(max, d0, d1)), planks() if timber else conc())
    n = max(4, int((L1 - L0) / 10))
    for s in (sc - w / 2, sc + w / 2):
        cyl(P(L0, s, zd), P(L1, s, zd), 0.35, m, 6); cyl(P(L0, s, zd + th), P(L1, s, zd + th), 0.35, m, 6)
        for k in range(n + 1):
            a = L0 + (L1 - L0) * k / n; cyl(P(a, s, zd), P(a, s, zd + th), 0.25, m, 6)
            if k < n: cyl(P(a, s, zd), P(L0 + (L1 - L0) * (k + 1) / n, s, zd + th), 0.2, m, 4)
    for a in (L0 + 2, L1 - 2):
        for s in (sc - w / 2, sc + w / 2): cyl(P(a, s, lo.z), P(a, s, zd - 0.8), 1.0, conc() if not timber else wood(), 12)

def b_trestle(lo, hi, name):
    """Timber rail trestle: deck with ties and rails on braced bents."""
    ax = _span_axis(lo, hi); P = (lambda a, s, z: V((a, s, z))) if ax == "x" else (lambda a, s, z: V((s, a, z)))
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x)
    sc = (S0 + S1) / 2; zt = hi.z - 1.5; tw = 7.0
    for s in (sc - 2.4, sc + 2.4): beam(P(L0, s, zt - 1), P(L1, s, zt - 1), 1.2, oldwood())
    n = int((L1 - L0) / 2.2)
    for k in range(n): a = L0 + (k + 0.5) * (L1 - L0) / n; box(V(map(min, P(a - 0.4, sc - 4.2, zt - 0.4), P(a + 0.4, sc + 4.2, zt))), V(map(max, P(a - 0.4, sc - 4.2, zt - 0.4), P(a + 0.4, sc + 4.2, zt))), oldwood())
    for s in (sc - 2.35, sc + 2.35): beam(P(L0, s, zt), P(L1, s, zt), 0.3, steel(), 0.45)
    nb = max(1, int((L1 - L0) / 15))
    for k in range(nb + 1):
        a = L0 + (L1 - L0) * k / nb; a = min(max(a, L0 + 1), L1 - 1)
        foot = max(lo.z, zt - 1 - 120); spread = (zt - foot) * 0.18 + tw / 2
        for s in (-1, -0.33, 0.33, 1):
            beam(P(a, sc + s * spread, foot), P(a, sc + s * tw / 2, zt - 1.6), 0.9, oldwood())
        for z in range(int(foot) + 8, int(zt - 2), 12):
            f = (z - foot) / (zt - foot); sp = spread + (tw / 2 - spread) * f
            beam(P(a, sc - sp, z), P(a, sc + sp, z), 0.6, oldwood())

def b_debris(lo, hi, name):
    rnd = random.Random(name); c = (lo + hi) / 2; W, D, H = hi - lo
    for k in range(3):
        o = box(V((-W * 0.3, -D * 0.22, -1.5)), V((W * 0.3, D * 0.22, 1.5)), conc())
        o.location = (c.x + rnd.uniform(-0.2, 0.2) * W, c.y + (k - 1) * D * 0.3, lo.z + H * rnd.uniform(0.2, 0.5))
        o.rotation_euler = (rnd.uniform(-0.5, 0.5), rnd.uniform(-0.6, 0.6), rnd.uniform(-0.3, 0.3))
        for j in range(5):   # bent rebar sticking out
            p = V((o.location)) + V((rnd.uniform(-W * 0.25, W * 0.25), rnd.uniform(-D * 0.1, D * 0.1), 1))
            cyl(p, p + V((rnd.uniform(-2, 2), rnd.uniform(-2, 2), rnd.uniform(2, 5))), 0.08, rust(), 4)

def rails_on(lo, hi, ax, z, curve=0.0):
    """Ballast, sleepers and two rails along an axis at height z."""
    P = (lambda a, s, zz: V((a, s, zz))) if ax == "x" else (lambda a, s, zz: V((s, a, zz)))
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x); sc = (S0 + S1) / 2
    n = int((L1 - L0) / 2.0)
    def off(a): t = (a - L0) / max(L1 - L0, 1); return curve * (t * t)
    gr = acg("Gravel022", 1 / 6.0, name="ballast") if False else T("Rock030", 3, tint=(0.8, 0.78, 0.74), name="ballast")
    for k in range(n):
        a = L0 + (k + 0.5) * (L1 - L0) / n; o = off(a)
        q0, q1 = P(a - 0.4, sc + o - 4.3, z), P(a + 0.4, sc + o + 4.3, z + 0.6)
        box(V(map(min, q0, q1)), V(map(max, q0, q1)), oldwood())
    for s in (-2.35, 2.35):
        pts = [P(L0 + (L1 - L0) * i / 12, sc + off(L0 + (L1 - L0) * i / 12) + s, z + 0.6) for i in range(13)]
        for p, q in zip(pts, pts[1:]): beam(p, q, 0.25, steel(), 0.5)
    return gr

def b_track(lo, hi, name):
    ax = long_axis(lo, hi); H = hi.z - lo.z
    gr = T("Rock030", 3, tint=(0.8, 0.78, 0.74), name="ballast")
    S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x); sc = (S0 + S1) / 2
    zt = lo.z + min(H, 6) - 1.1
    curve = (S1 - S0) * 0.25 if "radius" in name else 0.0
    if ax == "x": prism([(sc - 8, lo.z), (sc + 8, lo.z), (sc + 5.5, zt), (sc - 5.5, zt)], lo.x, hi.x, gr, axis="x")
    else: prism([(sc - 8, lo.z), (sc + 8, lo.z), (sc + 5.5, zt), (sc - 5.5, zt)], lo.y, hi.y, gr, axis="y")
    rails_on(lo, hi, ax, zt, curve)
    if "end" in name:   # buffer stop
        a = (hi.x - 2) if ax == "x" else (hi.y - 2)
        p0, p1 = (V((a, sc - 4, zt)), V((a + 1.5, sc + 4, zt + 4))) if ax == "x" else (V((sc - 4, a, zt)), V((sc + 4, a + 1.5, zt + 4)))
        box(p0, p1, paintm((0.75, 0.6, 0.1), "buffer"))

def b_crossing(lo, hi, name):
    ax = long_axis(lo, hi); z = lo.z
    box(V((lo.x, lo.y, z)), V((hi.x, hi.y, z + 0.3)), asphalt())
    rails_on(lo, hi, "y" if ax == "x" else "x", z + 0.3)
    c = (lo + hi) / 2
    box(V((c.x - 3.3, lo.y, z + 0.3)), V((c.x + 3.3, hi.y, z + 0.95)), oldwood()) if ax == "x" else box(V((lo.x, c.y - 3.3, z + 0.3)), V((hi.x, c.y + 3.3, z + 0.95)), oldwood())

def b_rail_support(lo, hi, name):
    c = (lo + hi) / 2
    for f in (0.25, 0.75):
        x = lo.x + (hi.x - lo.x) * f; beam(V((x, c.y, lo.z)), V((x, c.y, hi.z - 1)), 1.6, conc())
    box(V((lo.x, lo.y, hi.z - 2)), V((hi.x, hi.y, hi.z)), conc())

def b_elev_track(lo, hi, name):
    ax = long_axis(lo, hi); H = hi.z - lo.z
    b_rail_support(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, hi.z - 1.5)), name) if False else None
    zt = hi.z - 1.2
    S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x); sc = (S0 + S1) / 2
    d0, d1 = (V((lo.x, sc - 6, zt - 2)), V((hi.x, sc + 6, zt))) if ax == "x" else (V((sc - 6, lo.y, zt - 2)), V((sc + 6, hi.y, zt)))
    box(d0, d1, conc())
    rails_on(lo, hi, ax, zt, (S1 - S0) * 0.3 if "radius" in name else 0.0)
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y)
    for a in (L0 + 3, L1 - 3):
        p = V((a, sc, lo.z)) if ax == "x" else V((sc, a, lo.z)); beam(p, p + V((0, 0, zt - 2 - lo.z)), 2.5, conc())

def b_skijump(lo, hi, name):
    """Inrun on steel legs from the high end down to the take-off, then the landing hill."""
    c = (lo + hi) / 2; L = hi.y - lo.y; H = hi.z - lo.z; w = min(hi.x - lo.x, 12)
    top = V((c.x, hi.y - 2, hi.z)); take = V((c.x, lo.y + L * 0.45, lo.z + H * 0.48))
    d = take - top; n = 12
    for i in range(n):
        a = top + d * (i / n); b = top + d * ((i + 1) / n)
        bend = 1 - (i + 1) / n; b2 = b.copy(); b2.z -= 0 * bend
        o = beam(a, b, w, conc(), 1.2); o.location = (a.x, a.y, a.z)
        for sx in (-w / 2, w / 2):
            cyl(V((a.x + sx, a.y, lo.z)), V((a.x + sx, a.y, a.z - 0.6)), 0.5, steel(), 8)
    for i in range(0, n, 2):
        a = top + d * (i / n)
        cyl(V((a.x - w / 2, a.y, lo.z)), V((a.x + w / 2, a.y, a.z - 0.6)), 0.3, steel(), 6)
    prism([(take.y - 4, lo.z), (take.y, take.z - 4), (lo.y, lo.z)], c.x - (hi.x - lo.x) / 2, c.x + (hi.x - lo.x) / 2, acg("Grass004", 1 / 8.0, name="hillgrass"), axis="x")
    box(V((c.x - 3, hi.y - 8, hi.z)), V((c.x + 3, hi.y, hi.z + 8)), sidingw())

# ---------------- towers, poles, wires ----------------
def b_lattice_tower(lo, hi, name):
    c = (lo + hi) / 2; W = min(hi.x - lo.x, hi.y - lo.y); m = galv()
    lattice(V((c.x, c.y, 0)), max(lo.z, 0), hi.z - 2, W * 0.95, W * 0.25, m, panels=max(4, int((hi.z - lo.z) / 10)), leg=max(0.3, W * 0.03), brace=max(0.15, W * 0.012))
    cyl(V((c.x, c.y, hi.z - 2)), V((c.x, c.y, hi.z)), 0.3, m, 6)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(c.x, c.y, hi.z)); bpy.context.object.data.materials.append(emit("redlamp", (0.9, 0.1, 0.05)))
    if "cell" in name or "xmit" in name or "antenna" in name:
        for a in range(3):
            ang = a * 2 * math.pi / 3; p = V((c.x + math.cos(ang) * W * 0.3, c.y + math.sin(ang) * W * 0.3, hi.z - 4))
            box(p - V((0.4, 0.4, 2)), p + V((0.4, 0.4, 2)), paintm((0.85, 0.85, 0.83), "panelant"))

def b_mast(lo, hi, name):
    """Simple mast: tapered pole, with arms by type."""
    c = (lo + hi) / 2; z0 = lo.z; z1 = hi.z; W = hi.x - lo.x; D = hi.y - lo.y
    if "telephone" in name:
        cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1)), 0.5, wood(), 12, r2=0.38)
        arm = V((c.x, lo.y, z1 - 2)); box(V((c.x - 0.25, lo.y, z1 - 2.3)), V((c.x + 0.25, hi.y, z1 - 1.8)), wood())
        for f in (0.1, 0.35, 0.65, 0.9):
            y = lo.y + D * f; cyl(V((c.x, y, z1 - 1.8)), V((c.x, y, z1 - 1.0)), 0.18, flat("insulator", (0.25, 0.45, 0.35), 0.15), 8)
        return
    if "lightpole" in name:
        cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1 - 2)), 0.7, galv(), 16, r2=0.4)
        box(V((lo.x, c.y - 0.4, z1 - 3)), V((hi.x, c.y + 0.4, z1 - 2.4)), galv())
        for f in (0.12, 0.37, 0.63, 0.88):
            x = lo.x + W * f; box(V((x - 1.1, c.y - 0.7, z1 - 2.4)), V((x + 1.1, c.y + 0.9, z1)), galv())
            box(V((x - 1.0, c.y + 0.91, z1 - 2.2)), V((x + 1.0, c.y + 0.95, z1 - 0.2)), emit("floodlamp", (0.95, 0.95, 0.88)))
        return
    if "speaker" in name:
        cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1 - 2)), 0.35, galv(), 12)
        for a in (0, 2.1, 4.2):
            d = V((math.cos(a), math.sin(a), 0)); p = V((c.x, c.y, z1 - 1.5))
            cyl(p, p + d * 1.6, 0.25, paintm((0.85, 0.85, 0.82), "horn"), 12, r2=0.75)
        return
    if "signal" in name:
        cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1)), 0.25, galv(), 12)
        for s in (-1, 1):   # crossbuck
            o = box(V((-3.2, -0.05, -0.45)), V((3.2, 0.05, 0.45)), paintm((0.9, 0.9, 0.88), "xbuck")); o.location = (c.x, c.y - 0.3, z1 - 1.5); o.rotation_euler = (0, s * 0.7, 0)
        box(V((c.x - 2.2, c.y - 0.4, z1 - 5.5)), V((c.x + 2.2, c.y, z1 - 4.5)), paintm((0.04, 0.04, 0.04), "dark"))
        for sx in (-1.4, 1.4):
            cyl(V((c.x + sx, c.y - 0.45, z1 - 5)), V((c.x + sx, c.y - 0.6, z1 - 5)), 0.55, emit("redlamp", (0.9, 0.1, 0.05)), 16)
        return
    if "antenna" in name:
        cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1)), 0.15, galv(), 8)
        for k in range(5):
            z = z1 - 1 - k * 1.6; w = 2.2 - k * 0.25; cyl(V((c.x - w, c.y, z)), V((c.x + w, c.y, z)), 0.06, galv(), 4)
        return
    cyl(V((c.x, c.y, z0)), V((c.x, c.y, z1)), max(0.2, min(W, D) * 0.15), galv(), 12)

def b_skiliftpost(lo, hi, name):
    c = (lo + hi) / 2
    cyl(V((c.x, c.y, lo.z)), V((c.x, c.y, hi.z - 3)), 1.3, paintm((0.2, 0.22, 0.25), "liftgrey", 0.5, 0.6), 16, r2=0.9)
    box(V((lo.x, c.y - 0.6, hi.z - 3)), V((hi.x, c.y + 0.6, hi.z - 1.8)), steel())
    for x in (lo.x + 1.5, hi.x - 1.5):
        box(V((x - 1, lo.y, hi.z - 1.8)), V((x + 1, hi.y, hi.z - 1.0)), steel())
        for f in (0.2, 0.4, 0.6, 0.8):
            y = lo.y + (hi.y - lo.y) * f; cyl(V((x - 0.6, y, hi.z - 1.0)), V((x + 0.6, y, hi.z - 1.0)), 0.5, rubber(), 12)

def b_wire(lo, hi, name):
    """Cables: along the long horizontal axis with sag, or a straight run when vertical."""
    m = flat("cable", (0.05, 0.05, 0.05), 0.5, 0.6)
    W, D, H = hi - lo
    if H > max(W, D):   # lift cable: one straight run corner to corner
        cyl(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, hi.z)), 0.12, m, 6, cap=False); return
    ax = "y" if D >= W else "x"; sag = max(0.5, H * 0.8); top = hi.z
    n = 2 if W < 2 and ax == "y" else 3
    for k in range(n):
        s = (lo.x + (hi.x - lo.x) * (k + 0.5) / n) if ax == "y" else (lo.y + (hi.y - lo.y) * (k + 0.5) / n)
        a, b = (V((s, lo.y, top)), V((s, hi.y, top))) if ax == "y" else (V((lo.x, s, top)), V((hi.x, s, top)))
        catenary(a, b, sag, 0.09, m)

def b_headframe(lo, hi, name):
    """Mine headframe: steel A-frame with sheave wheels, hoist house beside."""
    c = (lo + hi) / 2; H = hi.z - lo.z; m = paintm((0.42, 0.14, 0.08), "minered", 0.6, 0.5)
    t = V((c.x, c.y - (hi.y - lo.y) * 0.15, hi.z - 6))
    for sx in (-1, 1):
        for sy in (-1, 1): cyl(V((c.x + sx * 10, c.y - (hi.y - lo.y) * 0.15 + sy * 8, lo.z)), t + V((sx * 4, sy * 3, 0)), 0.8, m, 8)
        cyl(V((c.x + sx * 6, c.y + (hi.y - lo.y) * 0.3, lo.z)), t + V((sx * 4, 0, 0)), 0.9, m, 8)
    box(t - V((6, 5, 0.5)), t + V((6, 5, 0.5)), m)
    for sx in (-2.5, 2.5):
        bpy.ops.mesh.primitive_torus_add(major_radius=5, minor_radius=0.35, location=t + V((sx, 0, 5)), rotation=(0, math.pi / 2, 0)); bpy.context.object.data.materials.append(steel())
    house(V((lo.x, c.y + (hi.y - lo.y) * 0.2, lo.z)), V((lo.x + (hi.x - lo.x) * 0.45, hi.y, lo.z + H * 0.3)), bricks(), roofing())

# ---------------- yard props ----------------
def b_container(lo, hi, name):
    rgb = paint_for(name) if callable(paint_for) else (0.55, 0.12, 0.08)
    col = random.Random(name).choice([(0.55, 0.12, 0.08), (0.12, 0.28, 0.45), (0.2, 0.36, 0.2), (0.6, 0.45, 0.1), (0.4, 0.4, 0.42)])
    box(lo, hi, corr(col))
    ax = long_axis(lo, hi)
    for a in ((lo.x, hi.x) if ax == "x" else (lo.y, hi.y)):
        for s in ((lo.y, hi.y) if ax == "x" else (lo.x, hi.x)):
            p = V((a, s, 0)) if ax == "x" else V((s, a, 0))
            box(V((p.x - 0.4, p.y - 0.4, lo.z)), V((p.x + 0.4, p.y + 0.4, hi.z)), paintm(tuple(x * 0.7 for x in col), f"cont{col}"))

def b_tank(lo, hi, name):
    c = (lo + hi) / 2; W, D, H = hi - lo
    if H < 0.6 * min(W, D) and max(W, D) > 2.2 * min(W, D):   # horizontal capsule (propane)
        ax = long_axis(lo, hi); r = H / 2 * 0.9; L = max(W, D)
        a, b = (V((lo.x + r, c.y, lo.z + r)), V((hi.x - r, c.y, lo.z + r))) if ax == "x" else (V((c.x, lo.y + r, lo.z + r)), V((c.x, hi.y - r, lo.z + r)))
        cyl(a, b, r, paintm((0.88, 0.88, 0.86), "tankwhite", 0.35, 0.2), 24, cap=False)
        for p in (a, b):
            bpy.ops.mesh.primitive_uv_sphere_add(radius=r, location=p, segments=24, ring_count=12); bpy.context.object.data.materials.append(paintm((0.88, 0.88, 0.86), "tankwhite", 0.35, 0.2))
        return
    R = min(W, D) / 2
    cyl(V((c.x, c.y, lo.z)), V((c.x, c.y, hi.z - R * 0.08)), R, corr((0.6, 0.6, 0.58)), 48)
    cyl(V((c.x, c.y, hi.z - R * 0.08)), V((c.x, c.y, hi.z)), R, corr((0.6, 0.6, 0.58)), 48, r2=R * 0.1)
    for k in range(int(H / 1.2)): box(V((c.x + R, c.y - 0.8, lo.z + k * 1.2)), V((c.x + R + 0.6, c.y + 0.8, lo.z + k * 1.2 + 0.15)), galv())

def b_finish(lo, hi, name):
    """Finish/checkpoint arch: two towers and a banner with a checker band."""
    from PIL import Image, ImageDraw, ImageFont
    import os
    from common import BUILD, image_mat
    c = (lo + hi) / 2; ax = long_axis(lo, hi); W = max(hi.x - lo.x, hi.y - lo.y); H = hi.z - max(lo.z, 0)
    txt = "FINISH" if "finish" in name else "CHECKPOINT"
    p = os.path.join(BUILD, "tex", f"arch_{txt.lower()}.png")
    if not os.path.exists(p):
        im = Image.new("RGB", (1024, 256), (20, 20, 22)); d = ImageDraw.Draw(im)
        for i in range(32):
            for j in range(2): d.rectangle([i * 32, j * 32 + 192, i * 32 + 31, j * 32 + 223], fill=(240, 240, 236) if (i + j) % 2 else (16, 16, 16))
            for j in range(2): d.rectangle([i * 32, j * 32, i * 32 + 31, j * 32 + 31], fill=(240, 240, 236) if (i + j) % 2 else (16, 16, 16))
        f = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 110)
        w = d.textlength(txt, font=f); d.text(((1024 - w) / 2, 62), txt, font=f, fill=(240, 240, 236)); im.save(p)
    bm = image_mat(p, f"arch_{txt}", 0.6)
    tw = 2.5; z0 = max(lo.z, 0)
    for s in (-1, 1):
        q = (V((c.x + s * (W / 2 - tw / 2), c.y, 0))) if ax == "x" else V((c.x, c.y + s * (W / 2 - tw / 2), 0))
        box(V((q.x - tw / 2, q.y - tw / 2, z0)), V((q.x + tw / 2, q.y + tw / 2, hi.z)), paintm((0.1, 0.1, 0.11), "archpost", 0.5, 0.4))
    bh = min(8.0, H * 0.3)
    if ax == "x":
        o = box(V((lo.x + tw, c.y - 0.6, hi.z - bh)), V((hi.x - tw, c.y + 0.6, hi.z)), paintm((0.1, 0.1, 0.11), "archpost", 0.5, 0.4))
        for y in (c.y - 0.62, c.y + 0.62):
            mesh("banner", [V((lo.x + tw, y, hi.z - bh)), V((hi.x - tw, y, hi.z - bh)), V((hi.x - tw, y, hi.z)), V((lo.x + tw, y, hi.z))], [[0, 1, 2, 3]], bm)
    else:
        box(V((c.x - 0.6, lo.y + tw, hi.z - bh)), V((c.x + 0.6, hi.y - tw, hi.z)), paintm((0.1, 0.1, 0.11), "archpost", 0.5, 0.4))
        for x in (c.x - 0.62, c.x + 0.62):
            mesh("banner", [V((x, lo.y + tw, hi.z - bh)), V((x, hi.y - tw, hi.z - bh)), V((x, hi.y - tw, hi.z)), V((x, lo.y + tw, hi.z))], [[0, 1, 2, 3]], bm)
    for o in [o for o in bpy.context.scene.objects if o.name.startswith("banner")]:   # stretch the picture over the banner
        me = o.data; uv = me.uv_layers[0]
        for poly in me.polygons:
            for li, k in zip(poly.loop_indices, range(4)): uv.data[li].uv = [(0, 0), (1, 0), (1, 1), (0, 1)][k]

def b_goalpost(lo, hi, name):
    c = (lo + hi) / 2; m = paintm((0.9, 0.78, 0.1), "goalyellow", 0.4, 0.3); z0 = lo.z
    cyl(V((c.x, c.y, z0)), V((c.x, c.y, z0 + 10)), 0.5, m, 12)
    cyl(V((c.x, c.y, z0 + 10)), V((lo.x + 0.5, c.y, z0 + 10)), 0.35, m, 12); cyl(V((c.x, c.y, z0 + 10)), V((hi.x - 0.5, c.y, z0 + 10)), 0.35, m, 12)
    for x in (lo.x + 0.5, hi.x - 0.5): cyl(V((x, c.y, z0 + 10)), V((x, c.y, hi.z)), 0.3, m, 12)

def b_pumpjack(lo, hi, name):
    c = (lo + hi) / 2; m = paintm((0.55, 0.42, 0.08), "jackpaint", 0.55, 0.4); ax = long_axis(lo, hi); H = hi.z - lo.z
    P = (lambda a, s, z: V((a, s, z))) if ax == "x" else (lambda a, s, z: V((s, a, z)))
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); sc = (lo.y + hi.y) / 2 if ax == "x" else (lo.x + hi.x) / 2
    L = L1 - L0; piv = P(L0 + L * 0.45, sc, lo.z + H * 0.62)
    box(V(map(min, P(L0, sc - 3, lo.z), P(L1, sc + 3, lo.z + 1))), V(map(max, P(L0, sc - 3, lo.z), P(L1, sc + 3, lo.z + 1))), conc())
    for s in (-2, 2): cyl(P(L0 + L * 0.3, sc + s, lo.z + 1), piv, 0.5, m, 8); cyl(P(L0 + L * 0.6, sc + s, lo.z + 1), piv, 0.5, m, 8)
    beam(P(L0 + L * 0.08, sc, lo.z + H * 0.68), P(L0 + L * 0.85, sc, lo.z + H * 0.58), 1.4, m)
    head = prism([(0, -3), (2.5, -1), (2.5, 4), (0, 5)], -0.8, 0.8, m, axis="y" if ax == "x" else "x")
    head.location = P(L0 + L * 0.02, sc, lo.z + H * 0.68)
    cyl(P(L0 + L * 0.85, sc - 1.6, lo.z + H * 0.35), P(L0 + L * 0.85, sc + 1.6, lo.z + H * 0.35), H * 0.2, m, 24)
    box(V(map(min, P(L0 + L * 0.75, sc - 3, lo.z + 1), P(L1, sc + 3, lo.z + 4))), V(map(max, P(L0 + L * 0.75, sc - 3, lo.z + 1), P(L1, sc + 3, lo.z + 4))), m)
    cyl(P(L0 + L * 0.05, sc, lo.z), P(L0 + L * 0.05, sc, lo.z + H * 0.35), 0.25, steel(), 8)

def b_windpump(lo, hi, name):
    c = (lo + hi) / 2; W = min(hi.x - lo.x, hi.y - lo.y); top = hi.z - W * 0.5
    lattice(V((c.x, c.y, 0)), max(lo.z, 0), top, W * 0.7, W * 0.12, galv(), panels=6, leg=0.25, brace=0.08)
    hub = V((c.x, c.y - 0.8, top + 0.6))
    for k in range(18):
        a = 2 * math.pi * k / 18; d = V((math.cos(a), 0, math.sin(a)))
        o = box(V((-0.4, -0.05, 0)), V((0.4, 0.05, W * 0.45)), galv()); o.location = hub + d * 0.6; o.rotation_euler = (0, -a + math.pi / 2, 0); o.rotation_euler.y = math.atan2(d.x, d.z)
    cyl(hub - V((0, 0.1, 0)), hub + V((0, 2.5, 0)), 0.3, galv(), 8)
    prism([(0, 0), (0, 2.5), (-W * 0.3, 1.8), (-W * 0.3, 0.6)], -0.03, 0.03, galv(), axis="x").location = (c.x, c.y + 2.5, top)

def b_hut(lo, hi, name):
    """Small huts by kind: outhouse, porta-potty, doghouse, pump shack, sheds, blinds."""
    if "toilet" in name:
        box(lo, V((hi.x, hi.y, hi.z - 0.6)), plastic((0.2, 0.38, 0.6), "portablue")); box(V((lo.x - 0.1, lo.y - 0.1, hi.z - 0.6)), V((hi.x + 0.1, hi.y + 0.1, hi.z)), plastic((0.85, 0.85, 0.83), "portawhite")); return
    wall = oldwood() if re.search(r"outhouse|duck|still|doghouse", name) else (sidingw() if "pump" in name else corr((0.5, 0.5, 0.48)))
    house(lo, hi, wall, roofing(), wall_frac=0.72, rows=1, door=("doghouse" not in name))

def b_carport(lo, hi, name):
    for x in (lo.x + 0.3, hi.x - 0.3):
        for y in (lo.y + 0.3, hi.y - 0.3): cyl(V((x, y, lo.z)), V((x, y, hi.z - 0.5)), 0.25, galv(), 8)
    box(V((lo.x, lo.y, hi.z - 0.5)), V((hi.x, hi.y, hi.z)), corr((0.6, 0.6, 0.6)))

def b_awning(lo, hi, name):
    col = (0.6, 0.12, 0.1) if name.endswith("1") else (0.15, 0.3, 0.5)
    for x in (lo.x + 0.2, hi.x - 0.2): cyl(V((x, lo.y + 0.2, lo.z)), V((x, lo.y + 0.2, hi.z - 2)), 0.12, galv(), 8)
    prism([(lo.y, hi.z - 2), (hi.y, hi.z), (hi.y, hi.z - 0.1), (lo.y, hi.z - 2.1)], lo.x, hi.x, acg("Fabric030", 0.3, ramp=(tuple(c * 0.5 for c in col), col), name=f"awn{col}"), axis="y")

def b_table(lo, hi, name):
    W, D, H = hi - lo; top = hi.z
    box(V((lo.x, lo.y, top - 0.25)), V((hi.x, hi.y, top)), wood())
    for x in (lo.x + 0.4, hi.x - 0.8):
        for y in (lo.y + 0.4, hi.y - 0.8): box(V((x, y, lo.z)), V((x + 0.4, y + 0.4, top - 0.25)), wood())
    if "picnic" in name:
        for y in (lo.y - 1.2, hi.y + 0.2): box(V((lo.x, y, lo.z + H * 0.55)), V((hi.x, y + 1.0, lo.z + H * 0.6)), wood())

def b_crate_pile(lo, hi, name):
    rnd = random.Random(name); m = plywood() if "boxes" in name or "green" in name or "cabinet" in name or "toolbox" in name else acg("Fabric030", 0.3, ramp=((0.33, 0.27, 0.18), (0.62, 0.52, 0.36)), name="burlap")
    if "green" in name: m = plastic((0.2, 0.38, 0.22), "greenbox")
    if "toolbox" in name: m = paintm((0.6, 0.08, 0.06), "toolred", 0.4, 0.4)
    if "cabinet" in name: m = paintm((0.5, 0.52, 0.55), "cabgrey", 0.4, 0.5)
    W, D, H = hi - lo; n = 1 if max(W, D) < 4 else 3
    for k in range(n):
        x0 = lo.x + W * k / n; box(V((x0 + 0.05, lo.y, lo.z)), V((x0 + W / n - 0.05, hi.y, hi.z - rnd.uniform(0, H * 0.3))), m, bevel=0.06 if "sacks" not in name else 0.25)

def b_machine(lo, hi, name):
    col = (0.65, 0.08, 0.07) if "soda" in name else (0.18, 0.18, 0.2)
    box(lo, hi, paintm(col, f"vend{col}", 0.35, 0.3), bevel=0.08)
    box(V((lo.x + (hi.x - lo.x) * 0.1, lo.y - 0.02, lo.z + (hi.z - lo.z) * 0.35)), V((lo.x + (hi.x - lo.x) * 0.65, lo.y + 0.05, hi.z - (hi.z - lo.z) * 0.1)), emit("vendlight", (0.85, 0.85, 0.8)))
    if "snow" in name: box(V((lo.x, lo.y, hi.z - 0.3)), V((hi.x, hi.y, hi.z)), flat("snow", (0.92, 0.93, 0.95), 0.6))

def b_fridge(lo, hi, name): box(lo, hi, paintm((0.86, 0.86, 0.84), "fridgewhite", 0.3, 0.1), bevel=0.12)

def b_chair(lo, hi, name):
    W, D, H = hi - lo; m = galv(); s = acg("Fabric030", 0.3, ramp=((0.1, 0.2, 0.35), (0.2, 0.4, 0.65)), name="chairfab")
    box(V((lo.x, lo.y, lo.z + H * 0.35)), V((hi.x, hi.y - D * 0.3, lo.z + H * 0.4)), s)
    o = box(V((lo.x, hi.y - D * 0.3, lo.z + H * 0.35)), V((hi.x, hi.y - D * 0.25, hi.z)), s)
    for x in (lo.x, hi.x): cyl(V((x, lo.y, lo.z)), V((x, hi.y - D * 0.3, lo.z + H * 0.38)), 0.06, m, 6)

def b_pallet(lo, hi, name):
    H = max(hi.z - lo.z, 0.45); top = lo.z + H
    for k in range(7): x = lo.x + (hi.x - lo.x) * k / 6.5; box(V((x, lo.y, top - 0.08)), V((x + 0.4, hi.y, top)), wood())
    for y in (lo.y, (lo.y + hi.y) / 2 - 0.2, hi.y - 0.4): box(V((lo.x, y, lo.z)), V((hi.x, y + 0.4, top - 0.08)), wood())

def b_steps(lo, hi, name):
    n = 4; ax = long_axis(lo, hi)
    for k in range(n):
        z = lo.z + (hi.z - lo.z) * (k + 1) / n
        if ax == "y": box(V((lo.x, lo.y + (hi.y - lo.y) * k / n, lo.z)), V((hi.x, hi.y, z)), conc())
        else: box(V((lo.x + (hi.x - lo.x) * k / n, lo.y, lo.z)), V((hi.x, hi.y, z)), conc())

def b_jersey(lo, hi, name):
    """Road block: a row of concrete jersey barriers with an orange board on top."""
    ax = long_axis(lo, hi); L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); sc = ((lo.y + hi.y) / 2) if ax == "x" else ((lo.x + hi.x) / 2)
    n = max(1, int((L1 - L0) / 10)); h = min(3.0, hi.z - lo.z)
    for k in range(n):
        a0 = L0 + (L1 - L0) * k / n + 0.1; a1 = L0 + (L1 - L0) * (k + 1) / n - 0.1
        prism([(sc - 1.3, lo.z), (sc + 1.3, lo.z), (sc + 0.4, lo.z + h * 0.35), (sc + 0.3, lo.z + h), (sc - 0.3, lo.z + h), (sc - 0.4, lo.z + h * 0.35)], a0, a1, conc(), axis="x" if ax == "x" else "y")
    if hi.z - lo.z > 5:
        for k in range(n + 1):
            a = L0 + (L1 - L0) * k / n; p = V((a, sc, lo.z + h)) if ax == "x" else V((sc, a, lo.z + h)); cyl(p, p + V((0, 0, hi.z - lo.z - h - 1)), 0.15, galv(), 6)
        b0 = V((L0, sc - 0.1, hi.z - 1.6)) if ax == "x" else V((sc - 0.1, L0, hi.z - 1.6)); b1 = V((L1, sc + 0.1, hi.z)) if ax == "x" else V((sc + 0.1, L1, hi.z))
        box(b0, b1, paintm((0.9, 0.4, 0.05), "roadorange", 0.5))

def b_cattleguard(lo, hi, name):
    box(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, lo.z + 0.6)), paintm((0.03, 0.03, 0.03), "dark"))
    ax = long_axis(lo, hi)
    for y in (lo.y, hi.y - 1.2): box(V((lo.x, y, lo.z)), V((hi.x, y + 1.2, hi.z)), conc()) if ax == "x" else box(V((y if False else lo.x, lo.y, lo.z)), V((lo.x + 1.2, hi.y, hi.z)), conc())
    n = int((hi.y - lo.y) / 0.6) if ax == "x" else int((hi.x - lo.x) / 0.6)
    for k in range(n):
        if ax == "x": y = lo.y + 1.2 + (hi.y - lo.y - 2.4) * k / n; cyl(V((lo.x, y, lo.z + 0.5)), V((hi.x, y, lo.z + 0.5)), 0.2, rust(), 6)
        else: x = lo.x + (hi.x - lo.x) * k / n; cyl(V((x, lo.y, lo.z + 0.5)), V((x, hi.y, lo.z + 0.5)), 0.2, rust(), 6)

def b_gate(lo, hi, name):
    m = galv(); W = hi.x - lo.x; H = hi.z - lo.z; y = (lo.y + hi.y) / 2
    for z in (lo.z + 0.3, lo.z + H * 0.5, hi.z - 0.1): cyl(V((lo.x, y, z)), V((hi.x, y, z)), 0.1, m, 6)
    for f in (0.0, 0.33, 0.66, 1.0): x = lo.x + W * f; cyl(V((x, y, lo.z + 0.3)), V((x, y, hi.z - 0.1)), 0.08, m, 6)
    cyl(V((lo.x, y, lo.z + 0.3)), V((hi.x, y, hi.z - 0.1)), 0.07, m, 6)

def b_metalarch(lo, hi, name):
    c = (lo + hi) / 2; m = paintm((0.2, 0.2, 0.22), "archmetal", 0.4, 0.7); W = hi.x - lo.x; H = hi.z - max(lo.z, 0)
    pts = [V((lo.x + W * (0.5 - 0.5 * math.cos(math.pi * t)), c.y, max(lo.z, 0) + H * math.sin(math.pi * t))) for t in [i / 16 for i in range(17)]]
    for p, q in zip(pts, pts[1:]): cyl(p, q, 0.35, m, 8)

def b_podium_truck(lo, hi, name):
    box(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, lo.z + 3.6)), corr((0.18, 0.2, 0.24)))
    box(V((lo.x, lo.y, lo.z + 3.6)), V((hi.x, hi.y, lo.z + 4)), plates())
    for f, h in ((0.5, 3.0), (0.3, 2.0), (0.7, 1.2)):
        x = lo.x + (hi.x - lo.x) * f; box(V((x - 3, lo.y + 4, lo.z + 4)), V((x + 3, lo.y + 10, lo.z + 4 + h)), paintm((0.85, 0.85, 0.83), "podwhite"))
    for x in (lo.x + 4, hi.x - 4):
        for y in (lo.y + 2, hi.y - 2): cyl(V((x - 0.8, y, lo.z + 1.6)), V((x + 0.8, y, lo.z + 1.6)), 1.6, tyre_mat(), 20)

def b_still(lo, hi, name):
    c = (lo + hi) / 2; cu = flat("copper", (0.72, 0.42, 0.25), 0.35, 1.0); R = min(hi.x - lo.x, hi.y - lo.y) * 0.22
    lathe([(0.01, lo.z), (R, lo.z + 0.3), (R * 1.1, lo.z + R * 1.4), (R * 0.4, lo.z + R * 2.3), (R * 0.15, lo.z + R * 2.6)], V((c.x - R, c.y, 0)), cu, 32)
    catenary(V((c.x - R, c.y, lo.z + R * 2.6)), V((c.x + R * 1.3, c.y, lo.z + R * 1.6)), -0.6, 0.08, cu)
    cyl(V((c.x + R * 1.6, c.y, lo.z)), V((c.x + R * 1.6, c.y, lo.z + R * 1.8)), R * 0.6, oldwood(), 16)
    for k in range(6): cyl(V((c.x - R + 0.4 * k - 1, c.y - R, lo.z)), V((c.x - R + 0.4 * k - 0.8, c.y + R, lo.z + 0.3)), 0.12, wood(), 5)

def b_scarecrow(lo, hi, name):
    c = (lo + hi) / 2
    cyl(V((c.x, c.y, lo.z)), V((c.x, c.y, hi.z - 1.2)), 0.12, wood(), 6); cyl(V((lo.x, c.y, hi.z - 3)), V((hi.x, c.y, hi.z - 3)), 0.1, wood(), 6)
    box(V((c.x - 0.8, c.y - 0.4, hi.z - 5)), V((c.x + 0.8, c.y + 0.4, hi.z - 2.4)), acg("Fabric030", 0.3, ramp=((0.25, 0.18, 0.1), (0.55, 0.4, 0.22)), name="sackcloth"))
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(c.x, c.y, hi.z - 1.6)); bpy.context.object.data.materials.append(acg("Fabric030", 0.3, ramp=((0.45, 0.38, 0.2), (0.75, 0.66, 0.4)), name="burlaphead"))
    cyl(V((c.x, c.y, hi.z - 1.2)), V((c.x, c.y, hi.z - 0.9)), 1.1, acg("ThatchedRoof001A", 0.5, ramp=((0.25, 0.18, 0.06), (0.7, 0.55, 0.25)), name="strawhat"), 16, r2=0.5)

def b_wagon_machine(lo, hi, name):
    """Farm and site machines: tractor, old tractor, bobcat, scraper, combine."""
    c = (lo + hi) / 2; W, D, H = hi - lo; ax = long_axis(lo, hi)
    col = {"tractor": (0.12, 0.35, 0.15), "tractor_old": (0.5, 0.12, 0.08), "bobcat": (0.85, 0.55, 0.05), "scraper": (0.85, 0.62, 0.05), "combine": (0.55, 0.08, 0.06)}.get(name, (0.5, 0.5, 0.5))
    pm = paintm(col, f"mach{col}", 0.45, 0.3)
    P = (lambda a, s, z: V((a, s, z))) if ax == "x" else (lambda a, s, z: V((s, a, z)))
    L0, L1 = (lo.x, hi.x) if ax == "x" else (lo.y, hi.y); S0, S1 = (lo.y, hi.y) if ax == "x" else (lo.x, hi.x); L = L1 - L0; S = S1 - S0
    def bx(a0, a1, s0, s1, z0, z1, m):
        p, q = P(L0 + L * a0, S0 + S * s0, lo.z + H * z0), P(L0 + L * a1, S0 + S * s1, lo.z + H * z1); box(V(map(min, p, q)), V(map(max, p, q)), m)
    def wheel(a, s, r, w):
        p = P(L0 + L * a, S0 + S * s, lo.z + r); d = (V((0, w / 2, 0)) if ax == "x" else V((w / 2, 0, 0)))
        cyl(p - d, p + d, r, tyre_mat(), 24); cyl(p - d * 1.05, p + d * 1.05, r * 0.55, pm, 16)
    if name == "combine":
        bx(0.25, 0.95, 0.2, 0.8, 0.15, 0.75, pm); bx(0.0, 0.25, 0.0, 1.0, 0.03, 0.22, pm); bx(0.28, 0.5, 0.3, 0.7, 0.75, 1.0, glass())
        bx(0.55, 0.9, 0.25, 0.75, 0.75, 0.92, pm)
        for s in (0.15, 0.85): wheel(0.35, s, H * 0.27, S * 0.12)
        for s in (0.25, 0.75): wheel(0.85, s, H * 0.17, S * 0.08)
        cyl(P(L0 + L * 0.05, S0, lo.z + H * 0.2), P(L0 + L * 0.05, S1, lo.z + H * 0.2), H * 0.08, pm, 12)
        return
    if name == "scraper":
        bx(0.0, 0.25, 0.15, 0.85, 0.3, 0.75, pm); bx(0.08, 0.22, 0.3, 0.7, 0.75, 1.0, glass()); bx(0.25, 0.45, 0.4, 0.6, 0.45, 0.6, pm)
        bx(0.45, 0.95, 0.05, 0.95, 0.2, 0.85, pm)
        for a in (0.12, 0.85):
            for s in (0.08, 0.92): wheel(a, s, H * 0.3, S * 0.16)
        return
    if name == "bobcat":
        bx(0.15, 0.85, 0.15, 0.85, 0.15, 0.55, pm); bx(0.25, 0.75, 0.2, 0.8, 0.55, 1.0, flat("cagegrey", (0.1, 0.1, 0.1), 0.5, 0.4))
        bx(0.0, 0.15, 0.05, 0.95, 0.0, 0.3, steel())
        for a in (0.3, 0.75):
            for s in (0.08, 0.92): wheel(a, s, H * 0.18, S * 0.16)
        return
    # tractors: big rear wheels, hood, cab
    bx(0.05, 0.6, 0.35, 0.65, 0.25, 0.6, pm); bx(0.55, 0.95, 0.25, 0.75, 0.3, 0.55, pm)
    if name == "tractor": bx(0.6, 0.95, 0.25, 0.75, 0.55, 1.0, glass())
    else: cyl(P(L0 + L * 0.7, S0 + S * 0.5, lo.z + H * 0.55), P(L0 + L * 0.7, S0 + S * 0.5, lo.z + H * 0.7), H * 0.05, rubber(), 12)
    cyl(P(L0 + L * 0.15, S0 + S * 0.5, lo.z + H * 0.6), P(L0 + L * 0.15, S0 + S * 0.5, lo.z + H * 0.95), H * 0.03, steel(), 8)
    for s in (0.1, 0.9): wheel(0.78, s, H * 0.42, S * 0.18); wheel(0.15, s, H * 0.22, S * 0.12)

def b_wreck(lo, hi, name):
    """Cargo plane wreck pieces: broken fuselage, nose, wing, scorched."""
    from objects import aircraft
    rnd = random.Random(name); c = (lo + hi) / 2
    burnt = acg("MetalPlates006", 0.1, ramp=((0.05, 0.05, 0.05), (0.5, 0.5, 0.48)), rough=0.7, metal=0.5, name="burnt")
    if "wing" in name:
        o = prism([(lo.x, 0), (hi.x, -0.6), (hi.x, 0.6), (lo.x, 1.4)], lo.y + (hi.y - lo.y) * 0.3, hi.y - (hi.y - lo.y) * 0.2, burnt, axis="y"); o.location.z = lo.z + 2; o.rotation_euler.y = 0.12
        cyl(V((c.x, c.y, lo.z + 1)), V((c.x, c.y + 6, lo.z + 1)), 3, burnt, 16)
        return
    L = hi.y - lo.y; R = min(hi.x - lo.x, hi.z - lo.z) / 2 * 0.9
    if "nose" in name:
        prof = [(0.01, 0), (R * 0.6, L * 0.15), (R * 0.95, L * 0.45), (R, L)]
    else:
        prof = [(R, 0), (R, L * 0.5), (R * 0.95, L)]
    o = lathe(prof, V((0, 0, 0)), burnt, 32); o.rotation_euler.x = -math.pi / 2; o.location = (c.x, lo.y, lo.z + R)
    o.rotation_euler.y = rnd.uniform(-0.15, 0.15)

def b_slide_door(lo, hi, name):
    box(lo, hi, oldwood())
    W = max(hi.x - lo.x, hi.y - lo.y); ax = long_axis(lo, hi)
    for f in (0.0, 1.0):
        if ax == "x": x = lo.x + (hi.x - lo.x) * f; beam(V((lo.x, lo.y - 0.1, lo.z)), V((hi.x, lo.y - 0.1, hi.z)), 0.4, wood())
        else: beam(V((lo.x - 0.1, lo.y, lo.z)), V((lo.x - 0.1, hi.y, hi.z)), 0.4, wood())

def b_barn(lo, hi, name):
    """Barn pieces: plank walls with frame posts, roof, floor, doors."""
    red = acg("Planks012", 1 / 6.0, ramp=((0.18, 0.05, 0.04), (0.5, 0.14, 0.1)), name="barnred")
    if "roof" in name:
        gable_roof(V((lo.x + 1, lo.y + 1, 0)), V((hi.x - 1, hi.y - 1, 0)), lo.z + 1, hi.z - 0.6, roofing(), over=1.0); return
    if "interior" in name:
        box(V((lo.x, lo.y, lo.z)), V((hi.x, hi.y, lo.z + 0.3)), planks())
        for f in (0.25, 0.5, 0.75):
            y = lo.y + (hi.y - lo.y) * f
            for x in (lo.x + 1, hi.x - 2): box(V((x, y - 0.5, lo.z)), V((x + 1, y + 0.5, hi.z)), wood())
            box(V((lo.x + 1, y - 0.5, hi.z - 1.2)), V((hi.x - 1, y + 0.5, hi.z)), wood())
        return
    W, D, H = hi - lo
    if min(W, D) < 1.5 or "door" in name:   # thin wall or door panel
        if W >= D: box(V((lo.x, (lo.y + hi.y) / 2 - 0.25, lo.z)), V((hi.x, (lo.y + hi.y) / 2 + 0.25, hi.z)), red)
        else: box(V(((lo.x + hi.x) / 2 - 0.25, lo.y, lo.z)), V(((lo.x + hi.x) / 2 + 0.25, hi.y, hi.z)), red)
        if "door" in name:
            if W >= D: cyl(V((lo.x, (lo.y + hi.y) / 2 - 0.3, lo.z)), V((hi.x, (lo.y + hi.y) / 2 - 0.3, hi.z)), 0.2, paintm((0.85, 0.84, 0.8), "trim"), 4)
        return
    # gable wall piece: plank wall in the long direction with a gable top
    if W >= D:
        y = (lo.y + hi.y) / 2; box(V((lo.x, y - 0.3, lo.z)), V((hi.x, y + 0.3, lo.z + H * 0.6)), red)
        prism([(lo.x, lo.z + H * 0.6), (hi.x, lo.z + H * 0.6), ((lo.x + hi.x) / 2, hi.z)], y - 0.3, y + 0.3, red, axis="y")
        box(V((lo.x + W * 0.3, y - 0.4, lo.z)), V((lo.x + W * 0.7, y + 0.4, lo.z + H * 0.45)), paintm((0.85, 0.84, 0.8), "trim"))
    else:
        x = (lo.x + hi.x) / 2; box(V((x - 0.3, lo.y, lo.z)), V((x + 0.3, hi.y, hi.z)), red)
        for k in range(int(D / 12) + 1): y = lo.y + D * k / max(1, int(D / 12)); box(V((x - 0.5, y - 0.5, lo.z)), V((x + 0.5, y + 0.5, hi.z)), wood())

def b_people(lo, hi, name):
    """Track-side person (flag girl, board man): simple figure in kit colours."""
    c = (lo + hi) / 2; H = hi.z - max(lo.z, 0); z0 = max(lo.z, 0); s = H / 6.0
    shirt = acg("Fabric030", 0.3, ramp=((0.45, 0.05, 0.04), (0.8, 0.12, 0.08)) if "girl" in name else ((0.05, 0.1, 0.25), (0.12, 0.22, 0.5)), name=f"shirt_{name}")
    pants = acg("Fabric030", 0.3, ramp=((0.04, 0.04, 0.05), (0.15, 0.15, 0.17)), name="pantsdark"); skin = flat("skinf", (0.62, 0.45, 0.35), 0.6)
    for sx in (-0.35, 0.35): cyl(V((c.x + sx * s, c.y, z0)), V((c.x + sx * s, c.y, z0 + 2.8 * s)), 0.32 * s, pants, 12)
    cyl(V((c.x, c.y, z0 + 2.8 * s)), V((c.x, c.y, z0 + 4.7 * s)), 0.75 * s, shirt, 16, r2=0.85 * s)
    for sx in (-1, 1): cyl(V((c.x + sx * 0.9 * s, c.y, z0 + 4.6 * s)), V((c.x + sx * 1.1 * s, c.y, z0 + 3.0 * s)), 0.2 * s, shirt, 8)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5 * s, location=(c.x, c.y, z0 + 5.3 * s)); bpy.context.object.data.materials.append(skin)
    if "flag" in name:
        cyl(V((c.x + 1.1 * s, c.y, z0 + 3.0 * s)), V((c.x + 1.1 * s, c.y, hi.z)), 0.05 * s, wood(), 6)
        box(V((c.x + 1.1 * s, c.y - 0.02, hi.z - 1.2 * s)), V((c.x + 2.6 * s, c.y + 0.02, hi.z)), flat("flaggreen", (0.1, 0.5, 0.15), 0.8))
    else:
        box(V((c.x - 1.2 * s, c.y - 0.4 * s, z0 + 4.6 * s)), V((c.x + 1.2 * s, c.y - 0.35 * s, hi.z)), paintm((0.92, 0.92, 0.9), "board"))

STRUCT = [
    (r"^lodge$|^skilodge$", b_lodge), (r"^co_opbrick$", b_brick), (r"^uigarage$", b_shed_metal), (r"^miningshack01$|^miningshed01$", b_shack),
    (r"^silo$", b_silo), (r"^observatory$", b_observatory), (r"^observatorycone$", b_dome), (r"^pyramid$", b_pyramid), (r"^ruin_ziggurat$", b_ziggurat),
    (r"^ruins2?$", b_ruins), (r"^ufo$", b_ufo), (r"^blimp$", b_blimp), (r"^missilesilo$", b_missilesilo), (r"^culvert0\d$|^rrt_culvert$", b_culvert),
    (r"^loadingramp$|^ramp01$", b_ramp), (r"^skilodge_patio$|^table_patio_deck$", b_deck), (r"^bridge01$|^sl-bridge$|^pit_bridge$", b_girder_bridge),
    (r"^pedbrdg_metal$", b_truss_bridge), (r"^pedbrdg_wood$", lambda lo, hi, n: b_truss_bridge(lo, hi, n, timber=True)),
    (r"^bridge_old01$|^trestle$|^trestleblock$", b_trestle), (r"^bridge_debris0\d$", b_debris), (r"^track(_short|end|radius)?$", b_track),
    (r"^trackcrossing$", b_crossing), (r"^rrsupport$|^support$", b_rail_support), (r"^rrt_elev(radius)?$|^rrtrack_supported$", b_elev_track),
    (r"^skijump$", b_skijump), (r"^twr-|^ant_cel-xmit$", b_lattice_tower), (r"^telephone_pole01$|^lightpole$|^speakerpole$|^signal$|^antenna01$", b_mast),
    (r"^skiliftpost$", b_skiliftpost), (r"^ht2?_wire|^lift_wire", b_wire), (r"^tm_head$", b_headframe), (r"^container01$", b_container),
    (r"^propanetank01$|^watertank01$", b_tank), (r"^finish(_big)?$|^waypoint_big$", b_finish), (r"^goalpost$", b_goalpost), (r"^oil_well$", b_pumpjack),
    (r"^windmill_pump$", b_windpump), (r"^outhouse01$|^toilet$|^doghouse01$|^pumpshack$|^duckblind$", b_hut), (r"^carport01$", b_carport),
    (r"^awning0\d$", b_awning), (r"^picnic_table01$|^table_patio$", b_table), (r"^cargo_(boxes|sacks)$|^greenboxes01$|^cabinets_\d$|^toolbox$", b_crate_pile),
    (r"^(soda|coffee)_machine01", b_machine), (r"^fridge01$", b_fridge), (r"^lawnchair01$", b_chair), (r"^palette01$", b_pallet), (r"^steps01$", b_steps),
    (r"^rdblock[ab]$", b_jersey), (r"^cattleguard$", b_cattleguard), (r"^gate_[lr]01$|^fence_chlinkgate$", b_gate), (r"^metalarch$", b_metalarch),
    (r"^truckpodium$", b_podium_truck), (r"^still01$", b_still), (r"^scarecrow$", b_scarecrow),
    (r"^(tractor|tractor_old|bobcat|scraper|combine)$", b_wagon_machine), (r"^cargowreck_", b_wreck), (r"^bn_slide$", b_slide_door), (r"^bn_", b_barn),
    (r"^(flaggirl|30secman)$", b_people),
]
