"""Photo-like crowd textures for the Free Pack stadiums: real 3D fans (the
CC-BY base body used for the rider, see CREDITS) posed seated, standing or
cheering on rows of plastic stadium seats, rendered straight onto the
slope of a stand. Each picture tiles left-right and top-bottom, and covers
WIDTH ft of stand along the row and ROWS rows up the slope.

    python blender/crowd_render.py SRC.glb [variant ...]   (variants 0-3)

Writes build/tex/fp_crowd2_<variant>.png (2048 px).
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
from mathutils import Vector, Matrix
from common import reset, BUILD
import rider_import as R

WIDTH, ROWS, RUN, RISE = 40.0, 15, 2.8, 1.6        # ft: texture width, rows per picture, row depth and step
SLOPE = math.atan2(RISE, RUN); ROWLEN = math.hypot(RUN, RISE)
SEAT = 1.75                                        # seat pitch along the row
DROP = ("Tearline", "Eye_Occlusion", "Cornea", "Teeth", "Tongue")
OUT = os.environ.get("CROWD_OUT") or os.path.join(BUILD, "tex"); os.makedirs(OUT, exist_ok=True)

def person(src, kind, hat=True):
    """One posed fan as a single mesh object (ft, facing -Y toward the camera, seat point at the origin)."""
    bpy.ops.import_scene.gltf(filepath=src)
    new = [o for o in bpy.data.objects if not o.name.startswith("fan_")]
    arm = next(o for o in new if o.type == "ARMATURE")
    for o in list(new):
        if o.type == "MESH" and ((not hat and o.data.materials and o.data.materials[0] and o.data.materials[0].name.split(".")[0] == "material") or o.name.startswith("Icosphere") or any(k in (o.data.materials[0].name if o.data.materials else "") for k in DROP)):
            bpy.data.objects.remove(o, do_unlink=True); new.remove(o)
    root = arm
    while root.parent: root = root.parent
    bpy.context.view_layer.update()
    b = lambda p: R.bone(arm, p); w = lambda n, tail=False: R.wpos(arm, n, tail)
    fwd = Vector((0, -1, 0)); up = Vector((0, 0, 1))
    for s, sx in (("L", 1), ("R", -1)):
        th, ca, ua, fa = b(f"CC_Base_{s}_Thigh"), b(f"CC_Base_{s}_Calf"), b(f"CC_Base_{s}_Upperarm"), b(f"CC_Base_{s}_Forearm")
        if kind != "stand":
            hp = w(th); R.aim(arm, th, hp + fwd * 1.0 + up * 0.08)
            kn = w(ca); R.aim(arm, ca, kn - up * 1.0 + fwd * 0.12)
        sh = w(ua)
        if kind == "cheer":
            R.aim(arm, ua, sh + up * 1.0 + Vector((sx * 0.35, 0, 0)) + fwd * 0.15); R.aim(arm, fa, w(fa) + up * 1.0 + fwd * 0.1)
        elif kind == "clap":
            R.aim(arm, ua, sh - up * 0.7 + fwd * 0.6 - Vector((sx * 0.1, 0, 0))); R.aim(arm, fa, w(fa) + fwd * 0.6 + up * 0.55 - Vector((sx * 0.35, 0, 0)))
        else:
            R.aim(arm, ua, sh - up * 1.0 + fwd * 0.25 + Vector((sx * 0.08, 0, 0))); R.aim(arm, fa, w(fa) + fwd * 1.0 - up * 0.15 - Vector((sx * 0.25, 0, 0)))
    if kind != "stand":   # pelvis down onto the seat: drop the whole body by the thigh length
        pass
    dg = bpy.context.evaluated_depsgraph_get(); parts = []
    for o in [o for o in new if o.type == "MESH"]:
        me = bpy.data.meshes.new_from_object(o.evaluated_get(dg)); me.transform(o.matrix_world)
        n = bpy.data.objects.new("p", me); bpy.context.scene.collection.objects.link(n); parts.append(n)
    keep = {p.name for p in parts}
    for o in list(bpy.data.objects):
        if o.name not in keep and not o.name.startswith("fan_"): bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts: p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]; bpy.ops.object.join(); o = bpy.context.view_layer.objects.active
    # model space: the import is in metres facing -Y; scale to a 5.7 ft person and seat point at origin
    vs = [v.co for v in o.data.vertices]; h = max(v.z for v in vs) - min(v.z for v in vs)
    s = 5.7 / (h if kind == "stand" else h / 0.74)
    o.data.transform(Matrix.Scale(s, 4))
    vs = [v.co for v in o.data.vertices]; zmin = min(v.z for v in vs)
    cx = sum(v.x for v in vs) / len(vs); cy = sum(v.y for v in vs) / len(vs)
    # seated: hips (lowest point of the thighs) sit at z = 0; feet hang below
    seat_z = 0.0
    if kind == "stand":
        o.data.transform(Matrix.Translation((-cx, -cy, -zmin)))
    else:
        hipz = sorted(v.z for v in vs)[int(len(vs) * 0.30)]
        o.data.transform(Matrix.Translation((-cx, -cy + 0.35, -zmin - 1.55)))
    o.name = f"fan_{kind}{'' if hat else '_bare'}"; o.hide_render = True
    return o

def recolour(o):
    """Materials keyed on what they are: clothes take the instance's colour."""
    for m in o.data.materials:
        if not m or not m.use_nodes: continue
        n = m.name.lower(); nt = m.node_tree; bsdf = nt.nodes.get("Principled BSDF")
        if not bsdf: continue
        info = nt.nodes.new("ShaderNodeObjectInfo")
        def ramp(colours, seed_mul):
            mul = nt.nodes.new("ShaderNodeMath"); mul.operation = "MULTIPLY"; mul.inputs[1].default_value = seed_mul
            fr = nt.nodes.new("ShaderNodeMath"); fr.operation = "FRACT"
            nt.links.new(info.outputs["Random"], mul.inputs[0]); nt.links.new(mul.outputs[0], fr.inputs[0])
            cr = nt.nodes.new("ShaderNodeValToRGB"); cr.color_ramp.interpolation = "CONSTANT"
            el = cr.color_ramp.elements
            while len(el) < len(colours): el.new(0.5)
            for i, c in enumerate(colours): el[i].position = i / len(colours); el[i].color = (*c, 1)
            nt.links.new(fr.outputs[0], cr.inputs[0]); return cr.outputs[0]
        def keep_detail(col):
            """Multiply the cloth's own shading (folds, seams) in, as grey."""
            tex = next((x for x in nt.nodes if x.type == "TEX_IMAGE"), None)
            if tex is None: return col
            bw = nt.nodes.new("ShaderNodeRGBToBW"); nt.links.new(tex.outputs["Color"], bw.inputs[0])
            norm = nt.nodes.new("ShaderNodeMath"); norm.operation = "MULTIPLY"; norm.inputs[1].default_value = 1.8
            nt.links.new(bw.outputs[0], norm.inputs[0])
            mx = nt.nodes.new("ShaderNodeMix"); mx.data_type = "RGBA"; mx.blend_type = "MULTIPLY"; mx.inputs["Factor"].default_value = 0.85
            nt.links.new(col, mx.inputs[6]); g = nt.nodes.new("ShaderNodeCombineColor")
            for i in range(3): nt.links.new(norm.outputs[0], g.inputs[i])
            nt.links.new(g.outputs[0], mx.inputs[7]); return mx.outputs[2]
        if "shirt" in n:
            nt.links.new(keep_detail(ramp(SHIRTS, 1.0)), bsdf.inputs["Base Color"])
        elif "jeans" in n:
            nt.links.new(keep_detail(ramp(PANTS, 3.7)), bsdf.inputs["Base Color"])
        elif "skin" in n:
            mx = nt.nodes.new("ShaderNodeMix"); mx.data_type = "RGBA"; mx.blend_type = "MULTIPLY"; mx.inputs["Factor"].default_value = 1.0
            tex = next((x for x in nt.nodes if x.type == "TEX_IMAGE"), None)
            if tex: nt.links.new(tex.outputs["Color"], mx.inputs[6])
            nt.links.new(ramp(SKINS, 7.3), mx.inputs[7]); nt.links.new(mx.outputs[2], bsdf.inputs["Base Color"])
        bsdf.inputs["Roughness"].default_value = max(bsdf.inputs["Roughness"].default_value, 0.6)

