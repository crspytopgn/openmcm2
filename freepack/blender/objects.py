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

# The race-day banners (fp_banner_2 .. fp_banner_7): each its own sponsor
# in strong colour, so a track's banners aren't all one pale sign.
RACE_BRANDS = ["ridgeback", "kestrel", "granite", "bluegum", "highmoor", "slatepeak", "northfork", "redtail"]
def race_banner(lo, hi, name):
    k = int(name.rsplit("_", 1)[1])
    b = RACE_BRANDS[(k - 2) % len(RACE_BRANDS)]
    m = image_mat(os.path.join(BUILD, "brands", b + "_race.png"), "race_" + b, rough=0.8)
    top = hi.z; panel_h = min(7.0, (hi.z - max(lo.z, 0)) * 0.95); bottom = top - panel_h
    th = max(hi.y - lo.y, 0.25); y0 = lo.y
    box(Vector((lo.x, y0 + th * 0.4, bottom)), Vector((hi.x, y0 + th, top)), plywood(), uv_scale=4)
    _banner_quad(Vector((lo.x + 0.1, y0 + th * 0.4 - 0.02, bottom + 0.1)), Vector((hi.x - 0.1, y0 + th * 0.4 - 0.02, top - 0.1)), m, -1)
    _banner_quad(Vector((lo.x + 0.1, y0 + th + 0.02, bottom + 0.1)), Vector((hi.x - 0.1, y0 + th + 0.02, top - 0.1)), m, 1)
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

class Frame:
    """Local frame for a vehicle: a along its length (0 rear, 1 front),
    b across (0..1), z up as a fraction of its height. Real proportions
    are kept from the stock length; the stock footprint only centres it."""
    def __init__(self, lo, hi, u, v, wr, hr, long_is_length=True):
        self.u, self.v = u, v
        Lb, Wb, Hb = _span(lo, hi, u), _span(lo, hi, v), hi.z - lo.z
        self.L = Lb; self.W = min(Wb, wr * Lb); self.H = min(max(Hb, hr * Lb), 1.3 * Hb) if hr else Hb
        self.O = (lo + hi) / 2; self.O.z = lo.z
    def p(self, a, b, z):
        return self.O + self.u * ((a - 0.5) * self.L) + self.v * ((b - 0.5) * self.W) + Vector((0, 0, z * self.H))
    def ext(self, pts, b0, b1, mat, bevel=0.0, uv=2.0):
        """Extrude a side profile [(a, z), ...] across b0..b1."""
        bm = bmesh.new()
        r0 = [bm.verts.new(self.p(a, b0, z)) for a, z in pts]; r1 = [bm.verts.new(self.p(a, b1, z)) for a, z in pts]
        n = len(pts); bm.faces.new(r0); bm.faces.new(r1[::-1])
        for i in range(n): bm.faces.new((r0[i], r1[i], r1[(i + 1) % n], r0[(i + 1) % n]))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        if bevel > 0: bmesh.ops.bevel(bm, geom=list(bm.edges), offset=bevel, segments=2, affect="EDGES", profile=0.5, clamp_overlap=True)
        _box_uv(bm, uv)
        me = bpy.data.meshes.new("ext"); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new("ext", me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat); return o
    def decal(self, a0, a1, z0, z1, side, mat, off=0.012):
        """Picture on the b=0 (side 0) or b=1 (side 1) face, readable from outside."""
        b = -off / self.W if side == 0 else 1 + off / self.W
        bm = bmesh.new(); uv = bm.loops.layers.uv.verify()
        q = [(a0, z0), (a1, z0), (a1, z1), (a0, z1)]
        vs = [bm.verts.new(self.p(a, b, z)) for a, z in q]
        f = bm.faces.new(vs if side == 0 else vs[::-1])
        for l in f.loops:
            a, z = next((qa, qz) for (qa, qz), v in zip(q, vs) if v == l.vert)
            t = (a - a0) / (a1 - a0); l[uv].uv = (t if side == 0 else 1 - t, (z - z0) / (z1 - z0))
        if side == 0: pass
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        me = bpy.data.meshes.new("decal"); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new("decal", me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat); return o
    def bx(self, a0, a1, b0, b1, z0, z1, mat, bevel=0.0):
        return self.ext([(a0, z0), (a1, z0), (a1, z1), (a0, z1)], b0, b1, mat, bevel)
    def wheels(self, at, r, tw=None, dual=False):
        w = tw or min(0.85, self.W * 0.11); rim = flat("rim", (0.42, 0.43, 0.44), 0.4, 0.8)
        for a in at:
            for side in (0, 1):
                for k in ((0, 1) if dual else (0,)):
                    off = (w / 2 + k * w * 1.05) / self.W; b = off if side == 0 else 1 - off
                    c = self.p(a, b, 0) + Vector((0, 0, r)); c2 = c - self.v * (w / 2); c3 = c + self.v * (w / 2)
                    cyl(c2, c3, r, tyre_mat(), 24)
                    o = (-1 if side == 0 else 1) * 0.02
                    cyl(c2 + self.v * o * (0 if side else 1) - self.v * 0.02, c3 + self.v * 0.02, r * 0.58, rim, 16)

def lamp(): return flat("headlamp", (0.85, 0.85, 0.8), 0.15)
def tail(): return flat("taillamp", (0.35, 0.03, 0.02), 0.3)
def trim(): return flat("trim", (0.05, 0.05, 0.055), 0.6)
def chrome(): return flat("chrome", (0.6, 0.6, 0.6), 0.25, 1.0)

def _lights(F, zl, zt=None, inset=0.12):
    zt = zt or zl
    for b in ((inset, inset + 0.18), (1 - inset - 0.18, 1 - inset)):
        F.bx(0.995, 1.004, b[0], b[1], zl, zl + 0.07, lamp()); F.bx(-0.004, 0.005, b[0], b[1], zt, zt + 0.07, tail())

