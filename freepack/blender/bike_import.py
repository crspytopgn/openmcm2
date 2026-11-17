"""Fit a third-party CC-BY/CC0 dirt bike model to the engine's bike frame
tree (see bike.py for the node list): scale it to the 4.97 ft wheelbase,
put the axles on the frame's pivots, and sort its pieces into the rigid
parts (wheels, forks, bars, swingarm, kickstart, body).

    python blender/bike_import.py SRC.glb MODEL_NAME [--front=-y]

Writes build/bikes/fp_bike_<MODEL_NAME>.glb. Pieces are sorted by where
they sit, so a model needs real separate parts for its wheels and forks.
"""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector, Matrix
from common import reset, BUILD
from bike import PIV, ORDER, PARENT, OUT, reg_uv, cell_uv, paint_livery, LIVERIES

def load(src):
    reset(); bpy.ops.import_scene.gltf(filepath=src)
    objs = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs: o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM"); bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if "--weld" in sys.argv:   # triangle-soup exports: weld so parts become whole pieces again
        vs = [v.co for o in objs for v in o.data.vertices]; size = max(max(v[i] for v in vs) - min(v[i] for v in vs) for i in range(3))
        for o in objs:
            bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.0023 * size); bm.to_mesh(o.data); bm.free()
    cap = int(next((x.split("=")[1] for x in sys.argv if x.startswith("--cap=")), "0"))
    if cap:   # thin out very dense pieces (springs, spokes) to keep the bike game-weight
        for o in objs:
            n = len(o.data.polygons)
            if n > cap:
                m = o.modifiers.new("d", "DECIMATE"); m.ratio = cap / n
                bpy.context.view_layer.objects.active = o; bpy.ops.object.modifier_apply(modifier="d")
    bpy.ops.object.join(); o = bpy.context.view_layer.objects.active
    for e in [x for x in bpy.context.scene.objects if x.type != "MESH"]: bpy.data.objects.remove(e)
    bpy.ops.object.mode_set(mode="EDIT"); bpy.ops.mesh.select_all(action="SELECT"); bpy.ops.mesh.separate(type="LOOSE"); bpy.ops.object.mode_set(mode="OBJECT")
    return [x for x in bpy.context.scene.objects if x.type == "MESH"]

def used_mats(o):
    """Material names actually used by the piece's faces (joined pieces keep every slot)."""
    ms = o.data.materials; return {ms[i].name for i in {f.material_index for f in o.data.polygons} if i < len(ms) and ms[i]}

def info(o):
    vs = [o.matrix_world @ v.co for v in o.data.vertices]
    lo = Vector(map(min, *vs)) if len(vs) > 1 else vs[0]; hi = Vector(map(max, *vs)) if len(vs) > 1 else vs[0]
    return lo, hi, (lo + hi) / 2

