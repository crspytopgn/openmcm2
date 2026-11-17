"""Bake real surfaces into a Free Pack bike or rider.

The game draws a free-pack bike or rider with one picture (the livery or
outfit) and flat colours for everything else, lit only by the sun on the
vertex normals. So all the detail a modern renderer would get from normal
and roughness maps is baked into the colour here: every part (frame, rims,
tyres, engine, chain, seat, kit fabric, boots, helmet) gets its own
material detail, ambient occlusion, edge wear and a little track dust,
all on one new 2048 px atlas. The livery or outfit picture is carried
over through a baked look-up of its old UVs, so every colourway gets the
same detail without re-baking.

    python blender/surface_bake.py bike  build/bikes/fp_bike_mx2.glb
    python blender/surface_bake.py rider build/riders/fp_rider.glb
    then python tools/compose_atlas.py fp_bike_mx2 (or fp_rider)

Writes the model back in place (one textured material plus "plate", and
"lens" on the rider) and the baked layers for compose_atlas.py, which
writes each colourway's full atlas (bikes/<model>/livery_<colour>.png,
riders/fp_rider_<outfit>.png). Keeps the source as *.src.glb and
*.src.png the first time, and always bakes from those.
"""
import os, sys, math, glob, shutil
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from mathutils import Vector
from common import reset

KIND, PATH = sys.argv[-2], os.path.abspath(sys.argv[-1])
S = int(os.environ.get("FP_ATLAS", "2048"))
SRC = PATH.replace(".glb", ".src.glb")
if not os.path.exists(SRC): shutil.copy(PATH, SRC)
DIR = os.path.dirname(PATH)
PICS = sorted(glob.glob(os.path.join(DIR, "fp_livery_*.png" if KIND == "bike" else "fp_rider_*.png")))
PICS = [p for p in PICS if not p.endswith(".src.png")]
for p in PICS:
    s = p.replace(".png", ".src.png")
    if not os.path.exists(s): shutil.copy(p, s)
SRCPIC = [p.replace(".png", ".src.png") for p in PICS]

reset(); bpy.ops.import_scene.gltf(filepath=SRC)
MESHES = [o for o in bpy.context.scene.objects if o.type == "MESH"]
TEXMAT = "livery" if KIND == "bike" else "outfit"
KEEP = {"plate", "lens"}          # left with their own UVs and colour

# ---------------- what each face is made of ----------------
A = 1024
if KIND == "bike":
    from bike import REG, CELL
    CELLKIND = {"main": "paint", "accent": "paint", "frame": "frame", "seatbase": "seat", "rim": "rim", "spring": "spring",
                "black": "black", "hub": "alu", "plate": "paint", "tank": "paint"}
    REGKIND = {"shroud": "plastic", "side": "plastic", "plate": "plastic", "ffender": "plastic", "rfender": "plastic", "seat": "seat"}
else:
    from rider import REG, CELLS
    REGKIND = {"jersey": "jersey", "sleeve": "jersey", "pants": "pants", "boot": "boot", "helmet": "helmet", "glove": "glove"}
    CELLKIND = {"main": "jersey", "accent": "jersey", "dark": "pants", "white": "boot", "skin": "skin", "strap": "strap",
                "brace": "boot", "sole": "sole"}

