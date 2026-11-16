"""Free Pack rider: our own procedural motocross rider in full kit, built
to the engine's 16-part rider frame (rigid parts, origins at the pivots,
standing base pose; the game's motions sit it on the bike).

    python blender/rider.py

Writes build/riders/fp_rider.glb and build/riders/fp_rider_<outfit>.png.
Every outfit shares one UV layout; only the material named "outfit"
takes the outfit picture. Built directly in game feet (Blender X = game x,
Y = game z forward, Z = game y up).
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector, Matrix
from common import reset, BUILD

OUT = os.path.join(BUILD, "riders"); os.makedirs(OUT, exist_ok=True)
G = {  # game (x, y, z) pivots from the engine's frame tree
    "Pelvis": (0, 3.177, -0.290), "LwrTorso": (0, 3.355, -0.437), "UprLegL": (-0.276, 3.177, -0.290), "UprLegR": (0.276, 3.177, -0.290),
    "UprTorso": (0, 3.976, -0.437), "Head": (0, 5.193, -0.437), "UprArmL": (-0.650, 4.901, -0.375), "UprArmR": (0.650, 4.901, -0.375),
    "LwrArmL": (-0.841, 3.812, -0.385), "HandL": (-0.992, 2.950, -0.318), "LwrArmR": (0.841, 3.812, -0.385), "HandR": (0.992, 2.950, -0.318),
    "LwrLegL": (-0.275, 1.707, -0.157), "FootL": (-0.274, 0.330, -0.300), "LwrLegR": (0.275, 1.707, -0.157), "FootR": (0.274, 0.330, -0.300)}
ORDER = list(G)
PARENT = {"LwrTorso": "Pelvis", "UprLegL": "Pelvis", "UprLegR": "Pelvis", "UprTorso": "LwrTorso", "Head": "UprTorso", "UprArmL": "UprTorso",
          "UprArmR": "UprTorso", "LwrArmL": "UprArmL", "HandL": "LwrArmL", "LwrArmR": "UprArmR", "HandR": "LwrArmR", "LwrLegL": "UprLegL",
          "FootL": "LwrLegL", "LwrLegR": "UprLegR", "FootR": "LwrLegR"}
def P(n): x, y, z = G[n]; return Vector((x, z, y))

# ---------------- outfit atlas ----------------
A = 1024
REG = {"jersey": (0, 0, 512, 512), "sleeve": (512, 0, 1024, 256), "pants": (512, 256, 1024, 512), "boot": (0, 512, 512, 768),
       "helmet": (512, 512, 1024, 1024), "glove": (0, 768, 256, 1024)}
CELLS = {"main": (256, 768), "accent": (320, 768), "dark": (384, 768), "white": (448, 768), "skin": (256, 832), "strap": (320, 832), "brace": (384, 832), "sole": (448, 832)}
def reg_uv(r, s, t):
    x0, y0, x1, y1 = REG[r]; return ((x0 + 2 + s * (x1 - x0 - 4)) / A, 1 - (y0 + 2 + (1 - t) * (y1 - y0 - 4)) / A)
def cell_uv(c):
    x, y = CELLS[c]; return ((x + 32) / A, 1 - (y + 32) / A)

OUTFITS = {  # main, accent, pants main, helmet, boot, number
    "red":   ((0.6, 0.08, 0.07), (0.9, 0.9, 0.88), (0.12, 0.12, 0.13), (0.88, 0.88, 0.86), (0.9, 0.9, 0.88), "3"),
    "blue":  ((0.12, 0.24, 0.52), (0.88, 0.88, 0.86), (0.12, 0.24, 0.52), (0.12, 0.24, 0.52), (0.88, 0.88, 0.86), "14"),
    "lime":  ((0.62, 0.74, 0.27), (0.07, 0.07, 0.08), (0.07, 0.07, 0.08), (0.07, 0.07, 0.08), (0.07, 0.07, 0.08), "27"),
    "orange": ((0.85, 0.36, 0.05), (0.08, 0.08, 0.09), (0.08, 0.08, 0.09), (0.85, 0.36, 0.05), (0.88, 0.88, 0.86), "71"),
    "white": ((0.88, 0.88, 0.86), (0.55, 0.1, 0.36), (0.88, 0.88, 0.86), (0.88, 0.88, 0.86), (0.88, 0.88, 0.86), "42"),
    "yellow": ((0.86, 0.68, 0.06), (0.1, 0.18, 0.4), (0.1, 0.18, 0.4), (0.86, 0.68, 0.06), (0.1, 0.18, 0.4), "9"),
}
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
def paint_outfit(name):
    from PIL import Image, ImageDraw, ImageFont, ImageFilter
    import numpy as np
    main, acc, pants, helm, boot, num = OUTFITS[name]
    C = lambda c: tuple(int(v * 255) for v in c)
    im = Image.new("RGB", (A, A), C(main)); d = ImageDraw.Draw(im)
    for k, c in (("main", main), ("accent", acc), ("dark", (0.05, 0.05, 0.055)), ("white", (0.85, 0.85, 0.83)), ("skin", (0.62, 0.45, 0.35)),
                 ("strap", acc), ("brace", (0.12, 0.12, 0.13)), ("sole", (0.04, 0.04, 0.04))):
        x, y = CELLS[k]; d.rectangle([x, y, x + 63, y + 63], fill=C(c))
    bdir = os.path.join(BUILD, "brands"); brands = sorted(f for f in os.listdir(bdir) if f.endswith(".png") and "_" not in f); r = random.Random(name)
    def brand(box):
        b = Image.open(os.path.join(bdir, r.choice(brands))).convert("RGB"); x0, y0, x1, y1 = box
        b.thumbnail((x1 - x0, y1 - y0)); im.paste(b, (x0 + (x1 - x0 - b.width) // 2, y0 + (y1 - y0 - b.height) // 2))
    def text(cx, cy, s, size, fill):
        f = ImageFont.truetype(FONT, size); w = d.textlength(s, font=f); d.text((cx - w / 2, cy - size * 0.6), s, font=f, fill=C(fill))
    # jersey: u 0.25 = chest, 0.75 = back; v 0 = hem, 1 = collar
    x0, y0, x1, y1 = REG["jersey"]
    d.rectangle([x0, y0, x1, y0 + 90], fill=C(acc))                          # shoulder yoke
    for u in (0.0, 0.5):                                                       # side panels
        xa = x0 + int(u * 512); d.rectangle([xa - 40, y0, xa + 40, y1], fill=C(acc)) if u else (d.rectangle([x0, y0, x0 + 40, y1], fill=C(acc)), d.rectangle([x1 - 40, y0, x1, y1], fill=C(acc)))
    d.rectangle([x0, y1 - 60, x1, y1], fill=C(acc))                            # hem band
    text(x0 + 384, y0 + 250, num, 170, acc if sum(acc) > 1.2 else (0.92, 0.92, 0.9))   # back number
    brand((x0 + 70, y0 + 140, x0 + 190, y0 + 210))                             # chest sponsor
    # sleeves: accent cuff and stripe
    x0, y0, x1, y1 = REG["sleeve"]; d.rectangle([x0, y1 - 40, x1, y1], fill=C(acc)); d.rectangle([x0, y0 + 60, x1, y0 + 80], fill=C(acc))
    # pants: their own main, accent knee panels and hip sponsor
    x0, y0, x1, y1 = REG["pants"]; d.rectangle([x0, y0, x1, y1], fill=C(pants))
    d.rectangle([x0 + 60, y0 + 140, x0 + 200, y0 + 220], fill=C(acc)); d.rectangle([x0 + 312, y0 + 140, x0 + 452, y0 + 220], fill=C(acc))
    brand((x0 + 200, y0 + 20, x0 + 320, y0 + 70))
    # boots: base colour, dark sole band and buckle straps
    x0, y0, x1, y1 = REG["boot"]; d.rectangle([x0, y0, x1, y1], fill=C(boot)); d.rectangle([x0, y1 - 40, x1, y1], fill=(14, 14, 14))
    for k in range(4): yy = y0 + 30 + k * 50; d.rectangle([x0, yy, x1, yy + 14], fill=(40, 40, 42))
    # helmet: shell with two stripes and a sponsor on the sides
    x0, y0, x1, y1 = REG["helmet"]; d.rectangle([x0, y0, x1, y1], fill=C(helm))
    for u in (0.47, 0.53): xa = x0 + int(u * 512); d.rectangle([xa - 12, y0, xa + 12, y1], fill=C(acc))
    brand((x0 + 20, y0 + 300, x0 + 160, y0 + 360)); brand((x0 + 352, y0 + 300, x0 + 492, y0 + 360))
    # gloves
    x0, y0, x1, y1 = REG["glove"]; d.rectangle([x0, y0, x1, y1], fill=C(acc if sum(acc) < 1 else main)); d.rectangle([x0, y0, x1, y0 + 50], fill=C(main))
    a = np.asarray(im.filter(ImageFilter.GaussianBlur(0.8))).astype(np.float32)
    n = np.random.RandomState(3).rand(A, A, 1); a = a * (0.95 + 0.05 * n)     # fabric grain
    p = os.path.join(OUT, f"fp_rider_{name}.png"); Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).save(p); return p

# ---------------- geometry ----------------
PARTS = []
def mat(name, rgb, rough, metal=0.0):
    m = bpy.data.materials.get(name)
    if m: return m
    m = bpy.data.materials.new(name); m.use_nodes = True; b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1); b.inputs["Roughness"].default_value = rough; b.inputs["Metallic"].default_value = metal; return m
def outfit_mat(png):
    m = bpy.data.materials.get("outfit")
    if m: return m
    m = bpy.data.materials.new("outfit"); m.use_nodes = True; nt = m.node_tree; b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(png); nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
    b.inputs["Roughness"].default_value = 0.85; b.inputs["Specular IOR Level"].default_value = 0.2; return m

def limb(node, a, b, secs, m, region=None, cell=None, seg=24, sub=1, fwd=Vector((0, 1, 0)), cap=True):
    """Smooth tube from a to b through sections [(t, half_width_x, half_depth_fwd, offset_fwd)]; cylindrical UVs."""
    a, b = Vector(a), Vector(b); ax = (b - a); L = ax.length; ax.normalize()
    side = fwd.cross(ax).normalized() if abs(fwd.dot(ax)) < 0.95 else Vector((1, 0, 0)); front = ax.cross(side).normalized()
    if front.dot(fwd) < 0: front, side = -front, -side
    bm = bmesh.new(); uvl = bm.loops.layers.uv.verify(); rings = []
    for t, rx, ry, off in secs:
        c = a + ax * (L * t) + front * off
        rings.append([bm.verts.new(c + side * (rx * math.cos(2 * math.pi * k / seg)) + front * (ry * math.sin(2 * math.pi * k / seg))) for k in range(seg)])
    for i in range(len(rings) - 1):
        for k in range(seg):
            f = bm.faces.new((rings[i][k], rings[i][(k + 1) % seg], rings[i + 1][(k + 1) % seg], rings[i + 1][k]))
            for l, (kk, ii) in zip(f.loops, ((k, i), (k + 1, i), (k + 1, i + 1), (k, i + 1))):
                # u: 0.25 faces forward, 0.75 backward (seam at the side)
                ang = (2 * math.pi * kk / seg); u = ((ang + math.pi / 2) / (2 * math.pi)) % 1.0 if kk != seg or True else 1.0
                u = 0.0 + ((kk / seg) + 0.0)
                u = (u - 0.0) % 1.0 if kk < seg else 1.0
                tv = secs[ii][0]
                l[uvl].uv = reg_uv(region, u, 1 - tv) if region else cell_uv(cell)
    if cap:
        for ring in (rings[0], rings[-1]):
            f = bm.faces.new(ring if ring is rings[-1] else ring[::-1])
            for l in f.loops: l[uvl].uv = cell_uv("dark") if not cell else cell_uv(cell)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(node); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    o = bpy.data.objects.new(node, me); bpy.context.scene.collection.objects.link(o); me.materials.append(m); o["node"] = node; PARTS.append(o)
    if sub: s = o.modifiers.new("s", "SUBSURF"); s.levels = s.render_levels = sub
    return o

def blob(node, c, r, m, cell=None, region=None, scale=(1, 1, 1), seg=24, rings=16, sub=1):
    bm = bmesh.new(); uvl = bm.loops.layers.uv.verify()
    bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=rings, radius=r)
    for v in bm.verts: v.co = Vector((v.co.x * scale[0], v.co.y * scale[1], v.co.z * scale[2]))
    for f in bm.faces:
        for l in f.loops:
            p = l.vert.co; ang = math.atan2(p.x, p.y)   # 0 = forward
            u = (ang / (2 * math.pi) + 0.5) % 1.0; v = 0.5 + p.z / (2 * r * scale[2])
            l[uvl].uv = reg_uv(region, u, v) if region else cell_uv(cell)
    for v in bm.verts: v.co += Vector(c)
    me = bpy.data.meshes.new(node); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = True
    o = bpy.data.objects.new(node, me); bpy.context.scene.collection.objects.link(o); me.materials.append(m); o["node"] = node; PARTS.append(o)
    if sub: s = o.modifiers.new("s", "SUBSURF"); s.levels = s.render_levels = sub
    return o

def build(png):
    O = outfit_mat(png); lens = mat("goggle_lens", (0.08, 0.06, 0.03), 0.08, 0.6); rub = mat("boot_sole", (0.03, 0.03, 0.03), 0.9)
    steel = mat("buckle", (0.55, 0.55, 0.57), 0.3, 1.0)
    F = Vector((0, 1, 0))
    # torso: baggy jersey over a chest protector
    limb("Pelvis", P("Pelvis") + Vector((0, 0, -0.42)), P("Pelvis") + Vector((0, 0, 0.3)), [(0, 0.5, 0.36, 0.0), (0.45, 0.6, 0.42, 0.0), (1, 0.56, 0.38, -0.03)], O, region="pants")
    limb("LwrTorso", P("LwrTorso") + Vector((0, 0.06, 0.0)), P("UprTorso") + Vector((0, 0.06, 0.05)), [(0, 0.58, 0.4, 0.0), (0.5, 0.55, 0.38, 0.0), (1, 0.6, 0.42, 0.02)], O, region="jersey")
    top = P("UprTorso") + Vector((0, 0.07, 1.0))
    limb("UprTorso", P("UprTorso") + Vector((0, 0.07, 0.0)), top, [(0, 0.6, 0.42, 0.0), (0.45, 0.68, 0.48, 0.05), (0.8, 0.74, 0.42, 0.03), (0.93, 0.58, 0.34, 0.0), (1, 0.22, 0.22, 0.0)], O, region="jersey")
    limb("UprTorso", top + Vector((0, 0.0, -0.06)), top + Vector((0, 0.0, 0.12)), [(0, 0.34, 0.36, 0.0), (0.5, 0.36, 0.38, 0.0), (1, 0.28, 0.32, 0.0)], O, cell="brace")   # neck brace
    # helmet: shell, chin bar, peak, goggles
    hc = P("Head") + Vector((0, 0.06, 0.5))
    blob("Head", hc, 0.5, O, region="helmet", scale=(0.92, 1.12, 1.0), sub=1)
    limb("Head", hc + Vector((0, 0.3, -0.42)), hc + Vector((0, 0.78, -0.3)), [(0, 0.36, 0.28, 0.0), (0.6, 0.24, 0.16, 0.0), (1, 0.1, 0.1, 0.0)], O, cell="main", fwd=Vector((0, 0, 1)))   # chin bar
    pk = bmesh.new(); uvl = pk.loops.layers.uv.verify()
    pts = [hc + Vector((x, 0.5 + 0.55 * (1 - (x / 0.42) ** 2) * 0.9, 0.3 + 0.1 * (1 - (x / 0.42) ** 2))) for x in [i / 8 * 0.84 - 0.42 for i in range(9)]]
    back = [hc + Vector((p.x - hc.x, 0.42 - 0.1 * abs(p.x - hc.x), 0.42)) for p in pts]
    vs1 = [pk.verts.new(p) for p in pts]; vs2 = [pk.verts.new(p) for p in back]
    for i in range(8):
        f = pk.faces.new((vs2[i], vs2[i + 1], vs1[i + 1], vs1[i]))
        for l in f.loops: l[uvl].uv = cell_uv("accent")
    me = bpy.data.meshes.new("peak"); pk.to_mesh(me); pk.free(); o = bpy.data.objects.new("peak", me); bpy.context.scene.collection.objects.link(o)
    me.materials.append(O); o["node"] = "Head"; PARTS.append(o); s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.03
    s2 = o.modifiers.new("s", "SUBSURF"); s2.levels = s2.render_levels = 1
    g = hc + Vector((0, 0.5, 0.02))
    blob("Head", g, 0.17, lens, scale=(1.75, 0.45, 0.75), sub=1, cell="dark")
    limb("Head", hc + Vector((-0.47, 0.18, 0.02)), hc + Vector((0.47, 0.18, 0.02)), [(0, 0.05, 0.075, 0), (1, 0.05, 0.075, 0)], O, cell="strap", fwd=Vector((0, 0, 1)), sub=0)
    blob("Head", g, 0.19, O, scale=(1.8, 0.4, 0.8), sub=1, cell="dark")
    # arms: jersey sleeves, elbow pads, gloves
    for sd in "LR":
        sh, el, wr = P("UprArm" + sd), P("LwrArm" + sd), P("Hand" + sd)
        limb("UprArm" + sd, sh + Vector((0, 0, 0.12)), el, [(0, 0.24, 0.24, 0), (0.3, 0.23, 0.22, 0), (1, 0.17, 0.17, 0)], O, region="sleeve")
        limb("LwrArm" + sd, el + (el - sh).normalized() * -0.06, wr, [(0, 0.17, 0.17, 0), (0.15, 0.18, 0.19, -0.02), (0.8, 0.13, 0.12, 0), (1, 0.12, 0.11, 0)], O, region="sleeve")
        d = (wr - el).normalized(); hand_end = wr + d * 0.5
        limb("Hand" + sd, wr - d * 0.05, hand_end, [(0, 0.12, 0.1, 0), (0.3, 0.14, 0.08, 0.0), (0.8, 0.13, 0.07, 0.02), (1, 0.08, 0.05, 0.02)], O, region="glove")
        sgn = -1 if sd == "L" else 1
        limb("Hand" + sd, wr + d * 0.12, wr + d * 0.28 + Vector((0, 0.08, 0)), [(0, 0.04, 0.04, 0), (1, 0.035, 0.035, 0)], O, region="glove", sub=1)   # thumb
    # legs: pants, knee braces, boots
    for sd in "LR":
        hip, kn, an = P("UprLeg" + sd), P("LwrLeg" + sd), P("Foot" + sd)
        limb("UprLeg" + sd, hip + Vector((0, 0, 0.12)), kn, [(0, 0.3, 0.33, 0.0), (0.5, 0.27, 0.28, 0.02), (1, 0.21, 0.22, 0.02)], O, region="pants")
        limb("LwrLeg" + sd, kn + Vector((0, 0, 0.08)), an + Vector((0, 0, 0.75)), [(0, 0.22, 0.22, 0.02), (0.2, 0.2, 0.2, 0.02), (0.55, 0.17, 0.17, 0.0), (1, 0.18, 0.18, 0.0)], O, region="pants")
        limb("LwrLeg" + sd, kn + Vector((0, 0.0, 0.25)), kn + Vector((0, 0, -0.35)), [(0, 0.21, 0.2, 0.06), (0.5, 0.23, 0.22, 0.09), (1, 0.21, 0.2, 0.06)], O, cell="brace", sub=1)   # knee brace
        limb("LwrLeg" + sd, an + Vector((0, 0.04, 0.95)), an + Vector((0, 0.02, -0.05)), [(0, 0.24, 0.26, 0.04), (0.5, 0.22, 0.25, 0.04), (1, 0.21, 0.24, 0.02)], O, region="boot")      # boot shaft
        for k in range(3):
            z = an.z + 0.15 + k * 0.28
            limb("LwrLeg" + sd, an + Vector((-0.25 if sd == "L" else 0.25, 0.04, z - an.z)), an + Vector((-0.25 if sd == "L" else 0.25, 0.04, z - an.z + 0.07)), [(0, 0.03, 0.1, 0), (1, 0.03, 0.1, 0)], steel, cell="dark", sub=0, fwd=Vector((0, 1, 0)))
        # foot: toe box and sole
        limb("Foot" + sd, an + Vector((0, -0.28, -0.12)), an + Vector((0, 0.72, -0.18)), [(0, 0.17, 0.2, 0.0), (0.3, 0.2, 0.22, 0.02), (0.75, 0.18, 0.18, -0.02), (1, 0.1, 0.08, -0.06)], O, region="boot", fwd=Vector((0, 0, 1)))
        limb("Foot" + sd, an + Vector((0, -0.3, -0.3)), an + Vector((0, 0.72, -0.3)), [(0, 0.17, 0.04, 0.0), (0.5, 0.2, 0.04, 0), (1, 0.1, 0.03, 0)], rub, cell="sole", sub=0, fwd=Vector((0, 0, 1)))

def assemble():
    bpy.context.view_layer.update()
    bpy.ops.object.select_all(action="DESELECT")
    for o in PARTS: o.select_set(True)
    bpy.context.view_layer.objects.active = PARTS[0]; bpy.ops.object.convert(target="MESH")
    nodes = {}
    for n in ORDER:
        ps = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.get("node") == n]; piv = P(n)
        bpy.ops.object.select_all(action="DESELECT")
        for o in ps: o.select_set(True)
        bpy.context.view_layer.objects.active = ps[0]
        if len(ps) > 1: bpy.ops.object.join()
        o = bpy.context.view_layer.objects.active; o.name = n; o.data.name = n
        while len(o.data.uv_layers) > 1: o.data.uv_layers.remove(o.data.uv_layers[-1])
        o.data.transform(Matrix.Translation(-piv)); o.location = piv; nodes[n] = o
    bpy.context.view_layer.update()
    for n, p in PARENT.items():
        c = nodes[n]; mw = c.matrix_world.copy(); c.parent = nodes[p]; c.matrix_world = mw
    return sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in nodes.values())

if __name__ == "__main__":
    pngs = {n: paint_outfit(n) for n in OUTFITS}
    reset(); PARTS.clear(); build(pngs["red"]); tris = assemble()
    bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "fp_rider.glb"), export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
    print("rider tris", tris)