def fit(src, model, front_sign=-1):
    parts = load(src)
    I = {p.name: info(p) for p in parts}
    lo_all = Vector(map(min, *(i[0] for i in I.values()))); hi_all = Vector(map(max, *(i[1] for i in I.values())))
    blen = hi_all.y - lo_all.y; ymid = (lo_all.y + hi_all.y) / 2
    def circ(p):
        lo, hi, c = I[p.name]; d = hi - lo; m = max(d.y, d.z)
        return abs(d.y - d.z) < 0.06 * m and d.x < 0.45 * m and m > 0.2 * blen
    tm = next((x.split("=")[1] for x in sys.argv if x.startswith("--tyre=")), None)
    if tm:   # the model names its tyre material: the biggest tyre-material piece in each half is a tyre
        cands = sorted([p for p in parts if tm in used_mats(p)], key=lambda p: -(I[p.name][1].z - I[p.name][0].z))
    else:
        cands = sorted([p for p in parts if circ(p)], key=lambda p: -(I[p.name][1].z - I[p.name][0].z))
    fr = [p for p in cands if (I[p.name][2].y - ymid) * front_sign > 0]; rr = [p for p in cands if (I[p.name][2].y - ymid) * front_sign < 0]
    tyres = [rr[0], fr[0]]; rear, front = I[rr[0].name][2], I[fr[0].name][2]
    wb = (front - rear).length; s = 4.965 / wb
    # turn so the bike faces +Y (game forward), scale, put the rear axle on its pivot
    rot = Matrix.Rotation(0 if front_sign > 0 else math.pi, 4, "Z")
    r0 = rot @ rear
    T = Matrix.Translation(Vector((0, -2.657, 1.115)) - r0 * s) @ Matrix.Scale(s, 4) @ rot
    for p in parts: p.data.transform(T); p.matrix_world = Matrix()
    I = {p.name: info(p) for p in parts}
    RA = Vector((0, -2.657, 1.115)); FA = (T @ front); R = (I[tyres[0].name][1].z - I[tyres[0].name][0].z) * 0.5
    HP = Vector((0, 0.950, 3.701)); SP = Vector((0, -0.731, 1.619)); KP = Vector((0.381, -0.816, 1.592))
    ax = (FA - HP).normalized()
    tyre_mat = tm or max(((m.name, sum(1 for f in t.data.polygons if t.data.materials[f.material_index] == m)) for t in tyres for m in t.data.materials), key=lambda x: x[1])[0]
    def node_of(p):
        lo, hi, cen = I[p.name]; size = (hi - lo).length
        dF, dR = (cen - FA).length, (cen - RA).length
        # wheel parts: centred on an axle and within the tyre radius
        for A_, n_ in ((FA, "TireFront"), (RA, "TireRear")):
            if abs(lo.x) < 0.5 and abs(hi.x) < 0.5 and lo.y > A_.y - R * 1.18 and hi.y < A_.y + R * 1.18 and lo.z > A_.z - R * 1.18 and hi.z < A_.z + R * 1.18:
                # round things centred on the axle, or small bits (spokes, knobs): the wheel.
                # Off-centre larger pieces (fork guards, calipers) stay with the forks/swingarm.
                cc = Vector((0, cen.y - A_.y, cen.z - A_.z)).length
                spans = lo.y < A_.y - R * 0.3 and hi.y > A_.y + R * 0.3 and lo.z < A_.z - R * 0.3 and hi.z > A_.z + R * 0.3
                if spans or size < R * 0.35 or tyre_mat in (used_mats(p) if "--slot-wheels" not in sys.argv else [m.name for m in p.data.materials if m]): return n_
        # along the steering axis, ahead of the frame: forks, bars, plate, fender
        t = (cen - HP).dot(ax); off = (cen - HP - ax * t).length
        q = HP + ax * t   # nearest point on the steering axis: forks and bars sit on it or ahead of it
        if -0.6 < t < (FA - HP).length + 0.3 and off < 0.75 and cen.y > HP.y - 0.35 and cen.y > q.y - 0.3:
            return "Shocks" if t > (FA - HP).length * 0.55 else "HandleBars"
        if cen.z > HP.z - 0.2 and cen.y > HP.y - 0.5 and abs(cen.x) > 0.25: return "HandleBars"   # bar ends, levers, mirrors
        # under and behind the pivot, toward the rear axle: swingarm, chain
        if cen.y < SP.y - 0.05 and cen.y > RA.y - 0.4 and cen.z < SP.z + 0.35 and size < 4.0 and dR > R * 0.25:
            rel = (cen.y - RA.y) / (SP.y - RA.y); zline = RA.z + (SP.z - RA.z) * rel
            if abs(cen.z - zline) < 0.55: return "SwingArm"
        if (cen - KP).length < 0.45 and cen.x > 0.25 and size < 1.2 and not any("peg" in m for m in used_mats(p)): return "KickStart"
        return "Bike"
    groups = {}
    for p in parts:
        n = node_of(p); groups.setdefault(n, []).append(p)
        if os.environ.get("DBG") and n.startswith("Tire") and any(m.name == os.environ["DBG"] for m in p.data.materials):
            lo, hi, cen = I[p.name]; print("dbg", n, len(p.data.polygons), [round(x, 2) for x in lo], [round(x, 2) for x in hi], "R", round(R, 2))
    # wheels: centre each on its axle pivot and size it to touch the ground, so it spins true
    for n in ("TireFront", "TireRear"):
        ps = groups.get(n, [])
        if not ps: continue
        vs = [v.co for p in ps for v in p.data.vertices]; lo = Vector(map(min, *vs)); hi = Vector(map(max, *vs))
        c = (lo + hi) / 2; Rw = (hi.z - lo.z) / 2; piv = Vector((0, PIV[n][2], PIV[n][1]))
        M = Matrix.Translation(piv) @ Matrix.Scale(piv.z / Rw, 4) @ Matrix.Translation(-c)
        for p in ps: p.data.transform(M)
        print("wheel", n, "radius", round(Rw, 3), "->", round(piv.z, 3), "centre shift", tuple(round(x, 3) for x in (piv - c)))
    nodes = {}
    for n in ORDER:
        piv = Vector((PIV[n][0], PIV[n][2], PIV[n][1])); ps = groups.get(n, [])
        if not ps:
            e = bpy.data.objects.new(n, None); bpy.context.scene.collection.objects.link(e); e.location = piv; nodes[n] = e; continue
        bpy.ops.object.select_all(action="DESELECT")
        for p in ps: p.select_set(True)
        bpy.context.view_layer.objects.active = ps[0]
        if len(ps) > 1: bpy.ops.object.join()
        o = bpy.context.view_layer.objects.active; o.name = n; o.data.name = n
        o.data.transform(Matrix.Translation(-piv)); o.location = piv; nodes[n] = o
    bpy.context.view_layer.update()
    for n, p in PARENT.items():
        ch = nodes[n]; mw = ch.matrix_world.copy(); ch.parent = nodes[p]; ch.matrix_world = mw
    print("fit", model, "scale", round(s, 3), "front axle", tuple(round(x, 3) for x in FA), {k: len(v) for k, v in groups.items()})
    return nodes