def tex_kind(u, v):
    x, y = u * A, (1 - v) * A
    for k, (x0, y0, x1, y1) in REG.items():
        if x0 <= x < x1 and y0 <= y < y1: return REGKIND[k]
    if KIND == "bike":
        i = int(x // 128) + 8 * int((y - 512) // 128)
        name = next((n for n, j in CELL.items() if j == i), "black"); return CELLKIND[name]
    for n, (cx, cy) in CELLS.items():
        if cx <= x < cx + 64 and cy <= y < cy + 64: return CELLKIND[n]
    return REGKIND["jersey"]

def solid_kind(m, obj):
    n = m.name.lower(); c = m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value if m.use_nodes else m.diffuse_color
    lum = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
    if any(k in n for k in ("tire", "tyre")) or (obj.name.startswith("Tire") and lum < 0.06): return "tyre"
    if "rubber" in n: return "rubber" if KIND == "rider" else "tyre"
    for key, kind in (("spring", "spring"), ("chain", "chain"), ("radiator", "radiator"), ("exahust", "pipe"), ("pipe", "pipe"),
                      ("disc", "disc"), ("seat", "seat"), ("cushon", "black"), ("grip", "grip"), ("case", "cast"), ("carb", "cast"),
                      ("skid", "alu"), ("sproiket", "alu"), ("sporks", "chrome"), ("metal_part", "chrome"), ("bolts", "chrome")):
        if key in n: return kind
    if c[0] > c[2] * 1.6 and c[0] > 0.3: return ("paint", c)
    if lum > 0.45: return "alu"
    if lum > 0.15: return "cast"
    return "black"

# ---------------- procedural surfaces (node trees, emitted for baking) ----------------
def tree(name):
    m = bpy.data.materials.new(name); m.use_nodes = True; nt = m.node_tree; nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial"); em = nt.nodes.new("ShaderNodeEmission"); nt.links.new(em.outputs[0], out.inputs[0])
    return m, nt, em

def N(nt, kind, **kw):
    n = nt.nodes.new(kind)
    for k, v in kw.items():
        if k.startswith("i_"): n.inputs[k[2:].replace("_", " ")].default_value = v
        else: setattr(n, k, v)
    return n

def coord(nt, scale=(1, 1, 1)):
    tc = N(nt, "ShaderNodeTexCoord"); mp = N(nt, "ShaderNodeMapping"); mp.inputs["Scale"].default_value = scale
    nt.links.new(tc.outputs["Object"], mp.inputs["Vector"]); return mp.outputs[0]

def noise(nt, vec, scale, detail=4.0, rough=0.55):
    n = N(nt, "ShaderNodeTexNoise", i_Scale=scale, i_Detail=detail, i_Roughness=rough); nt.links.new(vec, n.inputs["Vector"]); return n.outputs["Fac"]

def op(nt, oper, a, b=None, clamp=False):
    n = N(nt, "ShaderNodeMath", operation=oper, use_clamp=clamp)
    for i, x in enumerate((a, b)):
        if x is None: continue
        if isinstance(x, (int, float)): n.inputs[i].default_value = x
        else: nt.links.new(x, n.inputs[i])
    return n.outputs[0]

def around(nt, f, mid, amp):            # mid + amp * (f - 0.5) * 2
    return op(nt, "ADD", op(nt, "MULTIPLY", op(nt, "SUBTRACT", f, 0.5), 2 * amp), mid)

def surface(kind, colour=None):
    """Grey detail (for picture-coloured faces) or a full colour (solids):
    returns (material, is_tinted). Tinted ones multiply the livery/outfit."""
    m, nt, em = tree(f"bake_{kind}")
    p = coord(nt); geo = N(nt, "ShaderNodeNewGeometry")
    edge = op(nt, "MULTIPLY", op(nt, "SUBTRACT", geo.outputs["Pointiness"], 0.52), 6.0, clamp=True)   # convex edges
    def grain(scale, amp, mid=1.0, stretch=None):
        v = coord(nt, stretch) if stretch else p
        return around(nt, noise(nt, v, scale, 6, 0.6), mid, amp)
    def mul(*xs):
        o = xs[0]
        for x in xs[1:]: o = op(nt, "MULTIPLY", o, x)
        return o
    def mix_col(fac, c1, c2):
        mx = N(nt, "ShaderNodeMix", data_type="RGBA"); nt.links.new(fac, mx.inputs["Factor"])
        for sock, c in ((6, c1), (7, c2)):
            if isinstance(c, tuple): mx.inputs[sock].default_value = (*c, 1)
            else: nt.links.new(c, mx.inputs[sock])
        return mx.outputs[2]
    def grey(v):
        c = N(nt, "ShaderNodeCombineColor"); [nt.links.new(v, c.inputs[i]) for i in range(3)]; return c.outputs[0]
    def tint(col, g):
        mm = N(nt, "ShaderNodeMix", data_type="RGBA", blend_type="MULTIPLY"); mm.inputs["Factor"].default_value = 1
        mm.inputs[6].default_value = (*col, 1); nt.links.new(g, mm.inputs[7]); return mm.outputs[2]
    tinted = kind in ("paint", "plastic", "frame", "rim", "spring", "jersey", "pants", "boot", "helmet", "glove", "strap", "skin", "sole") and colour is None
    if kind in ("paint", "plastic", "helmet", "boot"):
        # moulded plastic: faint mottling, fine scratches lighter, scuffed (whitened) edges
        scratch = op(nt, "GREATER_THAN", noise(nt, coord(nt, (1, 1, 40)), 18, 2, 0.5), 0.71)
        g = mul(grain(9, 0.03), grain(60, 0.025))
        g = op(nt, "ADD", g, op(nt, "MULTIPLY", op(nt, "ADD", scratch, op(nt, "MULTIPLY", edge, 0.5)), 0.07))
        col = grey(g)
    elif kind in ("frame", "spring"):
        col = grey(op(nt, "ADD", mul(grain(14, 0.04), grain(90, 0.02)), op(nt, "MULTIPLY", edge, 0.12)))
    elif kind == "rim":
        col = grey(op(nt, "ADD", mul(grain(40, 0.06, 1.0, (1, 1, 30))), op(nt, "MULTIPLY", edge, 0.18)))
    elif kind == "jersey":
        # knit polyester: a fine weave plus soft mottling
        w1 = N(nt, "ShaderNodeTexWave", wave_type="BANDS", bands_direction="X", i_Scale=420, i_Distortion=0.6); nt.links.new(p, w1.inputs["Vector"])
        w2 = N(nt, "ShaderNodeTexWave", wave_type="BANDS", bands_direction="Z", i_Scale=420, i_Distortion=0.6); nt.links.new(p, w2.inputs["Vector"])
        weave = around(nt, op(nt, "MULTIPLY", w1.outputs["Fac"], w2.outputs["Fac"]), 1.0, 0.06)
        col = grey(mul(weave, grain(6, 0.05), grain(120, 0.04)))
    elif kind in ("pants", "glove", "strap"):
        # heavy nylon / leather panels: tighter weave, more grain
        w1 = N(nt, "ShaderNodeTexWave", wave_type="BANDS", bands_direction="DIAGONAL", i_Scale=300, i_Distortion=1.5); nt.links.new(p, w1.inputs["Vector"])
        col = grey(mul(around(nt, w1.outputs["Fac"], 1.0, 0.07), grain(5, 0.06), grain(80, 0.05)))
    elif kind in ("skin", "sole"):
        col = grey(grain(30, 0.06))
    elif kind == "tyre":
        c = mix_col(op(nt, "ADD", op(nt, "MULTIPLY", edge, 0.8), op(nt, "MULTIPLY", noise(nt, p, 25), 0.3), clamp=True), (0.032, 0.031, 0.03), (0.075, 0.068, 0.06))
        col = c
    elif kind == "rubber":
        col = mix_col(noise(nt, p, 40), (0.03, 0.03, 0.032), (0.05, 0.05, 0.052))
    elif kind in ("alu", "chrome", "disc"):
        # the game has no reflections, so metal carries a baked one: sky
        # bright on top, the ground's dark below, sharper for chrome
        lo, hi = {"alu": (0.16, 0.5), "chrome": (0.06, 0.78), "disc": (0.2, 0.48)}[kind]
        sepn = N(nt, "ShaderNodeSeparateXYZ"); nt.links.new(geo.outputs["Normal"], sepn.inputs[0])
        sky = op(nt, "POWER", op(nt, "MULTIPLY", op(nt, "ADD", sepn.outputs[2], 1.0), 0.5), 1.6 if kind == "chrome" else 1.0)
        base = op(nt, "ADD", lo, op(nt, "MULTIPLY", sky, hi - lo))
        brushed = grain(30, 0.1 if kind != "chrome" else 0.04, 1.0, (1, 1, 60) if kind != "disc" else (40, 40, 1))
        g = op(nt, "ADD", op(nt, "MULTIPLY", brushed, base), op(nt, "MULTIPLY", edge, 0.1))
        col = tint((1.0, 1.0, 1.02), grey(g))
    elif kind == "cast":
        g = op(nt, "MULTIPLY", mul(grain(70, 0.12), grain(8, 0.06)), 0.24)
        col = tint((1.0, 0.99, 0.96), grey(op(nt, "ADD", g, op(nt, "MULTIPLY", edge, 0.14))))
    elif kind == "black":
        g = op(nt, "MULTIPLY", mul(grain(12, 0.12), grain(80, 0.08)), 0.045)
        col = grey(op(nt, "ADD", g, op(nt, "MULTIPLY", edge, 0.05)))
    elif kind == "grip":
        w = N(nt, "ShaderNodeTexWave", wave_type="RINGS", i_Scale=200); nt.links.new(p, w.inputs["Vector"])
        col = grey(op(nt, "MULTIPLY", around(nt, w.outputs["Fac"], 1, 0.25), 0.04))
    elif kind == "seat":
        v = N(nt, "ShaderNodeTexVoronoi", i_Scale=180); nt.links.new(p, v.inputs["Vector"])
        col = grey(op(nt, "MULTIPLY", around(nt, v.outputs["Distance"], 1, 0.3), 0.05)) if colour is None or True else None
        tinted = False
    elif kind == "chain":
        col = mix_col(noise(nt, p, 90), (0.07, 0.06, 0.05), (0.2, 0.12, 0.07))
    elif kind == "radiator":
        w = N(nt, "ShaderNodeTexWave", wave_type="BANDS", bands_direction="Z", i_Scale=260); nt.links.new(p, w.inputs["Vector"])
        col = grey(op(nt, "MULTIPLY", around(nt, w.outputs["Fac"], 1, 0.35), 0.13))
    elif kind == "pipe":
        heat = noise(nt, p, 3, 2)
        col = mix_col(heat, (0.06, 0.058, 0.06), (0.11, 0.085, 0.07))
    else:
        col = grey(grain(20, 0.05))
    if colour is not None and kind == "paint":
        col = tint(tuple(colour[:3]), col)
    nt.links.new(col, em.inputs["Color"]); return m, tinted

# ---------------- one atlas UV over every part ----------------
for o in MESHES:
    # glTF splits vertices at every seam and hard edge; weld them so the
    # unwrap finds whole islands instead of thousands of single faces
    bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4); bm.to_mesh(o.data); bm.free()
    o.data.uv_layers.active_index = 0
    if "bake" not in o.data.uv_layers: o.data.uv_layers.new(name="bake")