SHIRTS = [(0.62, 0.62, 0.6), (0.03, 0.03, 0.035), (0.05, 0.08, 0.2), (0.38, 0.04, 0.03), (0.15, 0.16, 0.18), (0.45, 0.38, 0.28),
          (0.06, 0.15, 0.07), (0.5, 0.36, 0.05), (0.75, 0.75, 0.73), (0.18, 0.25, 0.4), (0.3, 0.08, 0.16), (0.55, 0.2, 0.04),
          (0.02, 0.02, 0.02), (0.25, 0.3, 0.12), (0.6, 0.55, 0.4), (0.1, 0.3, 0.45)]
PANTS = [(0.05, 0.07, 0.14), (0.03, 0.03, 0.035), (0.12, 0.13, 0.15), (0.28, 0.24, 0.18), (0.08, 0.1, 0.2), (0.2, 0.18, 0.14)]
SKINS = [(1.0, 0.9, 0.85), (0.85, 0.68, 0.58), (0.62, 0.45, 0.36), (0.38, 0.26, 0.2), (0.95, 0.8, 0.7), (0.72, 0.55, 0.45)]

def seats(mat_seat, mat_step, mat_rail):
    """Stepped concrete rows with moulded seats, over WIDTH*3 x (ROWS+4) rows, in slope space (x along, y up the slope)."""
    import bmesh
    bm = bmesh.new()
    def boxv(lo, hi, mi):
        r = bmesh.ops.create_cube(bm, size=1.0); vs = r["verts"]
        for v in vs:
            v.co = Vector((lo[i] + (hi[i] - lo[i]) * (v.co[i] + 0.5) for i in range(3)))
        for f in {f for v in vs for f in v.link_faces}: f.material_index = mi
    X0, X1 = -WIDTH * 1.5, WIDTH * 1.5
    for r in range(-3, ROWS + 3):
        y0, z0 = r * RUN, r * RISE
        boxv((X0, y0, z0 - RISE), (X1, y0 + RUN, z0), 1)                 # concrete step
        x = X0 + SEAT / 2
        while x < X1:
            boxv((x - 0.72, y0 + 0.9, z0 + 1.35), (x + 0.72, y0 + 2.0, z0 + 1.5), 0)          # seat pan
            boxv((x - 0.72, y0 + 1.95, z0 + 1.35), (x + 0.72, y0 + 2.1, z0 + 2.55), 0)        # seat back
            x += SEAT
    me = bpy.data.meshes.new("rows"); bm.to_mesh(me); bm.free()
    for m in (mat_seat, mat_step, mat_rail): me.materials.append(m)
    o = bpy.data.objects.new("rows", me); bpy.context.scene.collection.objects.link(o); return o