def livery(nodes, paint, png):
    """Swap the model's paint for the shared livery: side-on projection into
    the atlas regions by where each face sits (see bike.py REG)."""
    img = bpy.data.images.load(png)
    L = bpy.data.materials.new("livery"); L.use_nodes = True; nt = L.node_tree; b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage"); t.image = img; nt.links.new(t.outputs["Color"], b.inputs["Base Color"])
    uvn = nt.nodes.new("ShaderNodeUVMap"); uvn.uv_map = "UVMap"; nt.links.new(uvn.outputs["UV"], t.inputs["Vector"])
    b.inputs["Roughness"].default_value = 0.45; b.inputs["Specular IOR Level"].default_value = 0.35
    for n, o in nodes.items():
        if o.type != "MESH": continue
        me = o.data; idx = [i for i, m in enumerate(me.materials) if m and m.name in paint.split(",")]
        if not idx: continue
        for i in idx: me.materials[i] = L
        uvl = me.uv_layers[0] if me.uv_layers else me.uv_layers.new(name="UVMap")
        while len(me.uv_layers) > 1: me.uv_layers.remove(me.uv_layers[-1])
        mw = o.matrix_world
        faces = [f for f in me.polygons if f.material_index in idx]
        def zone(f):
            c = mw @ f.center; nrm = (mw.to_3x3() @ f.normal).normalized()
            if n.startswith("Tire"): return "rim"
            if n == "Shocks" or n == "KickStart" or n == "SwingArm": return "main"
            if n == "HandleBars": return "plate" if (abs(nrm.y) > 0.55 and c.z > 2.6 and c.y < 1.6) else "ffender"
            if c.y > 0.35: return "shroud"
            if c.y > -2.0: return "side"
            return "rfender"
        Z = {}
        for f in faces: Z.setdefault(zone(f), []).append(f)
        for z, fs in Z.items():
            if z in ("rim", "main"):
                u = cell_uv(z)
                for f in fs:
                    for li in f.loop_indices: uvl.data[li].uv = u
                continue
            pts = [mw @ me.vertices[v].co for f in fs for v in f.vertices]
            if z == "plate":
                x0, x1 = min(p.x for p in pts), max(p.x for p in pts); a0, a1 = min(p.z for p in pts), max(p.z for p in pts)
                for f in fs:
                    for li in f.loop_indices:
                        p = mw @ me.vertices[me.loops[li].vertex_index].co
                        uvl.data[li].uv = reg_uv("plate", (p.x - x0) / (x1 - x0 + 1e-6), (p.z - a0) / (a1 - a0 + 1e-6))
                continue
            y0, y1 = min(p.y for p in pts), max(p.y for p in pts); a0, a1 = min(p.z for p in pts), max(p.z for p in pts)
            for f in fs:
                left = (mw @ f.center).x < 0
                for li in f.loop_indices:
                    p = mw @ me.vertices[me.loops[li].vertex_index].co
                    s_ = (p.y - y0) / (y1 - y0 + 1e-6); t_ = (p.z - a0) / (a1 - a0 + 1e-6)
                    uvl.data[li].uv = reg_uv(z, 1 - s_ if left else s_, t_)
        # number plates: the front plate and the flat side panels under the seat get the "plate"
        # material (same picture), where the engine prints the rider's number
        PL = bpy.data.materials.get("plate")
        if PL is None:   # plain plate background; the engine prints the number over the plate's own 0..1 UVs
            PL = bpy.data.materials.new("plate"); PL.use_nodes = True; pb = PL.node_tree.nodes["Principled BSDF"]
            pb.inputs["Base Color"].default_value = (0.9, 0.9, 0.88, 1); pb.inputs["Roughness"].default_value = 0.45
        if PL.name not in [m.name for m in me.materials if m]: me.materials.append(PL)
        pi = [m.name for m in me.materials].index(PL.name)
        side = [f for f in Z.get("side", []) if abs((mw.to_3x3() @ f.normal).normalized().x) > 0.6 and (mw @ f.center).y < -0.3]
        groups_ = [("front", Z.get("plate", [])), ("left", [f for f in side if (mw @ f.center).x < 0]), ("right", [f for f in side if (mw @ f.center).x >= 0])]
        for kind, fs in groups_:
            if not fs: continue
            pts = [mw @ me.vertices[v].co for f in fs for v in f.vertices]
            ax_u = (lambda p: p.x) if kind == "front" else (lambda p: p.y)
            u0, u1 = min(map(ax_u, pts)), max(map(ax_u, pts)); z0, z1 = min(p.z for p in pts), max(p.z for p in pts)
            for f in fs:
                f.material_index = pi
                for li in f.loop_indices:
                    p = mw @ me.vertices[me.loops[li].vertex_index].co
                    u = (ax_u(p) - u0) / (u1 - u0 + 1e-6)
                    if kind == "left": u = 1 - u      # read left to right from outside on both sides
                    if kind == "front": u = 1 - u     # the front plate is seen from ahead (looking back along -y)
                    uvl.data[li].uv = (u, (p.z - z0) / (z1 - z0 + 1e-6))