bpy.ops.object.select_all(action="DESELECT")
for o in MESHES: o.select_set(True)
bpy.context.view_layer.objects.active = MESHES[0]
bpy.ops.object.mode_set(mode="EDIT")
for o in MESHES: o.data.uv_layers.active = o.data.uv_layers["bake"]
bpy.ops.mesh.select_all(action="SELECT")
for o in MESHES: o.data.uv_layers.active = o.data.uv_layers["bake"]
bpy.ops.uv.smart_project(angle_limit=math.radians(72), island_margin=0.003, area_weight=0.0, scale_to_bounds=False)
bpy.ops.uv.pack_islands(rotate=True, margin=0.003)
bpy.ops.object.mode_set(mode="OBJECT")

# per face: which bake surface, and for picture faces, the old UV kind
mats = {}
def bake_mat(kind, colour=None):
    key = (kind, tuple(round(c, 3) for c in colour[:3]) if colour is not None else None)
    if key not in mats: mats[key] = surface(kind, colour)
    return mats[key]
tintmask_mat = {}
final_tex = None
for o in MESHES:
    me = o.data; old = me.uv_layers[0]; orig = [m for m in me.materials]
    slots = {}
    assign = []
    for f in me.polygons:
        m = orig[f.material_index]
        if m is None or m.name.split(".")[0] in KEEP: assign.append(("keep", m)); continue
        if m.name.split(".")[0] == TEXMAT:
            final_tex = m
            u = sum(old.data[li].uv[0] for li in f.loop_indices) / f.loop_total; v = sum(old.data[li].uv[1] for li in f.loop_indices) / f.loop_total
            assign.append(("bake", bake_mat(tex_kind(u, v))))
        else:
            k = solid_kind(m, o)
            assign.append(("bake", bake_mat(*k) if isinstance(k, tuple) else bake_mat(k)))
    o["orig"] = [m.name if m else "" for m in orig]
    me.materials.clear()
    index = {}
    for f, (what, m) in zip(me.polygons, assign):
        mm = m if what == "keep" else m[0]
        if mm is None: continue
        if mm.name not in index: me.materials.append(mm); index[mm.name] = len(me.materials) - 1
        f.material_index = index[mm.name]
    o["keep"] = [n for n in index if n.split(".")[0] in KEEP]
    o["tinted"] = [m[0].name for (w, m) in assign if w == "bake" and m[1]]