def car(F, paint, kind):
    r = 0.22 if kind != "suv" else 0.24
    F.wheels((0.18, 0.8), r * F.H)
    if kind == "sports":
        body = [(0.0, 0.18), (1.0, 0.18), (1.0, 0.38), (0.97, 0.47), (0.66, 0.55), (0.2, 0.6), (0.0, 0.55)]
        glass_ = [(0.66, 0.54), (0.5, 0.92), (0.3, 0.93), (0.12, 0.6)]; roof = [(0.49, 0.9), (0.31, 0.9), (0.31, 0.97), (0.48, 0.97)]
    elif kind == "suv":
        body = [(0.0, 0.2), (1.0, 0.2), (1.0, 0.5), (0.97, 0.58), (0.76, 0.62), (0.02, 0.62), (0.0, 0.55)]
        glass_ = [(0.76, 0.6), (0.68, 0.92), (0.04, 0.92), (0.03, 0.61)]; roof = [(0.68, 0.9), (0.03, 0.9), (0.03, 0.98), (0.67, 0.98)]
    else:
        body = [(0.0, 0.2), (1.0, 0.2), (1.0, 0.44), (0.98, 0.54), (0.72, 0.58), (0.28, 0.6), (0.02, 0.58), (0.0, 0.5)]
        glass_ = [(0.71, 0.57), (0.58, 0.93), (0.32, 0.94), (0.22, 0.6)]; roof = [(0.57, 0.91), (0.33, 0.91), (0.33, 0.98), (0.56, 0.98)]
    F.ext(body, 0.02, 0.98, paint, bevel=0.15)
    F.ext(glass_, 0.07, 0.93, glass(), bevel=0.06)
    F.ext(roof, 0.08, 0.92, paint, bevel=0.06)
    a = (glass_[1][0] + glass_[2][0]) / 2 - 0.01; F.bx(a, a + 0.03, 0.065, 0.935, 0.58, 0.92, paint)   # B pillar
    F.bx(-0.01, 0.06, 0.01, 0.99, 0.19, 0.3, trim(), 0.05); F.bx(0.94, 1.01, 0.01, 0.99, 0.19, 0.3, trim(), 0.05)  # bumpers
    F.bx(0.996, 1.003, 0.35, 0.65, 0.33, 0.42, trim())   # grille
    _lights(F, 0.38 if kind != "sports" else 0.32, 0.42)

def jeep(F, paint):
    F.wheels((0.17, 0.8), 0.27 * F.H)
    F.ext([(0.0, 0.22), (1.0, 0.22), (1.0, 0.55), (0.62, 0.58), (0.0, 0.58)], 0.03, 0.97, paint, bevel=0.1)
    F.bx(0.6, 0.63, 0.06, 0.94, 0.58, 0.85, trim()); F.bx(0.603, 0.627, 0.1, 0.9, 0.6, 0.82, glass())
    for b in ((0.05, 0.09), (0.91, 0.95)): F.bx(0.2, 0.24, b[0], b[1], 0.58, 1.0, trim())
    F.bx(0.2, 0.24, 0.05, 0.95, 0.96, 1.0, trim())
    F.bx(0.25, 0.45, 0.12, 0.88, 0.45, 0.66, flat("seats", (0.08, 0.07, 0.06), 0.8))
    F.bx(0.996, 1.004, 0.3, 0.7, 0.32, 0.52, trim()); _lights(F, 0.45, 0.4, 0.06)
    cyl(F.p(-0.02, 0.5, 0.45) - F.u * 0.4, F.p(-0.02, 0.5, 0.45), 0.28 * F.H, tyre_mat(), 20)

def pickup(F, paint):
    F.wheels((0.17, 0.8), 0.24 * F.H)
    F.ext([(0.42, 0.2), (1.0, 0.2), (1.0, 0.46), (0.98, 0.55), (0.72, 0.58), (0.42, 0.58)], 0.02, 0.98, paint, bevel=0.12)
    F.ext([(0.71, 0.57), (0.61, 0.95), (0.44, 0.95), (0.44, 0.57)], 0.07, 0.93, glass(), bevel=0.05)
    F.ext([(0.6, 0.93), (0.44, 0.93), (0.44, 1.0), (0.595, 1.0)], 0.07, 0.93, paint, bevel=0.05)
    F.bx(0.0, 0.42, 0.02, 0.98, 0.2, 0.3, paint, 0.08)          # bed floor
    for b in ((0.02, 0.07), (0.93, 0.98)): F.bx(0.0, 0.42, b[0], b[1], 0.2, 0.58, paint, 0.06)
    F.bx(0.0, 0.03, 0.02, 0.98, 0.2, 0.56, paint, 0.04); F.bx(0.405, 0.42, 0.02, 0.98, 0.2, 0.62, paint)
    F.bx(-0.01, 0.04, 0.01, 0.99, 0.18, 0.27, chrome(), 0.04); F.bx(0.95, 1.01, 0.01, 0.99, 0.18, 0.29, chrome(), 0.04)
    F.bx(0.996, 1.003, 0.25, 0.75, 0.32, 0.46, trim()); _lights(F, 0.4, 0.42, 0.06)

def cabover(F, paint, a0, z_top=1.0, tall_glass=0.62):
    """Flat-fronted truck cab from a0 to the front."""
    F.ext([(a0, 0.18), (1.0, 0.18), (1.0, z_top * 0.97), (0.99, z_top), (a0, z_top)], 0.03, 0.97, paint, bevel=0.12)
    F.bx(0.998, 1.006, 0.08, 0.92, tall_glass, z_top * 0.9, glass())
    for b in (0.025, 0.975): F.ext([(a0 + (1 - a0) * 0.45, tall_glass), (0.985, tall_glass), (0.985, z_top * 0.88), (a0 + (1 - a0) * 0.45, z_top * 0.88)], b - 0.004, b + 0.004, glass())
    F.bx(0.998, 1.006, 0.3, 0.7, 0.24, 0.45, trim()); F.bx(0.99, 1.02, 0.01, 0.99, 0.12, 0.2, chrome(), 0.03)
    for b in ((0.06, 0.18), (0.82, 0.94)): F.bx(0.999, 1.007, b[0], b[1], 0.3, 0.37, lamp())

def boxtruck(F, paint, name):
    F.wheels((0.16, 0.82), 0.16 * F.H, dual=False)
    F.bx(0.0, 0.95, 0.12, 0.88, 0.12, 0.2, trim())                  # chassis
    cabover(F, paint, 0.74, 0.72, 0.42)
    body = flat("box_white", (0.66, 0.66, 0.64), 0.6)
    F.bx(0.0, 0.72, 0.02, 0.98, 0.2, 1.0, body, 0.06)
    for b, sg in ((0.015, -1), (0.985, 1)):
        p0 = F.p(0.08, b, 0.42); p1 = F.p(0.64, b, 0.86)
        _banner_quad(Vector((min(p0.x, p1.x), min(p0.y, p1.y), p0.z)), Vector((max(p0.x, p1.x), max(p0.y, p1.y), p1.z)), brand_mat(name), sg) if False else None
    F.bx(-0.004, 0.004, 0.05, 0.95, 0.22, 0.98, flat("box_door", (0.55, 0.55, 0.53), 0.6))

def firetruck(F, paint):
    F.wheels((0.16, 0.26, 0.83), 0.17 * F.H)
    F.bx(0.0, 0.97, 0.12, 0.88, 0.12, 0.2, trim())
    cabover(F, paint, 0.72, 0.8, 0.48)
    F.bx(0.0, 0.7, 0.03, 0.97, 0.2, 0.7, paint, 0.06)
    for k in range(6):  # roller-door lockers
        a = 0.04 + k * 0.105; F.bx(a, a + 0.09, 0.025, 0.975, 0.28, 0.64, flat("locker", (0.5, 0.5, 0.5), 0.35, 0.8))
    for b in (0.3, 0.7): F.bx(0.02, 0.85, b - 0.015, b + 0.015, 0.84, 0.88, chrome())
    for k in range(18): a = 0.04 + k * 0.045; F.bx(a, a + 0.008, 0.3, 0.7, 0.845, 0.875, chrome())
    for b in (0.3, 0.7): F.bx(0.1, 0.13, b - 0.02, b + 0.02, 0.7, 0.85, chrome())
    F.bx(0.85, 0.92, 0.25, 0.75, 0.8, 0.85, flat("lightbar_red", (0.5, 0.04, 0.03), 0.3))

