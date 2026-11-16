"""Free stand-ins for the stock track objects that player-made tracks
borrow by name. Each is new art fitted to the stock object's size so a
track's layout still works. Run with Blender's Python (bpy):

    python blender/objects.py [name ...]

Reads stock_sizes.tsv (name, game-space bounds in feet), writes
build/objects/<name>.glb and build/objects/stockmap.toml.
"""
import os, sys, math, random, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector
from common import *
from common import _box_uv

BRANDS = sorted(f[:-4] for f in os.listdir(os.path.join(BUILD, "brands")) if f.endswith(".png") and "_" not in f)
def brand_for(name):
    return BRANDS[sum(map(ord, name)) % len(BRANDS)]
def brand_mat(name):
    b = brand_for(name)
    return image_mat(os.path.join(BUILD, "brands", b + ".png"), "brand_" + b, rough=0.8)

def B(lo, hi):
    """game bounds -> Blender lo/hi (X=x, Y=z, Z=y)."""
    return Vector((lo[0], lo[2], lo[1])), Vector((hi[0], hi[2], hi[1]))

# ---- materials ----
def hay(): return acg("ThatchedRoof001A", 0.5, ramp=((0.05, 0.03, 0.01), (0.62, 0.45, 0.17)), name="hay")
def twine(): return flat("twine", (0.55, 0.42, 0.22), 0.9)
def wood(): return acg("Wood049", 0.25, name="wood")
def steel(): return acg("MetalPlates006", 0.3, tint=(0.45, 0.46, 0.48), rough=0.55, metal=0.7, name="steel")
def galv(): return flat("galv", (0.55, 0.56, 0.57), 0.5, 0.9)
def rubber(): return acg("Rubber004", 0.5, name="rubber")
def canvas(rgb): return acg("Fabric030", 0.3, ramp=(tuple(c * 0.55 for c in rgb), tuple(rgb)), name=f"canvas{rgb}")
def plywood(): return acg("Wood087", 0.2, name="plywood")

# ---- builders: each gets Blender-space lo, hi and the stock name ----
def bale_block(lo, hi, wrap=None):
    box(lo, hi, hay(), bevel=min(hi - lo) * 0.06, uv_scale=4)
    # two twine bands round the long axis
    long = 0 if (hi.x - lo.x) > (hi.y - lo.y) else 1
    for f in (0.3, 0.7):
        a = lo.copy(); b = hi.copy(); e = 0.03
        if long == 0: a.x = b.x = lo.x + (hi.x - lo.x) * f; a.x -= 0.08; b.x += 0.08
        else: a.y = b.y = lo.y + (hi.y - lo.y) * f; a.y -= 0.08; b.y += 0.08
        a -= Vector((e, e, e)) * Vector((long != 0, long != 1, 0)); b += Vector((e, e, e))
        a.z = lo.z + 0.02; box(a, b, twine())
    if wrap:
        # sponsor banner wrapped on both long sides, top 80% of the height
        h0, h1 = lo.z + (hi.z - lo.z) * 0.12, lo.z + (hi.z - lo.z) * 0.9
        if long == 0:
            for y, s in ((lo.y - 0.04, -1), (hi.y + 0.04, 1)): _banner_quad(Vector((lo.x + 0.3, y, h0)), Vector((hi.x - 0.3, y, h1)), wrap, s)
        else:
            for x, s in ((lo.x - 0.04, -1), (hi.x + 0.04, 1)): _banner_quad(Vector((x, lo.y + 0.3, h0)), Vector((x, hi.y - 0.3, h1)), wrap, s)

def _banner_quad(a, b, mat, side):
    bm = bmesh.new()
    if abs(a.y - b.y) < 1e-4:
        vs = [(a.x, a.y, a.z), (b.x, a.y, a.z), (b.x, a.y, b.z), (a.x, a.y, b.z)]
    else:
        vs = [(a.x, a.y, a.z), (a.x, b.y, a.z), (a.x, b.y, b.z), (a.x, a.y, b.z)]
    vv = [bm.verts.new(v) for v in vs]
    f = bm.faces.new(vv if side > 0 else vv[::-1])
    uv = bm.loops.layers.uv.verify()
    # stretch the banner art across; keep it upright and readable from outside
    uvs = [(0, 0), (1, 0), (1, 1), (0, 1)] if side > 0 else [(1, 0), (0, 0), (0, 1), (1, 1)]
    if side > 0: uvs = [(1, 0), (0, 0), (0, 1), (1, 1)] if abs(a.y - b.y) < 1e-4 else uvs
    else: uvs = [(0, 0), (1, 0), (1, 1), (0, 1)][::-1] if False else uvs
    for l, t in zip(f.loops, [(0, 0), (1, 0), (1, 1), (0, 1)] if side > 0 else [(1, 0), (0, 0), (0, 1), (1, 1)]):
        l[uv].uv = (1 - t[0], 1 - t[1])
    me = bpy.data.meshes.new("banner"); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new("banner", me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat)
    # solidify so it reads from both sides at a glance
    s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.03

def triplebale(lo, hi, name):
    n = 3; long = 0 if hi.x - lo.x > hi.y - lo.y else 1
    L = (hi[long] - lo[long]) / n
    for i in range(n):
        a = lo.copy(); b = hi.copy(); a[long] = lo[long] + i * L + 0.05; b[long] = lo[long] + (i + 1) * L - 0.05
        bale_block(a, b, wrap=brand_mat(name) if name[-1] in "2468" and i == 1 else None)

def single_bale(lo, hi, name):
    bale_block(lo, hi, wrap=brand_mat(name))

def bale_stack(lo, hi, name):
    # pyramid of big bales, 3 tiers
    tiers = 3; th = (hi.z - lo.z) / tiers
    for t in range(tiers):
        inset = (hi.x - lo.x) * 0.17 * t
        rows = tiers - t
        w = (hi.y - lo.y - 2 * inset) / rows
        for r in range(rows):
            a = Vector((lo.x + inset, lo.y + inset + r * w + 0.05, lo.z + t * th))
            b = Vector((hi.x - inset, lo.y + inset + (r + 1) * w - 0.05, lo.z + (t + 1) * th - 0.02))
            bale_block(a, b)