# ---------------- bakes ----------------
sc = bpy.context.scene; sc.render.engine = "CYCLES"; sc.cycles.device = "CPU"
sc.render.bake.margin = 12; sc.render.bake.use_clear = True
def target(name, float_=False):
    im = bpy.data.images.new(name, S, S, alpha=False, float_buffer=float_); im.colorspace_settings.name = "Non-Color"
    for m in bpy.data.materials:
        if not m.name.startswith("bake_") or not m.use_nodes: continue
        nt = m.node_tree; t = nt.nodes.get("bake_target") or nt.nodes.new("ShaderNodeTexImage"); t.name = "bake_target"; t.image = im
        uvn = nt.nodes.get("bake_uv") or nt.nodes.new("ShaderNodeUVMap"); uvn.name = "bake_uv"; uvn.uv_map = "bake"; nt.links.new(uvn.outputs[0], t.inputs[0])
        nt.nodes.active = t
    return im
def bakeable():
    bpy.ops.object.select_all(action="DESELECT")
    for o in MESHES:
        o.select_set(True); o.data.uv_layers.active = o.data.uv_layers["bake"]
    bpy.context.view_layer.objects.active = MESHES[0]
def pixels(im):
    a = np.empty(S * S * 4, np.float32); im.pixels.foreach_get(a); return a.reshape(S, S, 4)[::-1, :, :3].copy()