def tanktruck(F, paint, name):
    F.wheels((0.12, 0.24, 0.84), 0.17 * F.H, dual=True)
    F.bx(0.0, 0.97, 0.15, 0.85, 0.12, 0.22, trim())
    cabover(F, paint, 0.76, 0.72, 0.42)
    r = min(F.W * 0.46, F.H * 0.36); c0 = F.p(0.03, 0.5, 0.22) + Vector((0, 0, r)); c1 = F.p(0.73, 0.5, 0.22) + Vector((0, 0, r))
    tm = flat("tank_steel", (0.62, 0.63, 0.64), 0.35, 0.9) if "tanker" in name else flat("tank_white", (0.68, 0.68, 0.66), 0.5)
    cyl(c0, c1, r, tm, 32)
    for a in (0.08, 0.38, 0.68):
        c = F.p(a, 0.5, 0.22) + Vector((0, 0, r)); cyl(c - F.u * 0.15, c + F.u * 0.15, r * 1.02, trim(), 32)
    F.bx(0.2, 0.6, 0.45, 0.55, 0.22 + 2 * r / F.H, 0.24 + 2 * r / F.H, steel())

def semicab(F, paint):
    F.wheels((0.12, 0.28, 0.82), 0.17 * F.H, dual=True)
    F.bx(0.0, 0.97, 0.15, 0.85, 0.12, 0.22, trim())
    F.ext([(0.55, 0.18), (0.98, 0.18), (1.0, 0.36), (0.97, 0.44), (0.82, 0.48), (0.8, 0.8), (0.55, 0.8)], 0.04, 0.96, paint, bevel=0.15)   # hood + cab
    F.ext([(0.55, 0.8), (0.72, 0.8), (0.7, 0.97), (0.55, 0.97)], 0.06, 0.94, paint, bevel=0.1)   # sleeper roof
    F.ext([(0.8, 0.5), (0.86, 0.5), (0.82, 0.78), (0.8, 0.78)], 0.06, 0.94, glass())
    F.bx(0.0, 0.3, 0.3, 0.7, 0.22, 0.26, galv())                 # fifth wheel deck
    for b in (0.02, 0.98):
        c = F.p(0.78, b, 0.5); cyl(c, c + Vector((0, 0, 0.5 * F.H)), 0.1 * F.W * 0.5, chrome(), 10)  # stacks
    F.bx(0.995, 1.004, 0.3, 0.7, 0.2, 0.42, chrome()); _lights(F, 0.3, 0.3, 0.04)

def bus(F, paint, name):
    school = "school" in name
    F.wheels((0.2, 0.78) if school else (0.16, 0.26, 0.82), 0.15 * F.H)
    F.ext([(0.0, 0.12), (1.0, 0.12), (1.0, 0.55), (0.985, 0.95), (0.96, 1.0), (0.02, 1.0), (0.0, 0.95)], 0.02, 0.98, paint, bevel=0.25)
    F.ext([(0.03, 0.55), (0.9, 0.55), (0.9, 0.85), (0.03, 0.85)], 0.015, 0.985, glass())
    for k in range(13 if school else 9):
        a = 0.06 + k * (0.065 if school else 0.095); F.bx(a, a + 0.012, 0.012, 0.988, 0.55, 0.85, paint)
    F.bx(0.996, 1.004, 0.06, 0.94, 0.55, 0.92, glass()); F.bx(0.92, 0.985, 0.012, 0.03, 0.15, 0.9, glass())
    if school:
        for b in (0.012, 0.988): F.bx(0.02, 0.98, b - 0.006, b + 0.006, 0.4, 0.43, trim())
        F.bx(0.99, 1.08, 0.04, 0.96, 0.12, 0.45, paint, 0.1)     # nose
    F.bx(-0.01, 0.02, 0.0, 1.0, 0.1, 0.2, trim()); F.bx(0.99, 1.01, 0.0, 1.0, 0.1, 0.2, trim()); _lights(F, 0.3, 0.3, 0.06)

def haultruck(F, paint):
    F.wheels((0.2, 0.75), 0.26 * F.H, tw=F.W * 0.16, dual=True)
    F.bx(0.05, 0.95, 0.2, 0.8, 0.3, 0.48, steel())
    F.ext([(0.0, 0.48), (0.72, 0.48), (0.86, 1.0), (0.0, 0.92)], 0.0, 1.0, paint, bevel=0.3)    # dump body with canopy
    F.bx(0.74, 1.0, 0.04, 0.96, 0.48, 0.56, paint)               # deck
    F.bx(0.78, 0.96, 0.05, 0.4, 0.56, 0.82, paint, 0.1); F.bx(0.95, 0.965, 0.07, 0.38, 0.62, 0.78, glass())
    F.bx(0.94, 1.0, 0.3, 0.7, 0.3, 0.5, trim())
    for b in (0.43, 0.57): F.bx(0.74, 0.98, b, b + 0.01, 0.56, 0.75, galv())

def tug(F, paint):
    F.wheels((0.2, 0.8), 0.2 * F.H)
    F.bx(0.0, 1.0, 0.02, 0.98, 0.15, 0.45, paint, 0.12)
    F.bx(0.05, 0.4, 0.08, 0.92, 0.45, 0.95, trim()); F.bx(0.06, 0.39, 0.06, 0.94, 0.5, 0.9, glass())
    F.bx(0.06, 0.39, 0.08, 0.92, 0.93, 1.0, paint)

def vehicle(lo, hi, name):
    u, v = _axes(lo, hi); paint = paint_for(name)
    if "cop" in name: paint = flat("paint_cop", (0.05, 0.05, 0.06), 0.4, 0.3)
    R = [(r"greyhound|schoolbus", 0.24, 0.3, bus), (r"firetruck", 0.3, 0.3, firetruck), (r"tanker|watertruck", 0.32, 0.33, tanktruck),
         (r"semicab", 0.4, 0.5, semicab), (r"feduptruck", 0.33, 0.4, boxtruck), (r"dumptruck", 0.8, 0.45, haultruck),
         (r"airfield_tractor", 0.6, 0.55, tug), (r"companytruck|famtrkstr", 0.37, 0.32, pickup), (r"jeep", 0.45, 0.42, jeep),
         (r"4runner", 0.4, 0.37, "suv"), (r"300zx|mx3", 0.42, 0.27, "sports"), (r"", 0.4, 0.3, "sedan")]
    for pat, wr, hr, fn in R:
        if re.search(pat, name): break
    F = Frame(lo, hi, u, v, wr, hr)
    if isinstance(fn, str): car(F, paint, fn)
    elif fn in (boxtruck, tanktruck, bus): fn(F, paint, name)
    else: fn(F, paint)
    if "cop" in name:
        F.bx(0.47, 0.53, 0.25, 0.75, 0.98, 1.03, flat("lightbar", (0.2, 0.05, 0.05), 0.4))
        for b in (0.015, 0.985): F.bx(0.3, 0.7, b - 0.005, b + 0.005, 0.3, 0.55, flat("cop_white", (0.7, 0.7, 0.68), 0.4))