def billboard(lo, hi, name):
    top = hi.z; panel_h = min(7.0, (hi.z - max(lo.z, 0)) * 0.95); bottom = top - panel_h
    th = max(hi.y - lo.y, 0.25)
    y0 = lo.y
    # panel: plywood back with the banner on the face (-Y face faces the track)
    box(Vector((lo.x, y0 + th * 0.4, bottom)), Vector((hi.x, y0 + th, top)), plywood(), uv_scale=4)
    _banner_quad(Vector((lo.x + 0.1, y0 + th * 0.4 - 0.02, bottom + 0.1)), Vector((hi.x - 0.1, y0 + th * 0.4 - 0.02, top - 0.1)), brand_mat(name), -1)
    _banner_quad(Vector((lo.x + 0.1, y0 + th + 0.02, bottom + 0.1)), Vector((hi.x - 0.1, y0 + th + 0.02, top - 0.1)), brand_mat(name), 1)
    for x in (lo.x + (hi.x - lo.x) * 0.2, hi.x - (hi.x - lo.x) * 0.2):
        box(Vector((x - 0.25, y0 + th + 0.02, lo.z)), Vector((x + 0.25, y0 + th + 0.52, top - 0.3)), wood(), uv_scale=3)

def giant_tyre(lo, hi, name):
    # two upright earthmover tyres side by side, filling the footprint
    n = 2 if (hi.x - lo.x) > 1.5 * (hi.z - lo.z) else 1
    R = (hi.z - lo.z) / 2; W = hi.y - lo.y; seg = (hi.x - lo.x) / n
    for i in range(n):
        cx = lo.x + seg * (i + 0.5); c = Vector((cx, (lo.y + hi.y) / 2, lo.z + R))
        _tyre(c, R, R * 0.48, W, rubber())

