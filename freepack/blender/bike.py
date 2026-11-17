"""Free Pack dirt bikes. Our own procedural models (nothing traced from
MM2), built to the engine's bike frame tree so the game's motions drive
them: Bike, HandleBars, KickStart, NullFrame, SwingArm, Shocks,
TireFront, TireRear, each a rigid part with its origin at its pivot.

    python blender/bike.py            # all models + liveries
    python blender/bike.py two250     # one model

Writes build/bikes/fp_bike_<model>.glb and build/bikes/fp_livery_<name>.png.
Every model shares one UV layout, so any livery fits any bike. Only the
material named "livery" takes the livery picture.

Modelling space here is metres, X forward, Z up, Y left; the export step
scales and turns it into the game frame (feet, x right, y up, z forward).
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector, Matrix
from common import reset, BUILD

OUT = os.path.join(BUILD, "bikes"); os.makedirs(OUT, exist_ok=True)
K = 3.355                     # metres -> feet, sets the 4.97 ft wheelbase
def to_game(p):               # bike metres -> Blender build coords (X=game x, Y=game z, Z=game y)
    return Vector((-p[1] * K, p[0] * K - 2.657, p[2] * K - 0.042))
def from_game(x, y, z):       # game feet -> bike metres
    return Vector(((z + 2.657) / K, -x / K, (y + 0.042) / K))

PIV = {"Bike": (0, 0, 0), "HandleBars": (0, 3.701, 0.950), "KickStart": (0.381, 1.592, -0.816), "NullFrame": (0, 2.5, -0.333),
       "SwingArm": (0, 1.619, -0.731), "Shocks": (0, 3.701, 0.950), "TireFront": (0, 1.151, 2.308), "TireRear": (0, 1.115, -2.657)}
ORDER = ["Bike", "HandleBars", "KickStart", "NullFrame", "SwingArm", "Shocks", "TireFront", "TireRear"]
PARENT = {"HandleBars": "Bike", "KickStart": "Bike", "NullFrame": "Bike", "SwingArm": "Bike", "Shocks": "HandleBars", "TireFront": "Shocks", "TireRear": "SwingArm"}
RA = from_game(*PIV["TireRear"]); FA = from_game(*PIV["TireFront"]); HP = from_game(*PIV["HandleBars"]); SP = from_game(*PIV["SwingArm"])
KP = from_game(*PIV["KickStart"])

# ---------------- livery atlas ----------------
A = 1024
REG = {"shroud": (0, 0, 512, 256), "side": (512, 0, 1024, 256), "plate": (0, 256, 256, 512), "ffender": (256, 256, 512, 512),
       "rfender": (512, 256, 768, 512), "seat": (768, 256, 1024, 512)}
CELL = {"main": 0, "accent": 1, "frame": 2, "seatbase": 3, "rim": 4, "spring": 5, "black": 6, "hub": 7, "plate": 8, "tank": 9}
def cell_uv(name):
    i = CELL[name]; x = (i % 8) * 128 + 64; y = 512 + (i // 8) * 128 + 64
    return (x / A, 1 - y / A)
def reg_uv(name, s, t):
    x0, y0, x1, y1 = REG[name]; pad = 6
    return ((x0 + pad + s * (x1 - x0 - 2 * pad)) / A, 1 - (y0 + pad + (1 - t) * (y1 - y0 - 2 * pad)) / A)

LIVERIES = {   # name: main, accent, frame, rim, spring, plate bg, number ink, number
    "lime":   ((0.64, 0.76, 0.29), (0.07, 0.07, 0.08), (0.12, 0.12, 0.13), (0.06, 0.06, 0.07), (0.64, 0.76, 0.29), (0.07, 0.07, 0.08), (0.64, 0.76, 0.29), "27"),
    "blue":   ((0.12, 0.25, 0.52), (0.85, 0.85, 0.82), (0.16, 0.22, 0.42), (0.06, 0.06, 0.07), (0.85, 0.85, 0.82), (0.85, 0.85, 0.82), (0.12, 0.25, 0.52), "14"),
    "red":    ((0.62, 0.1, 0.08), (0.86, 0.86, 0.84), (0.6, 0.6, 0.62), (0.08, 0.08, 0.09), (0.62, 0.1, 0.08), (0.86, 0.86, 0.84), (0.12, 0.12, 0.12), "3"),
    "orange": ((0.85, 0.36, 0.05), (0.06, 0.06, 0.07), (0.85, 0.36, 0.05), (0.06, 0.06, 0.07), (0.85, 0.36, 0.05), (0.88, 0.88, 0.86), (0.06, 0.06, 0.07), "71"),
    "yellow": ((0.86, 0.68, 0.06), (0.1, 0.18, 0.4), (0.12, 0.18, 0.4), (0.1, 0.18, 0.4), (0.86, 0.68, 0.06), (0.86, 0.68, 0.06), (0.06, 0.06, 0.07), "9"),
    "white":  ((0.88, 0.88, 0.86), (0.55, 0.1, 0.36), (0.62, 0.62, 0.64), (0.62, 0.48, 0.18), (0.55, 0.1, 0.36), (0.07, 0.07, 0.08), (0.88, 0.88, 0.86), "42"),
    "black":  ((0.06, 0.06, 0.07), (0.82, 0.84, 0.12), (0.06, 0.06, 0.07), (0.06, 0.06, 0.07), (0.82, 0.84, 0.12), (0.06, 0.06, 0.07), (0.82, 0.84, 0.12), "88"),
    "purple": ((0.3, 0.12, 0.48), (0.86, 0.86, 0.84), (0.6, 0.6, 0.62), (0.06, 0.06, 0.07), (0.3, 0.12, 0.48), (0.86, 0.86, 0.84), (0.3, 0.12, 0.48), "5"),
    "teal":   ((0.04, 0.43, 0.46), (0.06, 0.06, 0.07), (0.6, 0.6, 0.62), (0.06, 0.06, 0.07), (0.04, 0.43, 0.46), (0.86, 0.86, 0.84), (0.06, 0.06, 0.07), "22"),
    "green":  ((0.09, 0.36, 0.17), (0.86, 0.86, 0.84), (0.09, 0.36, 0.17), (0.06, 0.06, 0.07), (0.86, 0.86, 0.84), (0.86, 0.86, 0.84), (0.09, 0.36, 0.17), "11"),
    "grey":   ((0.36, 0.37, 0.39), (0.68, 0.1, 0.08), (0.68, 0.1, 0.08), (0.06, 0.06, 0.07), (0.68, 0.1, 0.08), (0.68, 0.1, 0.08), (0.88, 0.88, 0.86), "7"),
    "navy":   ((0.06, 0.1, 0.24), (0.88, 0.42, 0.06), (0.06, 0.1, 0.24), (0.88, 0.42, 0.06), (0.88, 0.42, 0.06), (0.06, 0.1, 0.24), (0.88, 0.42, 0.06), "36"),
}
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

def paint_livery(name):
    from PIL import Image, ImageDraw, ImageFont, ImageFilter
    main, acc, frame, rim, spring, pbg, ink, num = LIVERIES[name]
    C = lambda c: tuple(int(v * 255) for v in c)
    im = Image.new("RGB", (A, A), C(main)); d = ImageDraw.Draw(im)
    # swatches
    sw = {"main": main, "accent": acc, "frame": frame, "seatbase": (0.05, 0.05, 0.055), "rim": rim, "spring": spring,
          "black": (0.035, 0.035, 0.04), "hub": (0.5, 0.5, 0.52), "plate": pbg, "tank": main}
    for k, i in CELL.items():
        x, y = (i % 8) * 128, 512 + (i // 8) * 128; d.rectangle([x, y, x + 127, y + 127], fill=C(sw[k]))
    bdir = os.path.join(BUILD, "brands"); brands = sorted(f for f in os.listdir(bdir) if f.endswith(".png") and "_" not in f)
    r = random.Random(name)
    def brand(box, which):
        b = Image.open(os.path.join(bdir, brands[which % len(brands)])).convert("RGB")
        x0, y0, x1, y1 = box; b.thumbnail((x1 - x0, y1 - y0)); im.paste(b, (x0 + (x1 - x0 - b.width) // 2, y0 + (y1 - y0 - b.height) // 2))
    def number(box, txt, fill, bg=None):
        x0, y0, x1, y1 = box
        if bg: d.rounded_rectangle(box, 14, fill=C(bg))
        size = int((y1 - y0) * 0.78); f = ImageFont.truetype(FONT, size)
        while d.textlength(txt, font=f) > (x1 - x0) * 0.86: size -= 4; f = ImageFont.truetype(FONT, size)
        w = d.textlength(txt, font=f); d.text((x0 + (x1 - x0 - w) / 2, y0 + (y1 - y0 - size * 1.18) / 2), txt, font=f, fill=C(fill))
    # shroud: main colour, accent swoosh, sponsor
    x0, y0, x1, y1 = REG["shroud"]
    d.polygon([(x0, y1), (x0 + 200, y0 + 120), (x1, y0 + 40), (x1, y0 + 110), (x0 + 240, y0 + 175), (x0 + 60, y1)], fill=C(acc))
    d.polygon([(x0, y1 - 18), (x0 + 210, y0 + 150), (x1, y0 + 118), (x1, y0 + 128), (x0 + 220, y0 + 162), (x0 + 20, y1)], fill=C(main))
    brand((x0 + 250, y0 + 150, x1 - 20, y1 - 16), r.randrange(99))
    # side number boards
    x0, y0, x1, y1 = REG["side"]
    d.rectangle([x0, y0, x1, y1], fill=C(pbg)); number((x0 + 120, y0 + 30, x1 - 80, y1 - 40), num, ink)
    d.polygon([(x0, y1 - 40), (x1, y1 - 80), (x1, y1), (x0, y1)], fill=C(acc))
    # front plate
    x0, y0, x1, y1 = REG["plate"]
    d.rectangle([x0, y0, x1, y1], fill=C(pbg)); number((x0 + 20, y0 + 50, x1 - 20, y1 - 30), num, ink)
    brand((x0 + 20, y0 + 6, x1 - 20, y0 + 46), r.randrange(99))
    # fenders: main with an accent stripe
    for k in ("ffender", "rfender"):
        x0, y0, x1, y1 = REG[k]; d.rectangle([x0, y0, x1, y1], fill=C(main)); d.rectangle([x0, y0 + 110, x1, y0 + 140], fill=C(acc))
    # seat: dark gripper cover with a coloured top band and rib pattern
    x0, y0, x1, y1 = REG["seat"]; d.rectangle([x0, y0, x1, y1], fill=(14, 14, 16))
    for k in range(x0, x1, 10): d.line([(k, y0), (k, y0 + 150)], fill=(22, 22, 25), width=3)
    d.rectangle([x0, y0 + 150, x1, y0 + 200], fill=C(acc if sum(acc) < 2.4 else main))
    im = im.filter(ImageFilter.GaussianBlur(0.6))
    # faint wear so plastics don't look factory-flat
    import numpy as np
    a = np.asarray(im).astype(np.float32); n = np.random.RandomState(7).rand(A, A, 1)
    a = a * (0.96 + 0.04 * n); im = Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))
    p = os.path.join(OUT, f"fp_livery_{name}.png"); im.save(p); return p

# ---------------- materials ----------------
def mat(name, rgb, rough=0.6, metal=0.0):
    m = bpy.data.materials.get(name)
    if m: return m
    m = bpy.data.materials.new(name); m.use_nodes = True; b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1); b.inputs["Roughness"].default_value = rough; b.inputs["Metallic"].default_value = metal
    b.inputs["Specular IOR Level"].default_value = 0.35
    return m
def livery_mat(png):
    m = bpy.data.materials.get("livery")
    if m: return m
    m = bpy.data.materials.new("livery"); m.use_nodes = True; nt = m.node_tree; b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(png); nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
    b.inputs["Roughness"].default_value = 0.5; b.inputs["Specular IOR Level"].default_value = 0.35
    return m
M = {}
def mats(png):
    M.update(livery=livery_mat(png), alu=mat("alu", (0.6, 0.6, 0.62), 0.38, 1.0), engine=mat("engine", (0.11, 0.11, 0.12), 0.45, 0.7),
             chrome=mat("chrome", (0.75, 0.75, 0.77), 0.15, 1.0), rubber=mat("rubber", (0.035, 0.035, 0.035), 0.88),
             exhaust=mat("exhaust", (0.36, 0.32, 0.28), 0.42, 1.0), black=mat("blackplastic", (0.03, 0.03, 0.033), 0.7),
             disc=mat("disc", (0.32, 0.32, 0.34), 0.35, 1.0), chain=mat("chain", (0.1, 0.09, 0.08), 0.5, 0.8),
             gold=mat("fork_gold", (0.55, 0.42, 0.17), 0.3, 1.0), rad=mat("radiator", (0.22, 0.22, 0.23), 0.55, 0.6),
             spoke=mat("spoke", (0.62, 0.62, 0.64), 0.3, 1.0))

# ---------------- primitives (each tagged with its frame node) ----------------
OBJ = []
def _new(name, me, m, node):
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o)
    if m: me.materials.append(m)
    o["node"] = node; OBJ.append(o); return o
def solid_uv(o, cell):
    uvl = o.data.uv_layers.new() if not o.data.uv_layers else o.data.uv_layers[0]; u = cell_uv(cell)
    for l in uvl.data: l.uv = u
def cyl(node, a, b, r, m, verts=20, r2=None, cell=None):
    a, b = Vector(a), Vector(b); d = b - a
    bm = bmesh.new(); bmesh.ops.create_cone(bm, cap_ends=True, segments=verts, radius1=r, radius2=r if r2 is None else r2, depth=d.length)
    me = bpy.data.meshes.new("cyl"); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    o = _new("cyl", me, m, node); o.location = (a + b) / 2; o.rotation_euler = d.to_track_quat("Z", "Y").to_euler()
    if cell: solid_uv(o, cell)
    return o
def tube(node, pts, r, m, res=10, cell=None):
    cu = bpy.data.curves.new("tube", "CURVE"); cu.dimensions = "3D"; cu.bevel_depth = r; cu.bevel_resolution = 3; cu.resolution_u = res; cu.use_fill_caps = True
    sp = cu.splines.new("BEZIER"); sp.bezier_points.add(len(pts) - 1)
    for bp, p in zip(sp.bezier_points, pts): bp.co = p; bp.handle_left_type = bp.handle_right_type = "AUTO"
    o = bpy.data.objects.new("tube", cu); bpy.context.scene.collection.objects.link(o); cu.materials.append(m); o["node"] = node; o["cell"] = cell or ""; OBJ.append(o); return o
def rbox(node, loc, size, m, bevel=0.35, sub=2, rot=(0, 0, 0), cell=None):
    bm = bmesh.new(); bmesh.ops.create_cube(bm, size=1); me = bpy.data.meshes.new("rbox"); bm.to_mesh(me); bm.free()
    o = _new("rbox", me, m, node); o.location = loc; o.scale = size; o.rotation_euler = rot
    bv = o.modifiers.new("b", "BEVEL"); bv.width = 0.5 * bevel * min(size) / max(min(size), 1e-6) * 1.0; bv.segments = 3
    bv.width = min(size) * bevel * 0.5 / max(size) if False else 0.5 * bevel * min(size) / 1.0 / max(1e-6, 1.0)
    if sub: s = o.modifiers.new("s", "SUBSURF"); s.levels = s.render_levels = sub
    for p in me.polygons: p.use_smooth = True
    if cell: solid_uv(o, cell)
    return o
def panel(node, outline, thick, m, yoff=0.0, region=None, cell=None, sub=2, side=None):
    """Side-profile outline [(x, z)] extruded by thick along +Y from yoff, UVs projected side-on."""
    bm = bmesh.new(); vs = [bm.verts.new((x, yoff, z)) for x, z in outline]; f = bm.faces.new(vs)
    r = bmesh.ops.extrude_face_region(bm, geom=[f]); ev = [e for e in r["geom"] if isinstance(e, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=ev, vec=(0, thick, 0)); bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    uvl = bm.loops.layers.uv.verify()
    xs = [p[0] for p in outline]; zs = [p[1] for p in outline]; x0, x1, z0, z1 = min(xs), max(xs), min(zs), max(zs)
    left = (side if side is not None else (yoff + thick / 2) > 0)
    for fa in bm.faces:
        for l in fa.loops:
            s = (l.vert.co.x - x0) / (x1 - x0); t = (l.vert.co.z - z0) / (z1 - z0)
            if left: s = 1 - s
            l[uvl].uv = reg_uv(region, s, t) if region else cell_uv(cell)
    me = bpy.data.meshes.new("panel"); bm.to_mesh(me); bm.free()
    o = _new("panel", me, m, node)
    bv = o.modifiers.new("b", "BEVEL"); bv.width = min(thick * 0.45, 0.02); bv.segments = 2
    if sub: s_ = o.modifiers.new("s", "SUBSURF"); s_.levels = s_.render_levels = sub
    for p in me.polygons: p.use_smooth = True
    return o
def plate(node, outline, thick, centre, tilt, m):
    """Front number plate: outline [(y, z)] seen from the front, tilted back by tilt (rad)."""
    bm = bmesh.new(); vs = [bm.verts.new((0, y, z)) for y, z in outline]; f = bm.faces.new(vs)
    r = bmesh.ops.extrude_face_region(bm, geom=[f]); ev = [e for e in r["geom"] if isinstance(e, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=ev, vec=(-thick, 0, 0)); bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    uvl = bm.loops.layers.uv.verify(); ys = [p[0] for p in outline]; zs = [p[1] for p in outline]
    for fa in bm.faces:
        for l in fa.loops:
            l[uvl].uv = reg_uv("plate", (l.vert.co.y - min(ys)) / (max(ys) - min(ys)), (l.vert.co.z - min(zs)) / (max(zs) - min(zs)))
    me = bpy.data.meshes.new("plate"); bm.to_mesh(me); bm.free()
    o = _new("plate", me, m, node); o.location = centre; o.rotation_euler = (0, -tilt, 0)
    bv = o.modifiers.new("b", "BEVEL"); bv.width = 0.008; bv.segments = 2
    s = o.modifiers.new("s", "SUBSURF"); s.levels = s.render_levels = 2
    for p in me.polygons: p.use_smooth = True
    return o

# ---------------- parts ----------------
def wheel(node, c, R_out, R_rim, width, front):
    seg, n = 72, 12; th = R_out - R_rim
    prof = [(R_rim + 0.015 + (th - 0.03) * math.sin(math.pi * i / n), (width / 2) * math.cos(math.pi * i / n) * 0.95) for i in range(n + 1)]
    bm = bmesh.new(); rings = []
    for k in range(seg):
        a = 2 * math.pi * k / seg; rings.append([bm.verts.new((r * math.cos(a), y, r * math.sin(a))) for r, y in prof])
    for k in range(seg):
        a_, b_ = rings[k], rings[(k + 1) % seg]
        for i in range(n): bm.faces.new((a_[i], a_[i + 1], b_[i + 1], b_[i]))
    nk = 40
    for k in range(nk):
        ang = 2 * math.pi * k / nk; rows = (0.0, -0.36, 0.36) if k % 2 == 0 else (-0.18, 0.18, -0.46, 0.46)
        for yf in rows:
            kh = 0.02 + (0.008 if abs(yf) < 0.2 else 0); kd, kw = 0.026, 0.03
            rr = R_out - 0.03 - (0.012 if abs(yf) > 0.4 else 0)
            for v in bmesh.ops.create_cube(bm, size=1)["verts"]:
                x, yy, z = v.co.x * kd, v.co.y * kw + yf * width, v.co.z * kh + kh / 2 + rr
                ca, sa = math.cos(ang), math.sin(ang); v.co = Vector((z * ca - x * sa, yy, z * sa + x * ca))
    me = bpy.data.meshes.new("tyre"); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    t = _new("tyre", me, M["rubber"], node); t.location = c
    bpy.ops.mesh.primitive_torus_add(major_radius=R_rim, minor_radius=0.013, major_segments=72, minor_segments=8, location=c, rotation=(math.pi / 2, 0, 0))
    r_ = bpy.context.object; r_.scale = (1, 1, 2.0); r_.data.materials.append(M["livery"]); r_["node"] = node; OBJ.append(r_); solid_uv(r_, "rim")
    for p in r_.data.polygons: p.use_smooth = True
    cyl(node, c + Vector((0, -0.065, 0)), c + Vector((0, 0.065, 0)), 0.042, M["livery"], cell="hub")
    cyl(node, c + Vector((0, -0.09, 0)), c + Vector((0, 0.09, 0)), 0.012, M["alu"], 8)
    for i in range(32):
        side = 1 if i % 2 else -1; a0 = 2 * math.pi * i / 32; a1 = a0 + side * 0.32
        h = c + Vector((0.036 * math.cos(a0), side * 0.055, 0.036 * math.sin(a0)))
        e = c + Vector((R_rim * math.cos(a1) * 0.985, side * 0.01, R_rim * math.sin(a1) * 0.985))
        cyl(node, h, e, 0.0022, M["spoke"], 5)
    rd = 0.135 if front else 0.12; yd = 0.075 if front else -0.075
    disc = cyl(node, c + Vector((0, yd - 0.0025, 0)), c + Vector((0, yd + 0.0025, 0)), rd, M["disc"], 40)
    cyl(node, c + Vector((0, yd - 0.003, 0)), c + Vector((0, yd + 0.003, 0)), rd * 0.36, M["alu"], 20)

def build_bike(model):
    four = model.startswith("four"); cc = int(model.lstrip("twofur")); s = {125: 0.88, 250: 1.0, 500: 1.12, 400: 1.0, 600: 1.1}[cc]
    L = M["livery"]
    # wheels
    wheel("TireRear", RA, 0.345, 0.235, 0.115 if cc > 125 else 0.105, False)
    wheel("TireFront", FA, 0.355, 0.27, 0.09, True)
    # steering axis: from the bar pivot straight down to the front axle
    ax = (FA - HP).normalized(); tilt = math.atan2(ax.x, -ax.z)
    top = HP + Vector((0, 0, 0.0)); lower = HP + ax * 0.2
    for sgn in (-1, 1):
        o = Vector((0, sgn * 0.1, 0))
        cyl("HandleBars", top + o - ax * 0.02, HP + o + ax * 0.47, 0.028, M["gold"] if not four else M["engine"], 20)     # outer tubes
        cyl("Shocks", HP + o + ax * 0.4, FA + o + ax * 0.0 - ax * 0.03, 0.022, M["chrome"], 16)                           # stanchion
        cyl("Shocks", HP + o + ax * 0.6, FA + o - ax * 0.05, 0.03, M["livery"], 16, r2=0.026, cell="black")             # guard
        cyl("Shocks", FA + o - ax * 0.06, FA + o + ax * 0.04, 0.03, M["alu"], 14)                                         # axle lug
    rbox("Shocks", FA + Vector((0, 0.1, 0)) - ax * 0.1 + Vector((-0.05, 0.025, 0.04)), (0.07, 0.03, 0.05), M["alu"], sub=1)     # caliper
    rbox("HandleBars", top, (0.08, 0.27, 0.035), M["alu"], 0.4, 1, rot=(0, tilt, 0))
    rbox("HandleBars", lower, (0.09, 0.27, 0.04), M["alu"], 0.4, 1, rot=(0, tilt, 0))
    hb = top + Vector((-0.04, 0, 0.1))
    for sgn in (-1, 1): cyl("HandleBars", top + Vector((0, sgn * 0.035, 0.0)), hb + Vector((0, sgn * 0.035, 0)), 0.014, M["alu"], 10)
    tube("HandleBars", [hb + Vector((0.02, -0.4, 0.05)), hb + Vector((0.01, -0.22, 0.0)), hb, hb + Vector((0.01, 0.22, 0.0)), hb + Vector((0.02, 0.4, 0.05))], 0.0125, M["alu"])
    rbox("HandleBars", hb + Vector((0, 0, 0.015)), (0.05, 0.14, 0.04), L, sub=2, cell="accent")
    for sgn in (-1, 1):
        cyl("HandleBars", hb + Vector((0.02, sgn * 0.3, 0.025)), hb + Vector((0.02, sgn * 0.42, 0.05)), 0.018, M["black"], 14)
        tube("HandleBars", [hb + Vector((0.03, sgn * 0.2, 0.025)), hb + Vector((0.09, sgn * 0.3, 0.0)), hb + Vector((0.11, sgn * 0.37, -0.012))], 0.005, M["black"])
        rbox("HandleBars", hb + Vector((0.02, sgn * 0.18, 0.02)), (0.04, 0.035, 0.03), M["black"], sub=1)
    plate("HandleBars", [(-0.13, 0.0), (0.13, 0.0), (0.155, 0.17), (0.12, 0.27), (-0.12, 0.27), (-0.155, 0.17)], 0.012, lower + ax * -0.05 + Vector((0.09, 0, -0.08)), tilt, L)
    ff = [(1.22, 0.79), (1.40, 0.815), (1.6, 0.79), (1.78, 0.72), (1.85, 0.68), (1.82, 0.67), (1.6, 0.735), (1.42, 0.755), (1.24, 0.745)]
    panel("HandleBars", ff, 0.17, L, yoff=-0.085, region="ffender")
    # frame
    fr = M["alu"] if four else L; fcell = None if four else "frame"
    head_a, head_b = HP + ax * 0.12, HP + ax * 0.36
    cyl("Bike", head_a, head_b, 0.033, fr, 16, cell=fcell)
    for sgn in (-1, 1):
        o = Vector((0, sgn * 0.06, 0)); o2 = Vector((0, sgn * 0.115, 0))
        tube("Bike", [head_a + o + Vector((-0.02, 0, -0.03)), Vector((0.86, sgn * 0.115, 0.88)), Vector((0.64, sgn * 0.13, 0.74)), SP + o2 + Vector((0.0, 0, 0.06)), SP + o2 + Vector((0.03, 0, -0.16))], 0.022 if four else 0.018, fr, cell=fcell)
        tube("Bike", [head_b + o * 0.5, Vector((1.0, sgn * 0.05, 0.5)), Vector((0.92, sgn * 0.07, 0.26)), Vector((0.66, sgn * 0.08, 0.25)), SP + o2 + Vector((0.03, 0, -0.16))], 0.014, fr, cell=fcell)
        tube("Bike", [Vector((0.64, sgn * 0.12, 0.74)), Vector((0.3, sgn * 0.1, 0.82)), Vector((0.0, sgn * 0.08, 0.86))], 0.011, M["alu"])
        tube("Bike", [SP + o2 + Vector((-0.03, 0, 0.03)), Vector((0.25, sgn * 0.1, 0.67)), Vector((0.06, sgn * 0.085, 0.84))], 0.009, M["alu"])
        rbox("Bike", (0.6, sgn * 0.2, 0.37), (0.055, 0.11, 0.025), M["alu"], sub=0)       # footpegs
    rbox("Bike", (0.78, 0, 0.215), (0.4, 0.18, 0.02), M["alu"], 0.2, 1)                    # skid plate
    # rear shock
    sb = SP + Vector((-0.14, 0, -0.17)); st = Vector((0.6, 0, 0.76))
    cyl("Bike", st, st + (sb - st) * 0.45, 0.026, M["alu"], 16); cyl("Bike", sb, st, 0.009, M["chrome"], 8)
    cyl("Bike", st + Vector((-0.02, 0.055, -0.03)), st + Vector((-0.08, 0.065, -0.18)), 0.022, M["alu"], 14)
    Ls = st - sb; p1 = Ls.cross(Vector((0, 1, 0))).normalized(); p2 = Ls.normalized().cross(p1)
    tube("Bike", [sb + Ls * (0.12 + 0.6 * t / 120) + p1 * math.cos(2 * math.pi * 10 * t / 120) * 0.04 + p2 * math.sin(2 * math.pi * 10 * t / 120) * 0.04 for t in range(121)], 0.0065, L, res=2, cell="spring")
    # swingarm + chain
    for sgn in (-1, 1):
        tube("SwingArm", [SP + Vector((0, sgn * 0.1, -0.03)), Vector((0.3, sgn * 0.11, 0.41)), RA + Vector((0.03, sgn * 0.085, 0.0))], 0.024, M["alu"])
        rbox("SwingArm", RA + Vector((0.04, sgn * 0.085, 0)), (0.07, 0.025, 0.05), M["alu"], sub=1)
    cyl("SwingArm", SP + Vector((0, -0.14, -0.03)), SP + Vector((0, 0.14, -0.03)), 0.017, M["alu"], 12)
    rbox("SwingArm", (0.3, 0.12, 0.35), (0.3, 0.012, 0.05), M["black"], sub=1)              # chain guide
    cyl("SwingArm", RA + Vector((0, 0.072, 0)), RA + Vector((0, 0.077, 0)), 0.11, M["alu"], 40)
    fs = Vector((0.72, 0.077, 0.36)); cyl("Bike", fs - Vector((0, 0.003, 0)), fs + Vector((0, 0.003, 0)), 0.035, M["engine"], 16)
    tube("SwingArm", [fs + Vector((0, 0, 0.037)), RA + Vector((0.0, 0.077, 0.112))], 0.006, M["chain"])
    tube("SwingArm", [fs + Vector((0, 0, -0.037)), Vector((0.42, 0.077, 0.31)), RA + Vector((0.0, 0.077, -0.112))], 0.006, M["chain"])
    rbox("SwingArm", RA + Vector((0.06, -0.09, -0.06)), (0.06, 0.025, 0.045), M["alu"], sub=1)   # rear caliper
    # engine
    E = M["engine"]
    rbox("Bike", (0.8, 0, 0.36), (0.34 * s, 0.17, 0.25 * s), E, 0.5)
    c = Vector((0.83, -0.085, 0.37)); cyl("Bike", c, c + Vector((0, -0.035, 0)), 0.115 * s, M["alu"] if four else E, 28)          # clutch cover
    cyl("Bike", c + Vector((0.03, -0.035, 0.02)), c + Vector((0.03, -0.045, 0.02)), 0.05 * s, M["alu"], 16)
    c = Vector((0.74, 0.085, 0.37)); cyl("Bike", c, c + Vector((0, 0.03, 0)), 0.095 * s, E, 28)                              # ignition cover
    rbox("Bike", (0.7, 0.1, 0.36), (0.07, 0.02, 0.06), M["black"], sub=1)                                                  # sprocket cover
    if four:
        rbox("Bike", (0.88, 0, 0.56), (0.16 * s, 0.14 * s, 0.2 * s), E, 0.3, rot=(0, math.radians(-12), 0))
        rbox("Bike", (0.9, 0, 0.7), (0.2 * s, 0.17 * s, 0.1 * s), M["alu"], 0.4, rot=(0, math.radians(-12), 0))      # cam cover
        hd = [Vector((0.98, -0.02, 0.62)), Vector((1.07, -0.06, 0.52)), Vector((1.06, -0.1, 0.33)), Vector((0.9, -0.15, 0.26)),
              Vector((0.62, -0.17, 0.35)), Vector((0.42, -0.17, 0.58)), Vector((0.3, -0.16, 0.7))]
        tube("Bike", hd, 0.024 * s, M["exhaust"])
        cyl("Bike", Vector((0.34, -0.165, 0.69)), Vector((-0.12, -0.165, 0.83)), 0.062 * s, M["alu"], 28)
        cyl("Bike", Vector((-0.12, -0.165, 0.83)), Vector((-0.15, -0.165, 0.84)), 0.04 * s, E, 16)
    else:
        rbox("Bike", (0.87, 0, 0.53), (0.14 * s, 0.13 * s, 0.17 * s), E, 0.3, rot=(0, math.radians(-18), 0))
        rbox("Bike", (0.9, 0, 0.63), (0.15 * s, 0.14 * s, 0.05 * s), M["alu"], 0.4, rot=(0, math.radians(-18), 0))
        r = 0.038 * s
        pipe = [Vector((0.98, -0.02, 0.55)), Vector((1.08, -0.05, 0.45)), Vector((1.07, -0.1, 0.29)), Vector((0.94, -0.13, 0.23)), Vector((0.72, -0.16, 0.31)),
                Vector((0.48, -0.17, 0.52)), Vector((0.34, -0.16, 0.64))]
        tube("Bike", pipe, r, M["exhaust"])
        cyl("Bike", Vector((0.36, -0.16, 0.65)), Vector((-0.06, -0.16, 0.77)), 0.045 * s, M["alu"], 24, r2=0.04 * s)
        cyl("Bike", Vector((-0.06, -0.16, 0.77)), Vector((-0.1, -0.16, 0.78)), 0.022, E, 12)
    rbox("Bike", (1.0, 0, 0.47), (0.03, 0.06, 0.12), E, 0.3)                                # water pump / hose block
    for sgn in (-1, 1):
        rbox("Bike", (1.0, sgn * 0.13, 0.73), (0.05, 0.07, 0.28), M["rad"], 0.2, 1, rot=(0, math.radians(-14), 0))
        tube("Bike", [Vector((0.98, sgn * 0.1, 0.6)), Vector((0.95, sgn * 0.05, 0.52)), Vector((1.0, 0, 0.5))], 0.012, M["black"])
    # kickstart lever, right side
    tube("KickStart", [KP + Vector((0, -0.03, 0)), KP + Vector((0, -0.06, 0)), KP + Vector((-0.12, -0.07, -0.05)), KP + Vector((-0.2, -0.07, -0.06))], 0.011, M["alu"])
    rbox("KickStart", KP + Vector((-0.2, -0.1, -0.06)), (0.03, 0.06, 0.02), M["alu"], sub=0)
    # bodywork
    rbox("Bike", (0.86, 0, 0.86), (0.36, 0.24, 0.17), L, 0.5, cell="tank")
    sh = [(0.84, 0.975), (1.0, 0.995), (1.12, 0.94), (1.11, 0.81), (1.04, 0.68), (0.95, 0.63), (0.88, 0.69), (0.84, 0.84)]
    for sgn in (-1, 1): panel("Bike", sh, 0.012, L, yoff=sgn * 0.158 - (0.012 if sgn < 0 else 0), region="shroud")
    seat = [(0.95, 0.965), (0.78, 0.99), (0.4, 0.995), (0.0, 0.99), (-0.18, 0.975), (-0.2, 0.945), (0.2, 0.935), (0.6, 0.925), (0.92, 0.9)]
    panel("Bike", seat, 0.24, L, yoff=-0.12, region="seat", side=False)
    sp_ = [(0.56, 0.91), (0.2, 0.935), (-0.03, 0.925), (0.02, 0.74), (0.36, 0.68), (0.54, 0.75)]
    for sgn in (-1, 1): panel("Bike", sp_, 0.012, L, yoff=sgn * 0.132 - (0.012 if sgn < 0 else 0), region="side")
    rf = [(0.25, 0.94), (-0.15, 0.955), (-0.52, 0.905), (-0.54, 0.88), (-0.15, 0.915), (0.25, 0.905)]
    panel("Bike", rf, 0.22, L, yoff=-0.11, region="rfender", side=False)
    rbox("Bike", (0.42, 0, 0.77), (0.3, 0.2, 0.14), M["black"])                              # airbox
    bpy.data.objects.new("NullFrame", None)

def assemble(model):
    """Bake to game space, join each frame node's parts, origins at the pivots."""
    T = Matrix.Translation(to_game((0, 0, 0))) @ Matrix.Rotation(math.pi / 2, 4, "Z") @ Matrix.Scale(K, 4)
    bpy.context.view_layer.update()
    for o in list(OBJ):
        o.matrix_world = T @ o.matrix_world
    bpy.ops.object.select_all(action="DESELECT")
    for o in OBJ: o.select_set(True)
    bpy.context.view_layer.objects.active = OBJ[0]
    bpy.ops.object.convert(target="MESH")
    for o in bpy.context.scene.objects:
        if o.type == "MESH" and o.get("cell"): solid_uv(o, o["cell"])
        if o.type == "MESH" and not o.data.uv_layers: solid_uv(o, "black")
        if o.type == "MESH":
            while len(o.data.uv_layers) > 1: o.data.uv_layers.remove(o.data.uv_layers[-1])
            o.data.uv_layers[0].name = "UVMap"
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    nodes = {}
    for n in ORDER:
        parts = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.get("node") == n]
        piv = Vector((PIV[n][0], PIV[n][2], PIV[n][1]))
        if not parts:
            e = bpy.data.objects.new(n, None); bpy.context.scene.collection.objects.link(e); e.location = piv; nodes[n] = e; continue
        bpy.ops.object.select_all(action="DESELECT")
        for o in parts: o.select_set(True)
        bpy.context.view_layer.objects.active = parts[0]
        if len(parts) > 1: bpy.ops.object.join()
        o = bpy.context.view_layer.objects.active; o.name = n; o.data.name = n
        o.data.transform(Matrix.Translation(-piv)); o.location = piv; nodes[n] = o
    bpy.context.view_layer.update()
    for n, p in PARENT.items():
        c = nodes[n]; mw = c.matrix_world.copy(); c.parent = nodes[p]; c.matrix_world = mw
    tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in nodes.values() if o.type == "MESH")
    return tris

def export(model):
    path = os.path.join(OUT, f"fp_bike_{model}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
    return path

MODELS = ["two125", "two250", "two500", "four400", "four600"]
if __name__ == "__main__":
    want = [a for a in sys.argv[1:] if not a.startswith("-")] or MODELS
    pngs = {n: paint_livery(n) for n in LIVERIES}
    for i, m in enumerate(want):
        reset(); OBJ.clear(); M.clear()
        mats(pngs[list(LIVERIES)[i % len(LIVERIES)]])
        build_bike(m if m[:3] != "fou" else m); tris = assemble(m); export(m)
        print("bike", m, "tris", tris, flush=True)