TRIM = [(0.42, 0.16, 0.08), (0.12, 0.2, 0.32), (0.14, 0.24, 0.16), (0.3, 0.3, 0.32), (0.45, 0.32, 0.1), (0.25, 0.12, 0.18)]
def trailer(lo, hi, name):
    u, v = _axes(lo, hi); h = sum(map(ord, name)); stripe = flat("stripe_%d" % (h % len(TRIM)), TRIM[h % len(TRIM)], 0.5)
    if "haywagon" in name:
        F = Frame(lo, hi, u, v, 0.5, None); F.wheels((0.15, 0.85), 0.11 * F.H)
        F.bx(0.0, 1.0, 0.02, 0.98, 0.18, 0.24, wood()); F.bx(0.9, 1.15, 0.47, 0.53, 0.12, 0.16, wood())
        for i in range(4):
            for k in range(2):
                for j in range(3 - k):
                    a0 = 0.03 + i * 0.24; b0 = 0.06 + j * 0.3 + k * 0.15
                    bale_block(*_sub_f(F, a0, a0 + 0.22, b0, b0 + 0.28, 0.24 + k * 0.38, 0.6 + k * 0.38))
        return
    if re.search(r"flatbed", name):
        return railcar(lo, hi, name)
    if re.search(r"semitrailer|nattrailer|pit_trailer", name):
        F = Frame(lo, hi, u, v, 0.2 if "semi" in name else 0.42, None)
        body = flat("hauler_white", (0.7, 0.7, 0.68), 0.45) if h % 2 else flat("hauler_dark", (0.1, 0.11, 0.12), 0.4, 0.3)
        F.wheels((0.08, 0.16) if "semi" in name else (0.12, 0.2), 0.09 * F.H, dual=True)
        F.bx(0.0, 1.0, 0.03, 0.97, 0.2, 1.0, body, 0.08)
        F.bx(0.0, 1.0, 0.025, 0.975, 0.2, 0.26, stripe)
        for side in (0, 1): F.decal(0.25, 0.75, 0.4, 0.9, side, brand_mat(name))
        F.bx(-0.003, 0.003, 0.05, 0.95, 0.24, 0.97, flat("hauler_doors", (0.5, 0.5, 0.5), 0.5))
        F.bx(0.78, 0.8, 0.2, 0.8, 0.0, 0.2, galv())     # landing legs
        if "pit_trailer" in name or "nattrailer" in name:   # side awning out over the pit
            F.ext([(0.1, 0.92), (0.9, 0.92), (0.9, 0.94), (0.1, 0.94)], -0.9, 0.0, canvas((0.75, 0.75, 0.72)))
            for a in (0.1, 0.5, 0.9): F.bx(a, a + 0.006, -0.9, -0.89, 0.0, 0.92, galv())
        return
    # travel trailer: rounded front, windows, door, stripe, awning
    F = Frame(lo, hi, u, v, 0.32, None); F.wheels((0.42,) if F.L < 30 else (0.38, 0.48), 0.1 * F.H)
    shell = flat("rv_cream", (0.72, 0.7, 0.64), 0.45)
    F.ext([(0.0, 0.15), (0.86, 0.15), (0.93, 0.25), (0.97, 0.55), (0.95, 0.85), (0.88, 0.97), (0.02, 0.97), (0.0, 0.9)], 0.02, 0.98, shell, bevel=0.2)
    for b0, b1 in ((0.015, 0.985),):
        F.ext([(0.0, 0.35), (0.92, 0.35), (0.945, 0.42), (0.0, 0.42)], b0, b1, stripe)
    F.ext([(0.88, 0.55), (0.955, 0.6), (0.94, 0.8), (0.88, 0.82)], 0.15, 0.85, glass())
    for side in (0, 1):
        for a0 in ((0.1, 0.55) if side else (0.08, 0.32, 0.6)):
            F.bx(a0, a0 + 0.14, -0.003 if side == 0 else 0.985, 0.015 if side == 0 else 1.003, 0.55, 0.8, glass())
    F.bx(0.45, 0.53, -0.004, 0.02, 0.18, 0.85, flat("rv_door", (0.6, 0.58, 0.52), 0.5))
    F.bx(0.95, 1.12, 0.47, 0.53, 0.12, 0.17, steel()); F.bx(1.0, 1.02, 0.48, 0.52, 0.0, 0.17, steel())
    F.bx(0.95, 1.03, 0.3, 0.7, 0.2, 0.32, flat("propane", (0.7, 0.7, 0.68), 0.4))
    if h % 3 == 0:
        F.ext([(0.12, 0.9), (0.72, 0.9), (0.72, 0.92), (0.12, 0.92)], -0.45, 0.0, canvas(TRIM[(h + 2) % len(TRIM)]))
        for a in (0.12, 0.72): F.bx(a, a + 0.008, -0.45, -0.44, 0.0, 0.9, galv())

def _sub_f(F, a0, a1, b0, b1, z0, z1):
    p0 = F.p(a0, b0, z0); p1 = F.p(a1, b1, z1)
    return Vector((min(p0.x, p1.x), min(p0.y, p1.y), p0.z)), Vector((max(p0.x, p1.x), max(p0.y, p1.y), p1.z))

def rail_trucks(F, at=(0.14, 0.86)):
    r = 0.07 * F.H if F.H > 10 else 1.4; w = acg("Rust004", 0.3, ramp=((0.04, 0.03, 0.025), (0.16, 0.1, 0.07)), rough=0.6, name="rail_wheel")
    for a in at:
        F.bx(a - 0.07, a + 0.07, 0.08, 0.92, r * 0.6 / F.H, r * 1.9 / F.H, trim())
        for d in (-0.04, 0.04):
            for b in (0.1, 0.9):
                c = F.p(a + d, b, 0) + Vector((0, 0, r)); cyl(c - F.v * 0.25, c + F.v * 0.25, r, w, 16)
    return r * 2.1 / F.H