def _tyre(c, R, Rin, W, mat):
    """Earthmover-style tyre standing on its tread, axle along Y."""
    bm = bmesh.new(); seg = 96
    t = R - Rin; rc = min(t, W) * 0.22
    sec = [(Rin, -W * 0.42), (R - t * 0.45, -W * 0.5)]
    for i in range(7):
        a = (math.pi / 2) * i / 6
        sec.append((R - rc + rc * math.sin(a), -W / 2 + rc - rc * math.cos(a)))
    sec += [(R - rc + rc * math.sin(math.pi / 2 - (math.pi / 2) * i / 6), W / 2 - rc + rc * math.sin((math.pi / 2) * i / 6)) for i in range(7)]
    sec += [(R - t * 0.45, W * 0.5), (Rin, W * 0.42)]
    rings = []
    for k in range(seg):
        ang = 2 * math.pi * k / seg; ca, sa = math.cos(ang), math.sin(ang)
        rings.append([bm.verts.new((r * ca, y, r * sa)) for r, y in sec])
    for k in range(seg):
        a, b = rings[k], rings[(k + 1) % seg]
        n = len(sec)
        for i in range(n): bm.faces.new((a[i], a[(i + 1) % n], b[(i + 1) % n], b[i]))
    # inner barrel
    # chevron lugs on the tread, two staggered rows
    nl = 26; lh = t * 0.07
    for k in range(nl):
        ang = 2 * math.pi * k / nl
        for side in (-1, 1):
            a0 = ang + side * 0.02
            pts = []
            for (u, y) in ((0, 0.04), (0.05, 0.04), (0.11, 0.36), (0.07, 0.36)):
                aa = a0 + side * u * 0.0 + u * 0.9 * (1 if side > 0 else 1)
                pts.append((aa, side * y * W))
            vb = [bm.verts.new(((R - t * 0.06) * math.cos(aa), y, (R - t * 0.06) * math.sin(aa))) for aa, y in pts]
            vt = [bm.verts.new(((R + lh) * math.cos(aa), y, (R + lh) * math.sin(aa))) for aa, y in pts]
            bm.faces.new(vt)
            for i in range(4): bm.faces.new((vb[i], vb[(i + 1) % 4], vt[(i + 1) % 4], vt[i]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        for l in f.loops:
            v = l.vert.co
            l[uv].uv = (v.x / 6, v.z / 6) if abs(f.normal.y) > 0.6 else (math.atan2(v.z, v.x) * R / 6, v.y / 6)
    me = bpy.data.meshes.new("tyre"); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    o = bpy.data.objects.new("tyre", me); bpy.context.scene.collection.objects.link(o); o.location = c; me.materials.append(mat)
    # rim/hub disc set into the hole
    cyl((c.x, c.y - W * 0.3, c.z), (c.x, c.y + W * 0.3, c.z), Rin, flat("tyre_rim", (0.32, 0.26, 0.08), 0.6, 0.3), 48)
    return o

def camera_platform(lo, hi, name):
    st = steel(); deck = plywood()
    top = hi.z - 3.5
    xs = [lo.x + 0.5, hi.x - 0.5]; ys = [lo.y + 0.5, hi.y - 0.5]
    for x in xs:
        for y in ys: cyl((x, y, lo.z), (x, y, hi.z), 0.25, st, 12)
    # cross bracing every 6 ft
    z = lo.z + 1
    while z < top:
        for y in ys: cyl((xs[0], y, z), (xs[1], y, z + 5), 0.12, st, 8); cyl((xs[0], y, z + 5), (xs[1], y, z), 0.12, st, 8)
        for x in xs: cyl((x, ys[0], z), (x, ys[1], z + 5), 0.12, st, 8)
        for x in xs:
            cyl((x, ys[0], z), (x, ys[1], z), 0.15, st, 8)
        for y in ys: cyl((xs[0], y, z), (xs[1], y, z), 0.15, st, 8)
        z += 6
    box(Vector((lo.x, lo.y, top)), Vector((hi.x, hi.y, top + 0.4)), deck, uv_scale=6)
    for y in ys: cyl((xs[0], y, top + 3.4), (xs[1], y, top + 3.4), 0.12, st, 8)
    for x in xs: cyl((x, ys[0], top + 3.4), (x, ys[1], top + 3.4), 0.12, st, 8)
    # ladder
    lx = (xs[0] + xs[1]) / 2
    for dx in (-0.8, 0.8): cyl((lx + dx, ys[0] - 0.4, lo.z), (lx + dx, ys[0] - 0.4, top + 3.4), 0.1, st, 8)
    z = lo.z + 1
    while z < top: cyl((lx - 0.8, ys[0] - 0.4, z), (lx + 0.8, ys[0] - 0.4, z), 0.07, st, 6); z += 1
    # canopy
    box(Vector((lo.x - 0.3, lo.y - 0.3, hi.z - 0.25)), Vector((hi.x + 0.3, hi.y + 0.3, hi.z)), canvas((0.22, 0.24, 0.27)), uv_scale=8)
    for x in xs:
        for y in ys: cyl((x, y, top), (x, y, hi.z), 0.12, st, 8)

def rail(lo, hi, name):
    w = wood(); L = hi.x - lo.x; n = max(2, round(L / 8)) + 1
    cy = (lo.y + hi.y) / 2
    for i in range(n):
        x = lo.x + L * i / (n - 1); box(Vector((x - 0.2, cy - 0.2, lo.z)), Vector((x + 0.2, cy + 0.2, hi.z)), w, uv_scale=3)
    for zf in (0.45, 0.85):
        z = lo.z + (hi.z - lo.z) * zf; box(Vector((lo.x, cy - 0.12, z - 0.25)), Vector((hi.x, cy + 0.12, z + 0.25)), w, uv_scale=3)

def drum(lo, hi, name):
    c = (lo + hi) / 2; r = min(hi.x - lo.x, hi.y - lo.y) / 2
    cyl((c.x, c.y, lo.z), (c.x, c.y, hi.z), r, flat("drum", (0.12, 0.2, 0.3), 0.55, 0.3), 24)
    for f in (0.33, 0.66): cyl((c.x, c.y, lo.z + (hi.z - lo.z) * f - 0.05), (c.x, c.y, lo.z + (hi.z - lo.z) * f + 0.05), r + 0.03, galv(), 24)

def crate(lo, hi, name):
    box(lo, hi, plywood(), bevel=0.05, uv_scale=3)
    for z in (lo.z + 0.1, hi.z - 0.35):
        box(Vector((lo.x - 0.03, lo.y - 0.03, z)), Vector((hi.x + 0.03, hi.y + 0.03, z + 0.25)), wood(), uv_scale=3)

def tent(lo, hi, name):
    st = galv(); c = canvas((0.75, 0.75, 0.72)) if "01" in name else canvas((0.3, 0.4, 0.55))
    eave = lo.z + (hi.z - lo.z) * 0.62
    for x in (lo.x + 0.3, hi.x - 0.3):
        for y in (lo.y + 0.3, hi.y - 0.3): cyl((x, y, max(lo.z, 0)), (x, y, eave), 0.15, st, 8)
    bm = bmesh.new()
    v = [bm.verts.new(p) for p in [(lo.x, lo.y, eave), (hi.x, lo.y, eave), (hi.x, hi.y, eave), (lo.x, hi.y, eave), ((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, hi.z)]]
    for a, b in ((0, 1), (1, 2), (2, 3), (3, 0)): bm.faces.new((v[a], v[b], v[4]))
    _box_uv(bm, 6)
    me = bpy.data.meshes.new("roof"); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new("roof", me); bpy.context.scene.collection.objects.link(o); me.materials.append(c)
    s = o.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.05
    # valance strip
    for (a, b) in [((lo.x, lo.y - 0.02, eave - 1.2), (hi.x, lo.y + 0.03, eave)), ((lo.x, hi.y - 0.03, eave - 1.2), (hi.x, hi.y + 0.02, eave)),
                   ((lo.x - 0.02, lo.y, eave - 1.2), (lo.x + 0.03, hi.y, eave)), ((hi.x - 0.03, lo.y, eave - 1.2), (hi.x + 0.02, hi.y, eave))]:
        box(Vector(a), Vector(b), c, uv_scale=6)

def chainlink(lo, hi, name):
    st = galv(); L = hi.x - lo.x; cy = (lo.y + hi.y) / 2
    n = max(1, round(L / 10))
    for i in range(n + 1):
        x = lo.x + L * i / n; cyl((x, cy, lo.z), (x, cy, hi.z), 0.2, st, 10)
    cyl((lo.x, cy, hi.z - 0.2), (hi.x, cy, hi.z - 0.2), 0.12, st, 8)
    m = _fence_mat()
    bm = bmesh.new(); v = [bm.verts.new(p) for p in [(lo.x, cy, lo.z + 0.2), (hi.x, cy, lo.z + 0.2), (hi.x, cy, hi.z - 0.2), (lo.x, cy, hi.z - 0.2)]]
    f = bm.faces.new(v); _box_uv(bm, 4)
    me = bpy.data.meshes.new("mesh"); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new("mesh", me); bpy.context.scene.collection.objects.link(o); me.materials.append(m)

def _fence_mat():
    if "chainlink" in bpy.data.materials: return bpy.data.materials["chainlink"]
    m = acg("Fence007A", 0.25, name="chainlink", metal=0.8)
    nt = m.node_tree; b = nt.nodes["Principled BSDF"]
    p = os.path.join(CACHE, "ambientcg", "Fence007A", "Fence007A_2K-JPG_Opacity.jpg")
    if os.path.exists(p):
        t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(p); t.image.colorspace_settings.name = "Non-Color"
        mp = [n for n in nt.nodes if n.type == "MAPPING"][0]; nt.links.new(mp.outputs["Vector"], t.inputs["Vector"])
        nt.links.new(t.outputs["Color"], b.inputs["Alpha"])
    return m

def waypoint(lo, hi, name):
    # checkpoint arch: two padded legs and an inflatable-style beam
    leg = canvas((0.82, 0.82, 0.8)); beam = brand_mat("checkpoint")
    W = 3.0; z0 = max(lo.z, 0)
    for x in (lo.x + W / 2, hi.x - W / 2):
        cyl((x, (lo.y + hi.y) / 2, lo.z), (x, (lo.y + hi.y) / 2, hi.z - W), W / 2, leg, 24)
    cy = (lo.y + hi.y) / 2
    cyl((lo.x, cy, hi.z - W / 2), (hi.x, cy, hi.z - W / 2), W / 2, leg, 24)
    _banner_quad(Vector((lo.x + W, cy - W / 2 - 0.05, hi.z - W + 0.2)), Vector((hi.x - W, cy - W / 2 - 0.05, hi.z - 0.2)), beam, -1)
    _banner_quad(Vector((lo.x + W, cy + W / 2 + 0.05, hi.z - W + 0.2)), Vector((hi.x - W, cy + W / 2 + 0.05, hi.z - 0.2)), beam, 1)

def podium(lo, hi, name):
    w = (hi.x - lo.x) / 3; base = max(lo.z, 0)
    for i, h in enumerate((0.6, 1.0, 0.45)):
        top = base + 4.5 * h
        box(Vector((lo.x + i * w, lo.y, lo.z)), Vector((lo.x + (i + 1) * w - 0.05, hi.y - (hi.y - lo.y) * 0.4, top)), acg("Plastic010", 0.5, tint=(0.82, 0.82, 0.8), name="podium"), bevel=0.08, uv_scale=4)
    # backdrop
    bt = hi.y - 0.4
    box(Vector((lo.x, bt, base)), Vector((hi.x, hi.y, hi.z)), steel(), uv_scale=4)
    _banner_quad(Vector((lo.x + 0.3, bt - 0.03, base + (hi.z - base) * 0.35)), Vector((hi.x - 0.3, bt - 0.03, hi.z - 0.3)), brand_mat(name), -1)

def concrete(): return acg("Concrete034", 0.12, ramp=((0.22, 0.21, 0.2), (0.55, 0.54, 0.52)), name="concrete")
def siding(): return acg("CorrugatedSteel007A", 0.2, ramp=((0.25, 0.26, 0.27), (0.62, 0.63, 0.62)), metal=0.4, name="siding")
def roofing(): return acg("CorrugatedSteel007A", 0.2, ramp=((0.12, 0.08, 0.06), (0.36, 0.2, 0.14)), metal=0.3, name="roofing")

def pole(lo, hi, name):
    d = hi - lo; ax = max(range(3), key=lambda i: d[i]); c = (lo + hi) / 2
    a = c.copy(); b = c.copy(); a[ax] = lo[ax]; b[ax] = hi[ax]
    r = max(0.15, min(sorted(d)[1], 1.6) / 2)
    cyl(a, b, r, galv(), 10)

def startgate(lo, hi, name):
    cy = (lo.y + hi.y) / 2; base = lo.z
    box(Vector((lo.x, cy - 0.4, base)), Vector((hi.x, cy + 0.4, base + 0.3)), steel(), uv_scale=3)
    n = max(1, int((hi.x - lo.x) / 3.2))
    w = (hi.x - lo.x) / n
    for i in range(n):
        x0 = lo.x + i * w + 0.15
        box(Vector((x0, cy - 0.06, base + 0.3)), Vector((x0 + w - 0.3, cy + 0.06, hi.z)), flat("gate_grey", (0.38, 0.39, 0.4), 0.5, 0.6))

def retainwall(lo, hi, name):
    box(lo, hi, concrete(), bevel=0.1, uv_scale=8)

def watertower(lo, hi, name):
    c = (lo + hi) / 2; R = min(hi.x - lo.x, hi.y - lo.y) / 2; tank0 = lo.z + (hi.z - lo.z) * 0.6
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        cyl((c.x + math.cos(a) * R * 0.9, c.y + math.sin(a) * R * 0.9, lo.z), (c.x + math.cos(a) * R * 0.6, c.y + math.sin(a) * R * 0.6, tank0), 0.5, steel(), 10)
    cyl((c.x, c.y, tank0), (c.x, c.y, hi.z - R * 0.4), R * 0.85, flat("tank", (0.55, 0.56, 0.55), 0.6, 0.4), 32)
    cyl((c.x, c.y, hi.z - R * 0.4), (c.x, c.y, hi.z), R * 0.85, flat("tank", (0.55, 0.56, 0.55), 0.6, 0.4), 32, r2=0.3)

def helipad(lo, hi, name):
    box(lo, hi, concrete(), uv_scale=10)
    c = (lo + hi) / 2; m = flat("paint_white", (0.75, 0.75, 0.72), 0.7)
    w = (hi.x - lo.x) * 0.3
    for a, b in (((c.x - w / 2, c.y - w * 0.6), (c.x - w / 2 + 2, c.y + w * 0.6)), ((c.x + w / 2 - 2, c.y - w * 0.6), (c.x + w / 2, c.y + w * 0.6)), ((c.x - w / 2, c.y - 1), (c.x + w / 2, c.y + 1))):
        box(Vector((a[0], a[1], hi.z)), Vector((b[0], b[1], hi.z + 0.05)), m)

def building(lo, hi, name):
    wall_top = lo.z + (hi.z - max(lo.z, 0)) * 0.7
    box(Vector((lo.x + 0.5, lo.y + 0.5, lo.z)), Vector((hi.x - 0.5, hi.y - 0.5, wall_top)), siding(), uv_scale=8)
    # pitched roof along the longer side
    bm = bmesh.new(); long_x = (hi.x - lo.x) >= (hi.y - lo.y)
    if long_x:
        pts = [(lo.x, lo.y, wall_top), (hi.x, lo.y, wall_top), (hi.x, (lo.y + hi.y) / 2, hi.z), (lo.x, (lo.y + hi.y) / 2, hi.z), (lo.x, hi.y, wall_top), (hi.x, hi.y, wall_top)]
        v = [bm.verts.new(p) for p in pts]; bm.faces.new((v[0], v[1], v[2], v[3])); bm.faces.new((v[3], v[2], v[5], v[4]))
        bm.faces.new((v[0], v[3], v[4])); bm.faces.new((v[1], v[5], v[2]))
    else:
        pts = [(lo.x, lo.y, wall_top), (lo.x, hi.y, wall_top), ((lo.x + hi.x) / 2, hi.y, hi.z), ((lo.x + hi.x) / 2, lo.y, hi.z), (hi.x, lo.y, wall_top), (hi.x, hi.y, wall_top)]
        v = [bm.verts.new(p) for p in pts]; bm.faces.new((v[0], v[3], v[2], v[1])); bm.faces.new((v[3], v[4], v[5], v[2]))
        bm.faces.new((v[0], v[4], v[3])); bm.faces.new((v[1], v[2], v[5]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces); _box_uv(bm, 8)
    me = bpy.data.meshes.new("roof"); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new("roof", me); bpy.context.scene.collection.objects.link(o); me.materials.append(roofing())
    s_ = o.modifiers.new("s", "SOLIDIFY"); s_.thickness = 0.3
    # a door and two windows on the -Y face
    dx = (lo.x + hi.x) / 2
    box(Vector((dx - 2, lo.y + 0.4, lo.z)), Vector((dx + 2, lo.y + 0.55, lo.z + 7)), wood(), uv_scale=3)
    for wx in (lo.x + (hi.x - lo.x) * 0.22, hi.x - (hi.x - lo.x) * 0.22):
        box(Vector((wx - 2, lo.y + 0.4, lo.z + 3.5)), Vector((wx + 2, lo.y + 0.55, lo.z + 6.5)), flat("window", (0.05, 0.06, 0.07), 0.2, 0.0))

def dome(lo, hi, name):
    """Stadium shell: a low dome seen from inside, dark, with a ring wall."""
    c = (lo + hi) / 2; R = (hi.x - lo.x) / 2; H = hi.z - max(lo.z, 0); base = max(lo.z, 0)
    bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=24, radius=1, location=(c.x, c.y, base))
    o = bpy.context.object; o.scale = (R, R, H)
    bm = bmesh.new(); bm.from_mesh(o.data)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.co.z < -0.001], context="VERTS")
    bmesh.ops.reverse_faces(bm, faces=bm.faces); _box_uv(bm, 0.02)
    bm.to_mesh(o.data); bm.free(); o.data.materials.append(acg("MetalPlates006", 1.0, ramp=((0.03, 0.03, 0.035), (0.12, 0.12, 0.13)), name="dome_roof"))
    bpy.ops.object.shade_smooth()
    # light rings hanging from the roof
    for k in range(3):
        r = R * (0.15 + 0.12 * k); z = base + H * (0.82 - 0.05 * k)
        bpy.ops.mesh.primitive_torus_add(major_radius=r, minor_radius=2.0, major_segments=48, minor_segments=6, location=(c.x, c.y, z))
        bpy.context.object.data.materials.append(steel())

def generic(lo, hi, name):
    # unknown prop: a tarp-covered stack of crates in the same footprint
    box(lo, hi, canvas((0.28, 0.32, 0.26)), bevel=min(hi - lo) * 0.08, uv_scale=6)
    for f in (0.3, 0.7):
        x = lo.x + (hi.x - lo.x) * f
        box(Vector((x - 0.08, lo.y - 0.03, lo.z)), Vector((x + 0.08, hi.y + 0.03, hi.z + 0.03)), flat("strap", (0.12, 0.12, 0.12), 0.8))

# ---- vehicles, rail cars, aircraft, signs, fences ----
PAINT = {"blue": (0.12, 0.2, 0.32), "brown": (0.28, 0.18, 0.1), "green": (0.14, 0.24, 0.14), "white": (0.72, 0.72, 0.7), "red": (0.42, 0.08, 0.06),
         "navy": (0.06, 0.08, 0.16), "grey": (0.35, 0.36, 0.37), "cop": (0.08, 0.08, 0.09), "fire": (0.45, 0.06, 0.04), "school": (0.7, 0.5, 0.06),
         "yellow": (0.62, 0.48, 0.08), "olive": (0.25, 0.27, 0.18)}
def paint_for(name):
    for k, v in PAINT.items():
        if k in name: return flat("paint_" + k, v, 0.45, 0.3)
    if re.search(r"firetruck", name): return flat("paint_fire", PAINT["fire"], 0.45, 0.3)
    if "schoolbus" in name: return flat("paint_school", PAINT["school"], 0.45, 0.3)
    if re.search(r"tractor|combine|bobcat|scraper|dumptruck|shovel", name): return flat("paint_yellow", PAINT["yellow"], 0.5, 0.2)
    pal = [(0.3, 0.12, 0.08), (0.14, 0.18, 0.24), (0.5, 0.48, 0.44), (0.2, 0.22, 0.16), (0.36, 0.3, 0.2)]
    c = pal[sum(map(ord, name)) % len(pal)]; return flat("paint_%d" % (sum(map(ord, name)) % len(pal)), c, 0.45, 0.25)
def glass(): return flat("glass", (0.03, 0.035, 0.04), 0.15, 0.0)
def tyre_mat(): return flat("tyre", (0.03, 0.03, 0.03), 0.9)

def _axes(lo, hi, long_is_length=True):
    """Return (u, v) unit axes: u along the vehicle, v across it."""
    lx, ly = hi.x - lo.x, hi.y - lo.y
    along_x = (lx >= ly) == long_is_length
    return (Vector((1, 0, 0)), Vector((0, 1, 0))) if along_x else (Vector((0, 1, 0)), Vector((1, 0, 0)))

def _span(lo, hi, u):
    return (hi - lo).dot(u)

def _sub(lo, hi, u, v, a0, a1, b0, b1, z0, z1):
    """Box covering fractions [a0,a1] along u, [b0,b1] across v, [z0,z1] up (of the bounds)."""
    L, W, H = _span(lo, hi, u), _span(lo, hi, v), hi.z - lo.z
    p0 = lo + u * (L * a0) + v * (W * b0); p1 = lo + u * (L * a1) + v * (W * b1)
    return Vector((min(p0.x, p1.x), min(p0.y, p1.y), lo.z + H * z0)), Vector((max(p0.x, p1.x), max(p0.y, p1.y), lo.z + H * z1))

def _wheels(lo, hi, u, v, at, r_frac=0.18, inset=0.04):
    L, W, H = _span(lo, hi, u), _span(lo, hi, v), hi.z - lo.z; r = min(H * r_frac * 1.6, L * 0.12); w = W * 0.14
    for a in at:
        for side in (inset, 1 - inset):
            c = lo + u * (L * a) + v * (W * side) + Vector((0, 0, r))
            cyl(c - v * (w / 2), c + v * (w / 2), r, tyre_mat(), 20)
            cyl(c - v * (w / 2 + 0.02), c + v * (w / 2 + 0.02), r * 0.55, galv(), 16)
    return r

def vehicle(lo, hi, name):
    u, v = _axes(lo, hi); paint = paint_for(name)
    bus = re.search(r"bus|greyhound|semicab|firetruck|watertruck|tanker|dumptruck|feduptruck|companytruck|famtrk|flatbed|truckbed", name)
    rw = _wheels(lo, hi, u, v, (0.16, 0.84) if not bus else (0.14, 0.72, 0.86))
    zb = rw * 0.9 / (hi.z - lo.z)
    if re.search(r"greyhound|schoolbus", name):
        box(*_sub(lo, hi, u, v, 0, 1, 0.03, 0.97, zb, 0.97), paint, bevel=0.4)
        box(*_sub(lo, hi, u, v, 0.05, 0.95, 0.02, 0.98, 0.55, 0.8), glass())
    elif re.search(r"tanker|watertruck", name):
        box(*_sub(lo, hi, u, v, 0.72, 1, 0.05, 0.95, zb, 0.9), paint, bevel=0.3); box(*_sub(lo, hi, u, v, 0.86, 0.99, 0.04, 0.96, 0.55, 0.8), glass())
        c0 = lo + u * (_span(lo, hi, u) * 0.03) + v * (_span(lo, hi, v) / 2); c0.z = lo.z + (hi.z - lo.z) * 0.5
        cyl(c0, c0 + u * (_span(lo, hi, u) * 0.66), min(_span(lo, hi, v), hi.z - lo.z) * 0.42, galv(), 24)
    elif bus or re.search(r"truck|jeep|4runner|cop|companytruck", name):
        # cab at the front, bed or box behind
        box(*_sub(lo, hi, u, v, 0.62, 1, 0.04, 0.96, zb, 0.62), paint, bevel=0.3)
        box(*_sub(lo, hi, u, v, 0.64, 0.86, 0.06, 0.94, 0.6, 0.98), paint, bevel=0.25)
        box(*_sub(lo, hi, u, v, 0.66, 0.84, 0.05, 0.95, 0.66, 0.92), glass())
        hb = 0.55 if re.search(r"flatbed|truckbed|jeep|4runner|cop", name) else 0.95
        box(*_sub(lo, hi, u, v, 0.0, 0.6, 0.04, 0.96, zb, hb), paint if hb < 0.6 else flat("box_white", (0.6, 0.6, 0.58), 0.6), bevel=0.2)
    else:
        # car: low body, cabin in the middle
        box(*_sub(lo, hi, u, v, 0, 1, 0.04, 0.96, zb, 0.55), paint, bevel=0.35)
        box(*_sub(lo, hi, u, v, 0.28, 0.72, 0.08, 0.92, 0.5, 0.98), paint, bevel=0.3)
        box(*_sub(lo, hi, u, v, 0.3, 0.7, 0.07, 0.93, 0.56, 0.92), glass())
    if "cop" in name: box(*_sub(lo, hi, u, v, 0.45, 0.55, 0.3, 0.7, 0.98, 1.0), flat("lightbar", (0.2, 0.05, 0.05), 0.4))

def trailer(lo, hi, name):
    u, v = _axes(lo, hi); paint = paint_for(name)
    rw = _wheels(lo, hi, u, v, (0.12, 0.24))
    zb = rw * 1.8 / (hi.z - lo.z)
    if re.search(r"flatbed|haywagon|pit_trailer", name) or (hi.z - lo.z) < 6:
        box(*_sub(lo, hi, u, v, 0, 1, 0.02, 0.98, zb, zb + 0.08), wood())
        if "haywagon" in name: bale_block(*_sub(lo, hi, u, v, 0.05, 0.95, 0.1, 0.9, zb + 0.08, 1.0))
    else:
        box(*_sub(lo, hi, u, v, 0, 1, 0.02, 0.98, zb, 1.0), siding() if (sum(map(ord, name)) % 2) else flat("trailer_white", (0.62, 0.62, 0.6), 0.6), uv_scale=6, bevel=0.2)
        box(*_sub(lo, hi, u, v, 0.94, 1.0, 0.45, 0.55, 0.0, zb), steel())

def railcar(lo, hi, name):
    u, v = _axes(lo, hi); paint = paint_for(name + "rail")
    L = _span(lo, hi, u); H = hi.z - lo.z
    for a in (0.15, 0.85):
        box(*_sub(lo, hi, u, v, a - 0.08, a + 0.08, 0.1, 0.9, 0.0, 0.18), steel())
    if re.search(r"hopper|gondola|coalcar", name):
        box(*_sub(lo, hi, u, v, 0.02, 0.98, 0.04, 0.96, 0.18, 0.95), paint, bevel=0.15)
        box(*_sub(lo, hi, u, v, 0.04, 0.96, 0.08, 0.92, 0.8, 0.96), flat("coal", (0.03, 0.03, 0.03), 0.9) if "coal" in name else flat("hold", (0.05, 0.05, 0.05), 0.9))
    elif "engine" in name:
        box(*_sub(lo, hi, u, v, 0.02, 0.98, 0.06, 0.94, 0.18, 0.8), paint, bevel=0.2); box(*_sub(lo, hi, u, v, 0.78, 0.98, 0.04, 0.96, 0.18, 1.0), paint, bevel=0.2)
        box(*_sub(lo, hi, u, v, 0.8, 0.96, 0.03, 0.97, 0.75, 0.92), glass())
    else:
        box(*_sub(lo, hi, u, v, 0.01, 0.99, 0.02, 0.98, 0.18, 0.97), paint if "caboose" in name else roofing(), bevel=0.15, uv_scale=6)
        if "cattle" in name:
            for k in range(8): box(*_sub(lo, hi, u, v, 0.05 + k * 0.115, 0.1 + k * 0.115, 0.0, 1.0, 0.4, 0.85), flat("slot", (0.03, 0.03, 0.03), 0.9))
        if "caboose" in name: box(*_sub(lo, hi, u, v, 0.4, 0.6, 0.1, 0.9, 0.97, 1.0), paint)

def aircraft(lo, hi, name):
    heli = re.search(r"bell|heli", name)
    u, v = _axes(lo, hi, long_is_length=bool(heli))  # planes: span is the long side
    paint = paint_for(name); L, W, H = _span(lo, hi, u), _span(lo, hi, v), hi.z - lo.z
    mid = lo + v * (W / 2); zc = lo.z + H * (0.45 if heli else 0.35)
    if heli:
        a = mid + u * (L * 0.15); b = mid + u * (L * 0.6); a.z = b.z = zc
        cyl(a, b, H * 0.3, paint, 24); bpy.ops.mesh.primitive_uv_sphere_add(radius=H * 0.3, location=b); bpy.context.object.data.materials.append(glass())
        t = mid + u * (L * 0.98); t.z = zc + H * 0.1; cyl(a, t, H * 0.08, paint, 12)
        c = mid + u * (L * 0.45); c.z = hi.z - 0.3; cyl(c, c + Vector((0, 0, 0.15)), L * 0.48, flat("rotor", (0.05, 0.05, 0.05), 0.6), 48)
        for s in (0.25, 0.75): box(*_sub(lo, hi, u, v, 0.2, 0.7, s - 0.02, s + 0.02, 0.0, 0.05), steel())
        return
    a = mid + u * (L * 0.02); b = mid + u * (L * 0.98); a.z = b.z = zc
    r = min(H * 0.28, W * 0.06); cyl(a, b, r, paint, 24, r2=r * 0.35)
    p0, p1 = _sub(lo, hi, u, v, 0.26, 0.42, 0.0, 1.0, (zc - lo.z - r * 0.3) / H, (zc - lo.z - r * 0.1) / H); box(p0, p1, paint)
    if "biplane" in name: p0, p1 = _sub(lo, hi, u, v, 0.26, 0.42, 0.04, 0.96, 0.75, 0.8); box(p0, p1, paint)
    p0, p1 = _sub(lo, hi, u, v, 0.9, 0.99, 0.32, 0.68, (zc - lo.z) / H, (zc - lo.z) / H + 0.03); box(p0, p1, paint)
    p0, p1 = _sub(lo, hi, u, v, 0.9, 0.99, 0.49, 0.51, (zc - lo.z) / H, 0.98); box(p0, p1, paint)
    p0, p1 = _sub(lo, hi, u, v, 0.08, 0.2, 0.44, 0.56, (zc - lo.z) / H + 0.02, (zc - lo.z + r * 1.1) / H); box(p0, p1, glass())
    for s in (0.35, 0.65): _ = cyl(lo + u * (L * 0.3) + v * (W * s), lo + u * (L * 0.3) + v * (W * s) + v * 0.6, H * 0.12, tyre_mat(), 16)

def sign(lo, hi, name):
    u, v = _axes(lo, hi); L = _span(lo, hi, u)
    m = brand_mat(name) if re.search(r"trpk|ironmountain|info", name) else flat("sign_paint", (0.62, 0.5, 0.1), 0.5)
    p0, p1 = _sub(lo, hi, u, v, 0.05, 0.95, 0.45, 0.55, 0.45, 1.0); box(p0, p1, m)
    for a in (0.15, 0.85): box(*_sub(lo, hi, u, v, a - 0.02, a + 0.02, 0.4, 0.6, 0.0, 0.5), wood() if "info" in name else galv())

def splitrail(lo, hi, name):
    u, v = _axes(lo, hi); L = _span(lo, hi, u); n = max(2, int(L / 9) + 1)
    for k in range(n): a = k / (n - 1); box(*_sub(lo, hi, u, v, max(0, a - 0.02), min(1, a + 0.02), 0.3, 0.7, 0, 1.0), wood())
    broken = "broke" in name
    for z in (0.45, 0.8):
        a1 = 0.55 if broken and z > 0.5 else 1.0
        box(*_sub(lo, hi, u, v, 0, a1, 0.4, 0.6, z - 0.06, z + 0.06), wood())

def picket(lo, hi, name):
    u, v = _axes(lo, hi); L = _span(lo, hi, u); n = max(3, int(L / 0.5))
    for k in range(n): a = k / n; box(*_sub(lo, hi, u, v, a, a + 0.6 / n, 0.4, 0.6, 0, 1.0), flat("paint_whitewood", (0.7, 0.7, 0.66), 0.7))
    for z in (0.3, 0.75): box(*_sub(lo, hi, u, v, 0, 1, 0.55, 0.75, z - 0.04, z + 0.04), flat("paint_whitewood", (0.7, 0.7, 0.66), 0.7))


# ---- Free Pack's own track-side props (fp_*), for the new free tracks ----
def plastic(rgb, name): return acg("Plastic010", 0.5, ramp=(tuple(c * 0.7 for c in rgb), tuple(rgb)), rough=0.6, name=name)

def tuffblock(lo, hi, name):
    # rotomoulded divider: tapered block with a grip slot, two-tone
    L, Wd, H = hi.x - lo.x, hi.y - lo.y, hi.z - lo.z; cy = (lo.y + hi.y) / 2
    body = plastic((0.78, 0.78, 0.76), "tuff_white")
    bm = bmesh.new()
    pts = [(-Wd / 2, 0), (Wd / 2, 0), (Wd * 0.36, H), (-Wd * 0.36, H)]
    vs = [[bm.verts.new((x, cy + y, z)) for (y, z) in pts] for x in (lo.x + 0.15, hi.x - 0.15)]
    bm.faces.new(vs[0][::-1]); bm.faces.new(vs[1])
    for i in range(4): bm.faces.new((vs[0][i], vs[0][(i + 1) % 4], vs[1][(i + 1) % 4], vs[1][i]))
    bmesh.ops.bevel(bm, geom=list(bm.edges), offset=0.12, segments=3, affect="EDGES", profile=0.5)
    _box_uv(bm, 2)
    me = bpy.data.meshes.new("tuff"); bm.to_mesh(me); bm.free()
    for f in me.polygons: f.use_smooth = True
    o = bpy.data.objects.new("tuff", me); bpy.context.scene.collection.objects.link(o); me.materials.append(body)
    band = plastic((0.16, 0.24, 0.42), "tuff_blue")
    box(Vector((lo.x + 0.4, cy - Wd * 0.47, lo.z + H * 0.2)), Vector((hi.x - 0.4, cy + Wd * 0.47, lo.z + H * 0.42)), band, bevel=0.05)
    box(Vector((lo.x + L * 0.42, cy - Wd * 0.2, hi.z - 0.35)), Vector((hi.x - L * 0.42, cy + Wd * 0.2, hi.z + 0.02)), flat("slot", (0.04, 0.04, 0.045), 0.9))

def startgate_post(lo, hi, name):
    # one bay of a drop-start gate: base rail, hinged steel plate, brand panel
    cy = (lo.y + hi.y) / 2; st = steel(); H = hi.z - lo.z
    box(Vector((lo.x, lo.y, lo.z)), Vector((hi.x, hi.y, lo.z + 0.35)), concrete(), uv_scale=3)
    box(Vector((lo.x + 0.1, cy - 0.08, lo.z + 0.35)), Vector((hi.x - 0.1, cy + 0.08, hi.z)), galv(), bevel=0.03)
    _banner_quad(Vector((lo.x + 0.2, cy - 0.1, lo.z + H * 0.35)), Vector((hi.x - 0.2, cy - 0.1, hi.z - 0.15)), brand_mat(name), -1)
    for x in (lo.x + 0.05, hi.x - 0.05): cyl((x, cy, lo.z + 0.4), (x, cy + 0.6, lo.z + 0.4), 0.12, st, 10)

def mesh_fence(lo, hi, name):
    # orange plastic barrier mesh on two steel T-posts, slight sag
    cy = (lo.y + hi.y) / 2; L = hi.x - lo.x; H = hi.z - lo.z; org = flat("mesh_orange", (0.62, 0.2, 0.04), 0.7)
    for x in (lo.x + 0.1, hi.x - 0.1): box(Vector((x - 0.06, cy - 0.06, lo.z)), Vector((x + 0.06, cy + 0.06, hi.z)), galv())
    nx, nz = int(L / 0.28), int(H / 0.2)
    for i in range(nx + 1):
        x = lo.x + L * i / nx; sag = 0.12 * math.sin(math.pi * i / nx)
        box(Vector((x - 0.03, cy + 0.07, lo.z + 0.15)), Vector((x + 0.03, cy + 0.1, hi.z - 0.1 - sag)), org)
    for k in range(nz + 1):
        z = lo.z + 0.15 + (H - 0.25) * k / nz
        box(Vector((lo.x, cy + 0.065, z - 0.025)), Vector((hi.x, cy + 0.105, z + 0.025)), org)

def tirewall(lo, hi, name):
    # stacks of car tyres bolted together, a stripe painted on the top row
    R = min((hi.y - lo.y) / 2, 1.15); h = 0.7; n = max(1, int((hi.x - lo.x) / (2 * R))); rows = max(1, int((hi.z - lo.z) / h))
    for i in range(n):
        for k in range(rows):
            c = Vector((lo.x + R + i * 2 * R, (lo.y + hi.y) / 2, lo.z + k * h + h / 2))
            m = flat("tyre_white", (0.62, 0.62, 0.6), 0.8) if (k == rows - 1 and i % 2 == 0) else tyre_mat()
            cyl(c - Vector((0, 0, h / 2 - 0.02)), c + Vector((0, 0, h / 2 - 0.02)), R * 0.98, m, 24)
            cyl(c - Vector((0, 0, h / 2)), c + Vector((0, 0, h / 2)), R * 0.5, flat("tyre_hole", (0.01, 0.01, 0.01), 0.95), 16)

def flagman_stand(lo, hi, name):
    # scaffold tower with a deck, kick boards and a ladder
    st = galv(); cz = lo.z + (hi.z - lo.z) * 0.62
    for x in (lo.x + 0.1, hi.x - 0.1):
        for y in (lo.y + 0.1, hi.y - 0.1): cyl((x, y, lo.z), (x, y, hi.z), 0.08, st, 8)
    box(Vector((lo.x, lo.y, cz - 0.15)), Vector((hi.x, hi.y, cz)), plywood(), uv_scale=3)
    for z in (cz + 1.6, hi.z - 0.05):
        for (a, b) in (((lo.x, lo.y), (hi.x, lo.y)), ((lo.x, hi.y), (hi.x, hi.y)), ((lo.x, lo.y), (lo.x, hi.y))):
            cyl((a[0], a[1], z), (b[0], b[1], z), 0.06, st, 8)
    box(Vector((lo.x, lo.y - 0.02, cz)), Vector((hi.x, lo.y + 0.03, cz + 0.5)), flat("kick_yellow", (0.62, 0.48, 0.08), 0.6))
    for k in range(int((cz - lo.z) / 1.0) + 1):
        z = lo.z + 0.5 + k; cyl((hi.x + 0.05, lo.y + 0.4, z), (hi.x + 0.05, hi.y - 0.4, z), 0.04, st, 6)
    for i in range(6):  # bracing
        cyl((lo.x + 0.1, lo.y + 0.1, lo.z + 0.2), (lo.x + 0.1, hi.y - 0.1, cz - 0.2), 0.04, st, 6)
        cyl((hi.x - 0.1, lo.y + 0.1, lo.z + 0.2), (hi.x - 0.1, hi.y - 0.1, cz - 0.2), 0.04, st, 6); break

FP_PROPS = {  # name: (game lo, game hi) in feet, y up
    "fp_haybale": ((-2, 0, -1), (2, 1.5, 1)),
    "fp_tuffblock": ((-2.5, 0, -1), (2.5, 2.6, 1)),
    "fp_startgate_post": ((-1.5, 0, -0.6), (1.5, 2.2, 0.6)),
    "fp_fence": ((-3.3, 0, -0.1), (3.3, 4.0, 0.1)),
    "fp_tirewall": ((-4.6, 0, -1.15), (4.6, 2.8, 1.15)),
    "fp_flagman_stand": ((-2, 0, -2), (2, 8, 2)),
    "fp_banner": ((-6, 0, -0.2), (6, 6, 0.2)),
}

RULES = [
    (r"^fp_haybale$", single_bale), (r"^fp_tuffblock$", tuffblock), (r"^fp_startgate_post$", startgate_post),
    (r"^fp_fence$", mesh_fence), (r"^fp_tirewall$", tirewall), (r"^fp_flagman_stand$", flagman_stand), (r"^fp_banner$", billboard),
    (r"^triplebale\d+$", triplebale), (r"^haybale$", bale_stack), (r"^haybale_", single_bale),
    (r"^(fox\d+|.*hb)$", single_bale), (r"^billboard", billboard), (r"^gianttire", giant_tyre),
    (r"^cameraplatform", camera_platform), (r"^rail$", rail), (r"^50gal$|^trash_can", drum),
    (r"^crate", crate), (r"^tent0", tent), (r"^fence_chlink(short)?$", chainlink), (r"^waypnt$", waypoint),
    (r"^podium$", podium), (r"^startgate$", startgate), (r"^retainwall", retainwall),
    (r"^watertower$", watertower), (r"^helipad$", helipad), (r"^asdome", dome),
    (r"^(300zx|4runner.*|chevelle.*|cop.*|jeep.*|mx3.*|sts.*|greyhound|schoolbus|semicab|firetruck|watertruck|tanker|dumptruck|feduptruck|companytruck|famtrkstr|truckbed01|airfield_tractor)$", vehicle),
    (r"^(trailer\d+|flatbed|semitrailer|nattrailer01|pit_trailer|haywagon)$", trailer),
    (r"^(boxcarclosed|boxcaropen|caboose|cattlecar|coalcar|engine|gondola|hopper)$", railcar),
    (r"^(biplane|supercub|beech18|dc-3|cargoplane01?|aircraftq|bellranger|helinocam)$", aircraft),
    (r"^(sign_.*|infosign.*|trpksign01|ironmountainsign)$", sign), (r"^splitrail", splitrail), (r"^fence_picket$", picket),
    (r"^(cabin|farmhouse|garage0\d|hut|office01|main_office|rangerstation.*|quonset|gasstation|radiostation|miningbldg01|station|hanger)$", building),
]

def build(name, lo_g, hi_g):
    reset(); lo, hi = B(lo_g, hi_g)
    fn = next((f for p, f in RULES if re.match(p, name)), generic)
    d = sorted(hi - lo)
    if fn is generic and d[1] < 3.0: fn = pole
    if fn is generic and d[1] > 40:
        # big buildings and stadium shells: a box would block the track, so
        # these stay out until each has its own model
        p = os.path.join(BUILD, "objects", name + ".glb")
        if os.path.exists(p): os.remove(p)
        return "missing"
    fn(lo, hi, name)
    o = join_all(name)
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    return fn.__name__

if __name__ == "__main__":
    sizes = {}
    for line in open(os.path.join(FREEPACK, "stock_sizes.tsv")):
        if line.startswith("#") or not line.strip(): continue
        n, *v = line.split("\t"); v = list(map(float, v[:6])); sizes[n] = (v[:3], v[3:])
    sizes.update(FP_PROPS)
    names = [a for a in sys.argv[1:] if not a.startswith("-")] or list(sizes)
    out = []
    for n in names:
        kind = build(n, *sizes[n]); out.append((n, kind)); print("built", n, kind, flush=True)
    with open(os.path.join(BUILD, "objects", "built.txt"), "a") as f:
        for n, k in out: f.write(f"{n}\t{k}\n")