# keep-material faces must not receive bakes: give them a throwaway target
for o in MESHES:
    for n in o["keep"]:
        m = bpy.data.materials[n]; m.use_nodes = True; nt = m.node_tree
        t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.new("junk_" + n, 8, 8); nt.nodes.active = t

def swap_emission(fn):
    """Point every bake material's emission at fn(nt) for one bake; return a restore list."""
    saved = []
    for m in bpy.data.materials:
        if not m.name.startswith("bake_"): continue
        nt = m.node_tree; em = next(n for n in nt.nodes if n.type == "EMISSION")
        old = em.inputs["Color"].links[0].from_socket if em.inputs["Color"].links else None
        saved.append((nt, em, old)); s = fn(nt, m)
        nt.links.new(s, em.inputs["Color"])
    return saved
def restore(saved):
    for nt, em, old in saved:
        if old is not None: nt.links.new(old, em.inputs["Color"])

bakeable()
sc.cycles.samples = 8
im_col = target("colour"); bpy.ops.object.bake(type="EMIT"); col = pixels(im_col)
print("baked colour")
# old UV + which faces take the picture
def remap(nt, m):
    uv = N(nt, "ShaderNodeUVMap", uv_map="UVMap"); sep = N(nt, "ShaderNodeSeparateXYZ"); nt.links.new(uv.outputs[0], sep.inputs[0])
    c = N(nt, "ShaderNodeCombineXYZ"); nt.links.new(sep.outputs[0], c.inputs[0]); nt.links.new(sep.outputs[1], c.inputs[1])
    c.inputs[2].default_value = 1.0 if any(m.name in o["tinted"] for o in MESHES) else 0.0
    return c.outputs[0]