def railcar(lo, hi, name):
    u, v = _axes(lo, hi)
    if name == "gondola":   # ski-lift cabin hanging on its grip arm
        F = Frame(lo, hi, u, v, 1.0, None); body = acg("PaintedMetal004", 0.25, ramp=((0.36, 0.06, 0.045), (0.46, 0.09, 0.065)), rough=0.45, name="cabin_red")
        F.ext([(0.05, 0.0), (0.95, 0.0), (1.0, 0.18), (0.98, 0.5), (0.9, 0.55), (0.1, 0.55), (0.02, 0.5), (0.0, 0.18)], 0.05, 0.95, body, bevel=0.2)
        F.ext([(0.06, 0.25), (0.94, 0.25), (0.92, 0.48), (0.08, 0.48)], 0.04, 0.96, glass())
        F.bx(0.47, 0.53, 0.47, 0.53, 0.55, 0.98, galv()); F.bx(0.3, 0.7, 0.45, 0.55, 0.95, 1.0, galv()); return
    if name == "hopper":    # gravel hopper bin on legs (pit/quarry)
        F = Frame(lo, hi, u, v, 1.0, None); m = acg("Rust004", 0.12, ramp=((0.22, 0.17, 0.05), (0.55, 0.45, 0.14)), rough=0.7, name="hopper_paint")
        F.ext([(0.0, 0.55), (1.0, 0.55), (1.0, 1.0), (0.0, 1.0)], 0.0, 1.0, m, bevel=0.05)
        F.ext([(0.3, 0.3), (0.7, 0.3), (1.0, 0.55), (0.0, 0.55)], 0.15, 0.85, m)
        for a in (0.02, 0.92):
            for b in (0.02, 0.92): F.bx(a, a + 0.06, b, b + 0.06, 0.0, 0.56, steel())
        F.bx(0.04, 0.96, 0.06, 0.94, 0.96, 0.99, acg("Gravel022", 0.2, name="gravel_top")); return
    F = Frame(lo, hi, u, v, 0.23 if name != "engine" else 0.17, None)
    hue = sum(map(ord, name)) % 3
    rust = acg("Rust004", 0.08, ramp=((0.09, 0.03, 0.018), (0.34, 0.12, 0.06)), rough=0.7, name="rail_red") if hue != 1 else acg("PaintedMetal004", 0.12, ramp=((0.05, 0.07, 0.09), (0.16, 0.21, 0.26)), rough=0.6, name="rail_blue")
    zd = rail_trucks(F)
    F.bx(0.0, 1.0, 0.06, 0.94, zd, zd + 0.06, trim())   # underframe
    for a in (-0.01, 1.0): F.bx(a, a + 0.01, 0.4, 0.6, zd + 0.01, zd + 0.06, galv())  # couplers
    z0 = zd + 0.06
    if name in ("boxcarclosed", "boxcaropen", "cattlecar"):
        F.bx(0.005, 0.995, 0.02, 0.98, z0, 0.96, rust, 0.05)
        F.bx(0.01, 0.99, 0.08, 0.92, 0.96, 1.0, rust, 0.08)
        for k in range(1, 12):
            a = k / 12; F.bx(a - 0.004, a + 0.004, 0.012, 0.988, z0, 0.96, rust)
        if name == "cattlecar":
            for k in range(6): F.bx(0.04, 0.96, -0.004, 1.004, z0 + 0.12 + k * 0.1, z0 + 0.16 + k * 0.1, flat("slot", (0.03, 0.03, 0.03), 0.9))
        else:
            dark = flat("boxcar_inside", (0.02, 0.02, 0.02), 0.9)
            for side in (0, 1):
                if name == "boxcaropen": F.bx(0.42, 0.58, -0.004 if side == 0 else 0.98, 0.02 if side == 0 else 1.004, z0 + 0.02, 0.88, dark)
                F.bx(0.42 if name != "boxcaropen" else 0.6, 0.58 if name != "boxcaropen" else 0.76, -0.012 if side == 0 else 0.996, 0.004 if side == 0 else 1.012, z0 + 0.02, 0.9, rust)
        F.bx(0.02, 0.98, 0.45, 0.55, 1.0, 1.02, galv())
    elif name == "coalcar":
        F.bx(0.005, 0.995, 0.02, 0.98, z0, 0.9, rust, 0.05)
        for k in range(1, 10): F.bx(k / 10 - 0.006, k / 10 + 0.006, 0.01, 0.99, z0, 0.9, rust)
        for k in range(4):
            a = 0.08 + k * 0.23; F.ext([(a, 0.9), (a + 0.08, 0.98), (a + 0.16, 0.98), (a + 0.22, 0.9)], 0.05, 0.95, flat("coal", (0.03, 0.03, 0.03), 0.9))
    elif name == "caboose":
        red = flat("caboose_red", (0.38, 0.07, 0.05), 0.5)
        F.bx(0.1, 0.9, 0.04, 0.96, z0, 0.78, red, 0.05); F.bx(0.08, 0.92, 0.02, 0.98, 0.78, 0.82, red, 0.05)
        F.bx(0.38, 0.62, 0.1, 0.9, 0.82, 0.98, red, 0.04); F.bx(0.36, 0.64, 0.08, 0.92, 0.98, 1.0, red)
        for b in (0.0, 0.96):
            for a in (0.2, 0.32, 0.68, 0.8): F.bx(a, a + 0.06, b - 0.004, b + 0.044, 0.5, 0.68, glass())
            F.bx(0.4, 0.6, b - 0.004, b + 0.044, 0.86, 0.95, glass())
        for a0, a1 in ((0.0, 0.1), (0.9, 1.0)):
            F.bx(a0, a1, 0.04, 0.96, z0, z0 + 0.02, steel())
            for b in (0.04, 0.94): F.bx(a0, a1, b, b + 0.02, z0 + 0.25, z0 + 0.27, flat("rail_yellow", (0.6, 0.48, 0.08), 0.5))
    elif name == "engine":
        loco = flat("loco_paint", PAINT["navy"] if hue else (0.12, 0.2, 0.14), 0.45, 0.2); yel = flat("rail_yellow", (0.6, 0.48, 0.08), 0.5)
        F.bx(0.0, 1.0, 0.0, 1.0, z0, z0 + 0.025, steel())
        F.bx(0.04, 0.74, 0.18, 0.82, z0 + 0.025, 0.82, loco, 0.06)            # long hood
        F.bx(0.74, 0.88, 0.04, 0.96, z0 + 0.025, 1.0, loco, 0.06)            # cab
        F.ext([(0.88, z0 + 0.025), (0.99, z0 + 0.025), (0.99, 0.62), (0.88, 0.66)], 0.2, 0.8, loco, bevel=0.05)  # short nose
        F.bx(0.875, 0.885, 0.1, 0.9, 0.74, 0.94, glass())
        for b in (0.0, 0.96):
            F.bx(0.77, 0.85, b - 0.004, b + 0.044, 0.72, 0.92, glass())
            F.bx(0.04, 0.99, b + 0.02 if b else 0.0, b + 0.04 if b else 0.02, z0 + 0.19, z0 + 0.21, yel)
        F.bx(0.3, 0.6, 0.15, 0.85, zd - 0.14, zd, trim())                    # fuel tank
        for a in (0.1, 0.35, 0.6):
            F.bx(a, a + 0.06, 0.4, 0.6, 0.82, 0.86, galv())
        F.bx(0.04, 0.99, 0.0, 1.0, 0.42, 0.44, yel)
        F.bx(0.99, 1.0, 0.3, 0.7, 0.55, 0.6, lamp())
    else:   # rail flatcar with stakes and a load of lumber
        F.bx(0.0, 1.0, 0.03, 0.97, z0, z0 + 0.06, wood())
        for k in range(10):
            for b in (0.03, 0.95): F.bx(0.05 + k * 0.1, 0.07 + k * 0.1, b, b + 0.02, z0 + 0.06, z0 + 0.3, steel())
        F.bx(0.15, 0.85, 0.1, 0.9, z0 + 0.06, z0 + 0.4, acg("Wood049", 0.25, name="lumber"))

