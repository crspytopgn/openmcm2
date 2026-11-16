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

def generic(lo, hi, name):
    # unknown prop: a tarp-covered stack of crates in the same footprint
    box(lo, hi, canvas((0.28, 0.32, 0.26)), bevel=min(hi - lo) * 0.08, uv_scale=6)
    for f in (0.3, 0.7):
        x = lo.x + (hi.x - lo.x) * f
        box(Vector((x - 0.08, lo.y - 0.03, lo.z)), Vector((x + 0.08, hi.y + 0.03, hi.z + 0.03)), flat("strap", (0.12, 0.12, 0.12), 0.8))

RULES = [
    (r"^triplebale\d+$", triplebale), (r"^haybale$", bale_stack), (r"^haybale_", single_bale),
    (r"^(fox\d+|.*hb)$", single_bale), (r"^billboard", billboard), (r"^gianttire", giant_tyre),
    (r"^cameraplatform", camera_platform), (r"^rail$", rail), (r"^50gal$|^trash_can", drum),
    (r"^crate", crate), (r"^tent0", tent), (r"^fence_chlink(short)?$", chainlink), (r"^waypnt$", waypoint),
    (r"^podium$", podium),
]

def build(name, lo_g, hi_g):
    reset(); lo, hi = B(lo_g, hi_g)
    fn = next((f for p, f in RULES if re.match(p, name)), generic)
    fn(lo, hi, name)
    o = join_all(name)
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    return fn.__name__

if __name__ == "__main__":
    sizes = {}
    for line in open(os.path.join(FREEPACK, "stock_sizes.tsv")):
        if line.startswith("#") or not line.strip(): continue
        n, *v = line.split("\t"); v = list(map(float, v[:6])); sizes[n] = (v[:3], v[3:])
    names = [a for a in sys.argv[1:] if not a.startswith("-")] or list(sizes)
    out = []
    for n in names:
        kind = build(n, *sizes[n]); out.append((n, kind)); print("built", n, kind, flush=True)
    with open(os.path.join(BUILD, "objects", "built.txt"), "a") as f:
        for n, k in out: f.write(f"{n}\t{k}\n")