saved = swap_emission(remap); sc.cycles.samples = 1
im_uv = target("remap", True); bpy.ops.object.bake(type="EMIT"); uvmap = pixels(im_uv); restore(saved)
print("baked remap")
# track dust: low parts, gathered in creases
def dust(nt, m):
    geo = N(nt, "ShaderNodeNewGeometry"); sep = N(nt, "ShaderNodeSeparateXYZ"); nt.links.new(geo.outputs["Position"], sep.inputs[0])
    low = op(nt, "SUBTRACT", 1.0, op(nt, "MULTIPLY", sep.outputs[2], 1.0 / (2.2 if KIND == "bike" else 1.6)), clamp=True)
    tc = N(nt, "ShaderNodeTexCoord"); n = N(nt, "ShaderNodeTexNoise", i_Scale=3.0, i_Detail=8.0, i_Roughness=0.7); nt.links.new(tc.outputs["Object"], n.inputs["Vector"])
    splat = op(nt, "MULTIPLY", op(nt, "SUBTRACT", n.outputs["Fac"], 0.42), 4.0, clamp=True)
    up = op(nt, "MULTIPLY", op(nt, "SUBTRACT", 1.0, sep.outputs[2]), 0.0)
    d = op(nt, "MULTIPLY", op(nt, "POWER", low, 1.6), op(nt, "ADD", op(nt, "MULTIPLY", splat, 0.7), 0.3))
    c = N(nt, "ShaderNodeCombineColor"); [nt.links.new(d, c.inputs[i]) for i in range(3)]; return c.outputs[0]
saved = swap_emission(dust); sc.cycles.samples = 4
im_d = target("dust"); bpy.ops.object.bake(type="EMIT"); dustm = pixels(im_d)[..., 0]; restore(saved)
print("baked dust")
sc.cycles.samples = int(os.environ.get("FP_AO_SAMPLES", "48"))
sc.world = sc.world or bpy.data.worlds.new("w")
sc.world.light_settings.distance = 0.6 if KIND == "bike" else 0.35
im_ao = target("ao"); bpy.ops.object.bake(type="AO"); ao = pixels(im_ao)[..., 0]
print("baked ao")
np.save(os.path.join(DIR, f".bake_{os.path.basename(PATH)[:-4]}.npy"), np.dstack([col, uvmap, dustm[..., None], ao[..., None]]).astype(np.float16))

# ---------------- final model: one atlas material, plate/lens kept ----------------
final = bpy.data.materials.new(TEXMAT); final.use_nodes = True; nt = final.node_tree; b = nt.nodes["Principled BSDF"]
t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(PICS[0]); nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
b.inputs["Roughness"].default_value = 0.6
for o in MESHES:
    me = o.data; old = me.uv_layers[0]; new = me.uv_layers["bake"]
    keep = {i for i, m in enumerate(me.materials) if m and m.name.split(".")[0] in KEEP}
    plates = {i for i, m in enumerate(me.materials) if m and m.name.split(".")[0] == "plate"}
    for f in me.polygons:
        if f.material_index in plates:   # number plates read upright with glTF's v (top = 0)
            for li in f.loop_indices: old.data[li].uv = (old.data[li].uv[0], 1.0 - old.data[li].uv[1])
        if f.material_index in keep: continue
        for li in f.loop_indices: old.data[li].uv = new.data[li].uv
    names = [m.name for m in me.materials]
    for i, m in enumerate(list(me.materials)):
        if i not in keep: me.materials[i] = final
    me.uv_layers.remove(me.uv_layers["bake"])
    # merge duplicate slots so the exporter writes few primitives
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
for m in list(bpy.data.materials):
    if m.name.startswith("bake_"): bpy.data.materials.remove(m)
bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(filepath=PATH, export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
print("wrote", PATH)