def loft(F, secs, mat, seg=20):
    """Smooth body through elliptical sections [(a, half_width_ft, half_height_ft, centre_z_frac)]."""
    bm = bmesh.new(); rings = []
    for a, rw, rh, zc in secs:
        c = F.p(a, 0.5, zc); rings.append([bm.verts.new(c + F.v * (rw * math.cos(t)) + Vector((0, 0, rh * math.sin(t))))
                                           for t in (2 * math.pi * k / seg for k in range(seg))])
    for r0, r1 in zip(rings, rings[1:]):
        for k in range(seg): bm.faces.new((r0[k], r0[(k + 1) % seg], r1[(k + 1) % seg], r1[k]))
    bm.faces.new(rings[0][::-1]); bm.faces.new(rings[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces); _box_uv(bm, 4)
    me = bpy.data.meshes.new("loft"); bm.to_mesh(me); bm.free()
    for p_ in me.polygons: p_.use_smooth = True
    o = bpy.data.objects.new("loft", me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat); return o

def airfoil(a0, a1, z, t):
    """Side profile of a wing: chord a0 (trailing) to a1 (leading), thickness t (fraction of H)."""
    c = a1 - a0
    return [(a0, z), (a0 + c * 0.5, z - t * 0.15), (a1 - c * 0.08, z - t * 0.1), (a1, z + t * 0.3), (a1 - c * 0.1, z + t * 0.9), (a0 + c * 0.6, z + t), (a0 + c * 0.05, z + t * 0.3)]

def prop(F, a, b, z, r, blades=3):
    c = F.p(a, b, z); dark = flat("prop", (0.05, 0.05, 0.05), 0.5)
    cyl(c - F.u * 0.3, c + F.u * 0.6, r * 0.12, flat("spinner", (0.5, 0.5, 0.52), 0.3, 0.8), 12, r2=r * 0.02)
    for k in range(blades):
        t = 2 * math.pi * k / blades + 0.4; d = (F.v * math.cos(t) + Vector((0, 0, math.sin(t))))
        cyl(c, c + d * r, r * 0.06, dark, 6, r2=r * 0.035)

def aircraft(lo, hi, name):
    heli = re.search(r"bell|heli", name)
    u, v = _axes(lo, hi, long_is_length=bool(heli))
    paint = paint_for(name); lt = flat("aircraft_white", (0.7, 0.7, 0.68), 0.45)
    if heli:
        F = Frame(lo, hi, u, v, 0.3, None); L, H = F.L, F.H
        body = paint if "bell" in name else lt
        loft(F, [(0.36, 0.2, 0.2, 0.3), (0.38, 2.1, 2.4, 0.3), (0.5, 2.6, 3.0, 0.3), (0.62, 2.5, 2.9, 0.3), (0.7, 2.0, 2.3, 0.28), (0.74, 0.6, 0.9, 0.26)], body)
        loft(F, [(0.6, 1.9, 2.0, 0.32), (0.66, 2.4, 2.5, 0.32), (0.72, 1.4, 1.9, 0.3), (0.745, 0.3, 0.5, 0.28)], glass())
        loft(F, [(0.02, 0.25, 0.35, 0.36), (0.4, 0.6, 0.8, 0.34)], body)                        # tail boom
        F.ext([(0.0, 0.35), (0.06, 0.38), (0.04, 0.62), (0.0, 0.6)], 0.49, 0.51, body)           # fin
        F.ext([(0.07, 0.34), (0.12, 0.34), (0.12, 0.37), (0.07, 0.37)], 0.42, 0.58, body)         # stabiliser
        for side in (0.38, 0.62):
            F.bx(0.34, 0.74, side - 0.006, side + 0.006, 0.0, 0.025, galv())
            for a in (0.42, 0.66): F.bx(a, a + 0.012, side - 0.004, side + 0.004, 0.0, 0.13, galv())
        for a in (0.42, 0.66): F.bx(a, a + 0.012, 0.38, 0.62, 0.12, 0.14, galv())
        hub = F.p(0.52, 0.5, 0.0) + Vector((0, 0, H * 0.62)); cyl(hub - Vector((0, 0, H * 0.06)), hub + Vector((0, 0, 0.6)), 0.35, galv(), 10)
        R = min(L * 0.36, 17.0); dark = flat("rotor", (0.06, 0.06, 0.06), 0.6)
        for t in (0.3, 0.3 + math.pi):
            d = F.u * math.cos(t) + F.v * math.sin(t); n = Vector((0, 0, 1)).cross(d).normalized()
            box_pts = [hub + n * 0.6 + Vector((0, 0, 0.5)), hub + d * R + n * 0.6 + Vector((0, 0, 0.5))]
            cyl(hub + Vector((0, 0, 0.55)), hub + d * R + Vector((0, 0, 0.45)), 0.45, dark, 4)
        tr = F.p(0.02, 0.53, 0.5); cyl(tr, tr + F.v * 0.2, 2.6, flat("tailrotor", (0.08, 0.08, 0.08), 0.6), 16)
        return
    F = Frame(lo, hi, u, v, 10.0, None); L, W, H = F.L, F.W, F.H
    high = re.search(r"supercub|cargoplane", name); bi = "biplane" in name
    twin = re.search(r"dc-3|beech18|aircraftq", name); four = "cargoplane" in name
    r = L * (0.06 if twin else 0.075 if four else 0.085); zc = (0.42 if high else 0.36) if not four else 0.35
    zr = zc * H
    body = lt if re.search(r"dc-3|cargoplane|aircraftq", name) else paint
    secs = [(0.0, r * 0.12, r * 0.25, zc + 0.08), (0.2, r * 0.45, r * 0.6, zc + 0.03), (0.5, r, r, zc), (0.75, r, r, zc), (0.9, r * 0.85, r * 0.85, zc - 0.01), (0.99, r * 0.35, r * 0.4, zc - 0.03)]
    loft(F, secs, body)
    F.ext([(0.86, zc + r * 0.35 / H), (0.92, zc + r * 0.35 / H), (0.885, zc + r * 0.85 / H), (0.86, zc + r * 0.85 / H)], 0.5 - r * 0.7 / W, 0.5 + r * 0.7 / W, glass())   # cockpit
    t = min(r * 0.45, 0.1 * 0.18 * L) / H
    wz = (zc + r * 0.75 / H) if high else (zc - r * 0.55 / H)
    F.ext(airfoil(0.52, 0.7, wz, t), 0.0, 1.0, body, uv=6)
    if bi: F.ext(airfoil(0.55, 0.72, zc + r * 2.2 / H, t), 0.02, 0.98, body, uv=6)
    if bi:
        for b in (0.2, 0.8, 0.35, 0.65): F.bx(0.6, 0.61, b - 0.004, b + 0.004, wz, zc + r * 2.2 / H, galv())
    F.ext(airfoil(0.0, 0.1, zc + 0.03, t * 0.6), 0.5 - 0.16, 0.5 + 0.16, body)                                  # tailplane
    F.ext([(0.0, zc + 0.05), (0.12, zc + 0.05), (0.04, 0.97), (0.0, 0.97)], 0.5 - 0.15 * r / W, 0.5 + 0.15 * r / W, body)   # fin
    gear_r = max(1.0, H * 0.07)
    if twin or four:
        for b in ((0.3, 0.7) if twin else (0.2, 0.36, 0.64, 0.8)):
            loft(F, [(0.5, r * 0.15, r * 0.15, wz), (0.58, r * 0.38, r * 0.42, wz), (0.72, r * 0.38, r * 0.42, wz), (0.78, r * 0.3, r * 0.3, wz)], body) if False else None
            c = F.p(0.0, b, 0.0)
            nac = [(0.48, r * 0.1, r * 0.1, wz), (0.55, r * 0.36, r * 0.4, wz), (0.72, r * 0.38, r * 0.42, wz), (0.76, r * 0.32, r * 0.34, wz)]
            Fb = Frame.__new__(Frame); Fb.__dict__.update(F.__dict__); Fb.O = F.p(0.5, b, 0) ; Fb.O.z = F.O.z
            loft(Fb, nac, body)
            prop(Fb, 0.765, 0.5, wz, min(r * 1.4, (W * 0.16)))
            if not four: _gear(Fb, 0.62, wz, gear_r)
    else:
        prop(F, 0.995, 0.5, zc - 0.03, r * 1.5)
        for b in (0.5 - 1.6 * r / W, 0.5 + 1.6 * r / W):
            Fb = Frame.__new__(Frame); Fb.__dict__.update(F.__dict__); Fb.O = F.p(0.5, b, 0); Fb.O.z = F.O.z
            _gear(Fb, 0.8, zc - r * 0.8 / H, gear_r)
    if four:
        for b in (0.5 - 0.8 * r / W, 0.5 + 0.8 * r / W):
            Fb = Frame.__new__(Frame); Fb.__dict__.update(F.__dict__); Fb.O = F.p(0.5, b, 0); Fb.O.z = F.O.z
            _gear(Fb, 0.55, zc - r * 0.8 / H, gear_r); _gear(Fb, 0.65, zc - r * 0.8 / H, gear_r)
    c = F.p(0.03, 0.5, 0) + Vector((0, 0, gear_r * 0.5)); cyl(c - F.v * 0.2, c + F.v * 0.2, gear_r * 0.5, tyre_mat(), 12)

def _gear(F, a, ztop, r):
    c = F.p(a, 0.5, 0) + Vector((0, 0, r)); top = F.p(a, 0.5, ztop)
    cyl(c, top, r * 0.15, galv(), 8); cyl(c - F.v * (r * 0.35), c + F.v * (r * 0.35), r, tyre_mat(), 18)

SIGN_FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
def _sign_png(name):
    """Plain road-sign faces in the common US highway style (our own drawing)."""
    from PIL import Image, ImageDraw, ImageFont
    path = os.path.join(BUILD, "tex", "sign_" + name + ".png")
    if os.path.exists(path): return path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    F = lambda s: ImageFont.truetype(SIGN_FONT, s)
    def centred(d, y, txt, size, fill):
        f = F(size); w = d.textlength(txt, font=f); d.text(((d.im.size[0] - w) / 2, y), txt, font=f, fill=fill)
    yel, blk, wht, grn = (212, 170, 30), (18, 18, 18), (232, 232, 226), (24, 92, 52)
    if re.search(r"narrowbrdg|intrsctn|roadends|worldends", name):
        im = Image.new("RGB", (512, 512), (120, 120, 118)); d = ImageDraw.Draw(im)
        d.polygon([(256, 6), (506, 256), (256, 506), (6, 256)], fill=blk); d.polygon([(256, 22), (490, 256), (256, 490), (22, 256)], fill=yel)
        if "narrow" in name:
            centred(d, 170, "NARROW", 62, blk); centred(d, 250, "BRIDGE", 62, blk)
        elif "intrsctn" in name:
            d.rectangle([236, 120, 276, 392], fill=blk); d.rectangle([120, 236, 392, 276], fill=blk)
        elif "roadends" in name:
            centred(d, 170, "ROAD", 70, blk); centred(d, 255, "ENDS", 70, blk)
        else:
            centred(d, 150, "END OF", 54, blk); centred(d, 215, "THE", 54, blk); centred(d, 280, "WORLD", 54, blk)
    elif "speedlmt" in name:
        im = Image.new("RGB", (400, 512), blk); d = ImageDraw.Draw(im); d.rectangle([10, 10, 389, 501], fill=wht)
        centred(d, 40, "SPEED", 72, blk); centred(d, 120, "LIMIT", 72, blk); centred(d, 230, "45", 200, blk)
    elif "hwy" in name:
        im = Image.new("RGB", (400, 512), blk); d = ImageDraw.Draw(im); d.rectangle([8, 8, 391, 503], fill=wht)
        d.polygon([(40, 60), (360, 60), (360, 300), (200, 470), (40, 300)], fill=blk)
        d.polygon([(56, 76), (344, 76), (344, 292), (200, 446), (56, 292)], fill=wht); centred(d, 150, "70", 170, blk)
    elif "milemrkr" in name:
        im = Image.new("RGB", (128, 512), grn); d = ImageDraw.Draw(im); d.rectangle([6, 6, 121, 505], outline=wht, width=5)
        centred(d, 60, "M", 60, wht); centred(d, 150, "I", 60, wht); centred(d, 240, "L", 60, wht); centred(d, 330, "E", 60, wht); centred(d, 420, "8", 70, wht)
    else:
        return None
    im.save(path); return path

def sign(lo, hi, name):
    u, v = _axes(lo, hi); L = _span(lo, hi, u); Wd = _span(lo, hi, v)
    png = _sign_png(name)
    if png:   # road sign: galvanised U-post with the face on top
        mid = lo + u * (L / 2) + v * (Wd / 2)
        face_h = {"milemrkr": 2.4, "speedlmt": L * 1.25, "hwy": L * 1.25}.get(next((k for k in ("milemrkr", "speedlmt", "hwy") if k in name), ""), L)
        z1 = hi.z; z0 = z1 - face_h
        cyl(Vector((mid.x, mid.y, lo.z)) + v * 0.12, Vector((mid.x, mid.y, z1 - 0.2)) + v * 0.12, 0.09, galv(), 8)
        m = image_mat(png, "signface_" + name, 0.5)
        bm = bmesh.new(); uvl = bm.loops.layers.uv.verify()
        p0 = lo + v * (Wd / 2); q = [(0, z0), (L, z0), (L, z1), (0, z1)]
        if re.search(r"narrowbrdg|intrsctn|roadends|worldends", name):   # diamond: cut the face to shape (no alpha in the pack's JPEGs)
            q = [(L / 2, z0), (L, z0 + face_h / 2), (L / 2, z1), (0, z0 + face_h / 2)]
        vs = [bm.verts.new(p0 + u * a + Vector((0, 0, z - p0.z))) for a, z in q]
        f = bm.faces.new(vs)
        for l, (a, z) in zip(f.loops, q): l[uvl].uv = (a / L, (z - z0) / face_h)
        me = bpy.data.meshes.new("face"); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new("face", me); bpy.context.scene.collection.objects.link(o); me.materials.append(m)
        sd = o.modifiers.new("s", "SOLIDIFY"); sd.thickness = 0.06; sd.offset = 1.0
        return
    # trailhead / park board: timber frame, roof, brand panel both sides
    F = Frame(lo, hi, u, v, 1.0, None); w = wood(); z0 = max(0.0, -lo.z / F.H)
    for a in (0.04, 0.9): F.bx(a, a + 0.06, 0.3, 0.7, 0.0, 0.92, w)
    F.bx(0.08, 0.92, 0.42, 0.58, 0.35, 0.82, plywood())
    for side in (0, 1):
        Fb = Frame.__new__(Frame); Fb.__dict__.update(F.__dict__); Fb.W = F.W * 0.16
        Fb.decal(0.1, 0.9, 0.38, 0.8, side, brand_mat(name))
    if F.H > 8:
        F.ext([(0.0, 0.88), (1.0, 0.88), (1.0, 0.9), (0.0, 0.9)], -0.1, 1.1, roofing())

def splitrail(lo, hi, name):
    u, v = _axes(lo, hi); L = _span(lo, hi, u); Wd = _span(lo, hi, v); g = max(lo.z, 0.0) if lo.z > -0.5 else 0.0
    top = hi.z; H = top - g; c = lo + v * (Wd / 2); w = wood()
    posts = [0.03, 0.97] if L > 3 else [0.5]
    for a in posts:
        p = c + u * (L * a); cyl(Vector((p.x, p.y, lo.z)), Vector((p.x, p.y, top)), min(0.35, Wd * 0.3), w, 10)
    if L <= 3: return
    broken = "broke" in name
    for k, z in enumerate((0.3, 0.58, 0.85)):
        zz = g + H * z; a1 = 1.0
        if broken and k == (sum(map(ord, name)) % 3): a1 = 0.55
        p0 = c + u * 0.0 + Vector((0, 0, zz - c.z)); p1 = c + u * (L * a1) + Vector((0, 0, zz - c.z + (0.6 if a1 < 1 else 0)))
        cyl(p0 + v * 0.25 * (-1) ** k, p1 + v * 0.25 * (-1) ** k, 0.22, w, 7)

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
    **{f"fp_banner_{k}": ((-6, 0, -0.2), (6, 6, 0.2)) for k in range(2, 8)},
}