def mat(name, rgb, rough=0.7):
    m = bpy.data.materials.new(name); m.use_nodes = True; b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1); b.inputs["Roughness"].default_value = rough; return m

FAN_Y = float(os.environ.get("FAN_Y", "0.9"))

def render(src, variant):
    reset(); rnd = random.Random(100 + variant)
    kinds = ["sit", "sit", "sit", "clap", "cheer", "stand"]
    fans = {k: person(src, k) for k in ("sit", "clap", "cheer", "stand")}
    fans.update({k + "_bare": person(src, k, hat=False) for k in ("sit", "clap", "cheer", "stand")})
    for f in fans.values(): recolour(f)
    seat_col = [(0.05, 0.09, 0.22), (0.12, 0.12, 0.13), (0.22, 0.04, 0.04), (0.05, 0.09, 0.22)][variant]
    seats(mat("seat", seat_col, 0.45), mat("step", (0.36, 0.36, 0.35), 0.9), mat("rail", (0.5, 0.5, 0.5), 0.4))
    full = [0.97, 0.95, 0.9, 0.98][variant]                 # how full the stand is
    cheer = [0.08, 0.12, 0.05, 0.3][variant]
    row_seeds = [rnd.random() for _ in range(ROWS)]
    for r in range(-3, ROWS + 3):
        rr = random.Random(int(row_seeds[r % ROWS] * 1e9))
        y0, z0 = r * RUN, r * RISE; n = int(WIDTH / SEAT)
        for i in range(n):
            if rr.random() > full: continue
            bare = rr.random() < 0.7
            u = rr.random(); kind = "cheer" if u < cheer else "clap" if u < cheer + 0.08 else "stand" if u < cheer + 0.12 else "sit"
            for rep in (-1, 0, 1):
                x = -WIDTH / 2 + (i + 0.5) * SEAT + rep * WIDTH
                o = bpy.data.objects.new("f", fans[kind + ("_bare" if bare else "")].data); bpy.context.scene.collection.objects.link(o)
                o.location = (x + rr.uniform(-0.08, 0.08), y0 + FAN_Y, z0 + 1.45 if kind != "stand" else z0)
                o.rotation_euler = (0, 0, rr.uniform(-0.25, 0.25))
                o.scale = (rr.uniform(0.92, 1.06),) * 3
                o.pass_index = 0
            rr.random()
    # camera square to the slope, orthographic, one picture = WIDTH x ROWS rows
    sc = bpy.context.scene; cam = bpy.data.objects.new("c", bpy.data.cameras.new("c")); sc.collection.objects.link(cam); sc.camera = cam
    cam.data.type = "ORTHO"; cam.data.ortho_scale = WIDTH
    mid = Vector((0, ROWS * RUN / 2, ROWS * RISE / 2 + 1.6))
    n = Vector((0, -math.sin(SLOPE), math.cos(SLOPE)))      # slope normal (up and toward the floor)
    # look along the slope normal but tilted to the floor's view (from below, as fans are seen)
    view = Vector((0, -math.cos(0.45), math.sin(0.45)))
    cam.location = mid + view * 200; cam.rotation_euler = (-view).to_track_quat("-Z", "Y").to_euler()
    a = 0.45; per_row = RUN * math.sin(a) + RISE * math.cos(a)       # one row's height in the picture
    tall = ROWS * per_row
    sc.render.resolution_x = 2048; sc.render.resolution_y = round(2048 * tall / WIDTH)
    cam.data.ortho_scale = WIDTH; cam.data.sensor_fit = "HORIZONTAL"
    sc.render.engine = "CYCLES"; sc.cycles.samples = int(os.environ.get("SAMPLES", "48")); sc.cycles.use_denoising = True
    sc.view_settings.view_transform = "AgX"; sc.view_settings.look = "AgX - Base Contrast"
    w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True; w.node_tree.nodes["Background"].inputs[0].default_value = (0.35, 0.36, 0.4, 1)
    w.node_tree.nodes["Background"].inputs[1].default_value = 0.9
    for i, (rx, rz, e) in enumerate(((math.radians(35), math.radians(10), 3.4), (math.radians(60), math.radians(-150), 1.2))):
        bpy.ops.object.light_add(type="SUN"); l = bpy.context.object; l.rotation_euler = (rx, 0, rz); l.data.energy = e; l.data.angle = 0.3
    # height of the picture in the slope: ortho camera covers WIDTH x WIDTH; set rows to fit
    out = os.path.join(OUT, f"fp_crowd2_{variant}.png"); sc.render.filepath = out
    bpy.ops.render.render(write_still=True)
    from PIL import Image
    Image.open(out).convert("RGB").resize((2048, 2048), Image.LANCZOS).save(out); print("crowd", out, "rows", ROWS, "slope ft", ROWS * ROWLEN)

if __name__ == "__main__":
    src = sys.argv[1]; vs = [int(v) for v in sys.argv[2:]] or [0, 1, 2, 3]
    for v in vs: render(src, v)