# real-world finishes for models that ship untextured, by material-name keyword (first match wins)
FINISH = {"sonnie": [
    (("rubber_tire", "grips", "cushon", "flap", "black", "seat", "fuel_cap", "box", "middle", "pump", "mian_cyl"), (0.025, 0.025, 0.025), 0.0, 0.75),
    (("chain",), (0.09, 0.08, 0.07), 0.9, 0.45), (("sporks", "bolts", "foot_pegs", "gear_lever", "breake_disc"), (0.62, 0.62, 0.6), 1.0, 0.3),
    (("metal_part_of_susp",), (0.85, 0.84, 0.8), 1.0, 0.12), (("fork_bottem", "bottem_forks", "caliper", "rim_part"), (0.035, 0.035, 0.04), 0.6, 0.35),
    (("spring",), (0.78, 0.5, 0.06), 0.7, 0.35), (("case",), (0.2, 0.2, 0.21), 0.8, 0.45), (("exahust", "fornt_pipe"), (0.07, 0.07, 0.075), 0.9, 0.5),
    (("radiator1",), (0.16, 0.16, 0.17), 0.8, 0.5), (("petrol_tank",), (0.78, 0.76, 0.7), 0.0, 0.35), (("button", "blue"), (0.5, 0.05, 0.04), 0.0, 0.4),
    (("frame", "swing_arm", "skid", "carb", "fork_holders", "handel_bar", "levers", "sproiket"), (0.6, 0.61, 0.62), 1.0, 0.33),
    (("blinn", "lambert"), (0.08, 0.08, 0.085), 0.3, 0.5)]}

def finish(key):
    for m in bpy.data.materials:
        for keys, rgb, metal, rough in FINISH.get(key, []):
            if any(k in m.name for k in keys) and m.node_tree and "Principled BSDF" in m.node_tree.nodes:
                b = m.node_tree.nodes["Principled BSDF"]; b.inputs["Base Color"].default_value = (*rgb, 1)
                b.inputs["Metallic"].default_value = metal; b.inputs["Roughness"].default_value = rough; break

if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    fs = -1 if "--front=-y" in sys.argv or "--front=+y" not in sys.argv else 1
    nodes = fit(a[0], a[1], fs)
    fk = next((x.split("=")[1] for x in sys.argv if x.startswith("--finish=")), None)
    if fk: finish(fk)
    paint = next((x.split("=")[1] for x in sys.argv if x.startswith("--paint=")), None)
    if paint:
        lv = next((x.split("=")[1] for x in sys.argv if x.startswith("--livery=")), "orange")
        livery(nodes, paint, os.path.join(OUT, f"fp_livery_{lv}.png"))
    path = os.path.join(OUT, f"fp_bike_{a[1]}.glb")
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
    print("wrote", path)