RULES = [
    (r"^fp_haybale$", single_bale), (r"^fp_tuffblock$", tuffblock), (r"^fp_startgate_post$", startgate_post),
    (r"^fp_fence$", mesh_fence), (r"^fp_tirewall$", tirewall), (r"^fp_flagman_stand$", flagman_stand), (r"^fp_banner$", billboard), (r"^fp_banner_\d$", race_banner),
    (r"^triplebale\d+$", triplebale), (r"^haybale$", bale_stack), (r"^haybale_", single_bale),
    (r"^(fox\d+|.*hb)$", single_bale), (r"^billboard", billboard), (r"^gianttire", giant_tyre),
    (r"^cameraplatform", camera_platform), (r"^rail$", rail), (r"^50gal$|^trash_can", drum),
    (r"^crate", crate), (r"^tent0", tent), (r"^fence_chlink(short)?$", chainlink), (r"^waypnt$", waypoint),
    (r"^podium$", podium), (r"^startgate$", startgate), (r"^retainwall", retainwall),
    (r"^watertower$", watertower), (r"^helipad$", helipad), (r"^asdome", dome),
    (r"^(300zx|4runner.*|chevelle.*|cop.*|jeep.*|mx3.*|sts.*|greyhound|schoolbus|semicab|firetruck|watertruck|tanker|dumptruck|feduptruck|companytruck|famtrkstr|truckbed01|airfield_tractor)$", vehicle),
    (r"^(trailer\d+|flatbed|semitrailer|nattrailer01|pit_trailer|haywagon)$", trailer),
    (r"^(boxcarclosed|boxcaropen|caboose|cattlecar|coalcar|engine|gondola|hopper)$", railcar),
    (r"^(biplane|supercub|beech18|dc-3|cargoplane(01)?|aircraftq|bellranger|helinocam)$", aircraft),
    (r"^(sign_.*|infosign.*|trpksign01|ironmountainsign)$", sign), (r"^splitrail", splitrail), (r"^fence_picket$", picket),
    (r"^(cabin|farmhouse|garage0\d|hut|office01|main_office|rangerstation.*|quonset|gasstation|radiostation|miningbldg01|station|hanger)$", building),
]

def build(name, lo_g, hi_g):
    from external import KIND
    if name in KIND: return KIND[name]          # imported or stadium shell, made by external.py
    reset(); lo, hi = B(lo_g, hi_g)
    from structures import STRUCT
    fn = next((f for p, f in STRUCT if re.match(p, name)), None) or next((f for p, f in RULES if re.match(p, name)), generic)
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
    kind = getattr(fn, "__name__", "struct")
    kind = "struct" if kind == "<lambda>" else kind
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    return kind

if __name__ == "__main__":
    sizes = {}
    for line in open(os.path.join(FREEPACK, "stock_sizes.tsv")):
        if line.startswith("#") or not line.strip(): continue
        n, *v = line.split("\t"); v = list(map(float, v[:6])); sizes[n] = (v[:3], v[3:])
    sizes.update(FP_PROPS)
    names = [a for a in sys.argv[1:] if not a.startswith("-")] or list(sizes)
    out = []
    for n in names:
        try:
            kind = build(n, *sizes[n])
        except Exception as e:
            import traceback; traceback.print_exc(); print("FAILED", n, e, flush=True); continue
        out.append((n, kind)); print("built", n, kind, flush=True)
    with open(os.path.join(BUILD, "objects", "built.txt"), "a") as f:
        for n, k in out: f.write(f"{n}\t{k}\n")
