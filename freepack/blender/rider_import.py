"""Free Pack rider from a realistic CC-BY human: poses a skinned base body
to the engine's 16-part rider frame, cuts it into rigid parts, turns the
clothes into motocross kit (jersey, pants, boots, gloves) on the shared
outfit atlas and adds our own helmet, goggles and neck brace.

    python blender/rider_import.py SRC.glb

Base body: "Hight Quality Realistic Man Character" by hammer.gamedev (CC-BY 4.0,
sketchfab 10cbeabee635483fae932f9e49f3cf7b). See CREDITS.
Writes build/riders/fp_rider.glb and build/riders/fp_rider_<outfit>.png.
"""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector, Matrix
from common import reset, BUILD
from rider import G, ORDER, PARENT, P, REG, CELLS, reg_uv, cell_uv, OUTFITS, paint_outfit, outfit_mat, OUT, A

M2F = 3.28084
K = 0.949                      # body scale so the hips sit at the frame's hip height
F = M2F * K
KEEP = ("Std_Skin_Head", "Std_Skin_Arm", "Std_Skin_Body", "Plaid_Punk_Shirt", "Slim_Jeans", "Sport_sneakers")

def node_of(bone):
    s = "L" if "_L_" in bone else "R" if "_R_" in bone else ""
    if "Toe" in bone or "_Foot" in bone: return "Foot" + s
    if "Calf" in bone or "Knee" in bone: return "LwrLeg" + s
    if "Thigh" in bone: return "UprLeg" + s
    if any(k in bone for k in ("Hand", "Thumb", "Index", "Mid", "Ring", "Pinky")): return "Hand" + s
    if "Forearm" in bone or "Elbow" in bone: return "LwrArm" + s
    if "Upperarm" in bone or "Shoulder" in bone: return "UprArm" + s
    if any(k in bone for k in ("Neck", "Head", "Facial", "Jaw", "Teeth", "Tongue", "Eye", "Lip", "Cheek", "Nose", "Brow", "Mouth", "Ear", "Hat")): return "Head"
    if "Spine02" in bone or "Clavicle" in bone or "Breast" in bone or "Ribs" in bone: return "UprTorso"
    if "Spine01" in bone or "Waist" in bone: return "LwrTorso"
    return "Pelvis"

def load(src):
    reset(); bpy.ops.import_scene.gltf(filepath=src)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    for o in list(bpy.data.objects):
        if o.type == "MESH" and not (o.data.materials and o.data.materials[0] and o.data.materials[0].name in KEEP):
            bpy.data.objects.remove(o, do_unlink=True)
    root = arm
    while root.parent: root = root.parent
    root.matrix_world = Matrix.Rotation(math.pi, 4, "Z") @ Matrix.Scale(F, 4) @ root.matrix_world
    bpy.context.view_layer.update(); return arm

def wpos(arm, name, tail=False):
    pb = arm.pose.bones[name]; return arm.matrix_world @ (pb.tail if tail else pb.head)

def aim(arm, name, target):
    pb = arm.pose.bones[name]; mw = arm.matrix_world @ pb.matrix
    h = mw.translation.copy(); cur = (arm.matrix_world @ pb.tail) - h
    R = cur.rotation_difference(target - h).to_matrix().to_4x4()
    pb.matrix = arm.matrix_world.inverted() @ (Matrix.Translation(h) @ R @ Matrix.Translation(-h) @ mw)
    bpy.context.view_layer.update()

CHAIN = {"L": ("CC_Base_L_Upperarm", "CC_Base_L_Forearm", "CC_Base_L_Hand", "CC_Base_L_Thigh", "CC_Base_L_Calf", "CC_Base_L_Foot"),
         "R": ("CC_Base_R_Upperarm", "CC_Base_R_Forearm", "CC_Base_R_Hand", "CC_Base_R_Thigh", "CC_Base_R_Calf", "CC_Base_R_Foot")}

def bone(arm, prefix): return next(b.name for b in arm.pose.bones if b.name.startswith(prefix + "_"))

def pose(arm):
    J = {}
    for s in "LR":
        ua, fa, hd, th, ca, ft = (bone(arm, p) for p in CHAIN[s])
        aim(arm, ua, P("LwrArm" + s)); aim(arm, fa, P("Hand" + s))
        aim(arm, th, P("LwrLeg" + s)); aim(arm, ca, P("Foot" + s))
        J["sh" + s], J["el" + s], J["wr" + s] = wpos(arm, ua), wpos(arm, fa), wpos(arm, hd)
        J["hp" + s], J["kn" + s], J["an" + s] = wpos(arm, th), wpos(arm, ca), wpos(arm, ft)
    J["head"] = wpos(arm, bone(arm, "CC_Base_Head"))
    return J

NECK = [None]   # base of the skull on the fitted body (the helmet and brace sit on it)

def seg_map(a, b, A_, B_):
    d = (b - a).normalized(); s = (B_ - A_).length / (b - a).length
    S = Matrix.Identity(3) + (s - 1) * Matrix(((d.x * d.x, d.x * d.y, d.x * d.z), (d.y * d.x, d.y * d.y, d.y * d.z), (d.z * d.x, d.z * d.y, d.z * d.z)))
    R = (b - a).rotation_difference(B_ - A_).to_matrix()
    return Matrix.Translation(A_) @ (R @ S).to_4x4() @ Matrix.Translation(-a)

def bake_and_split(arm):
    """Bake the pose into each kept mesh and cut it into per-node pieces."""
    dg = bpy.context.evaluated_depsgraph_get(); pieces = []
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        gname = {g.index: g.name for g in o.vertex_groups}
        vnode = []
        for v in o.data.vertices:
            acc = {}
            for g in v.groups:
                if g.group in gname: n = node_of(gname[g.group]); acc[n] = acc.get(n, 0) + g.weight
            vnode.append(max(acc, key=acc.get) if acc else "Pelvis")
        me = bpy.data.meshes.new_from_object(o.evaluated_get(dg)); me.transform(o.matrix_world)
        mat = o.data.materials[0].name
        amt = {"Plaid_Punk_Shirt": 0.05, "Slim_Jeans": 0.04, "Sport_sneakers": 0.05}.get(mat, 0.0)
        if mat == "Std_Skin_Arm": push_pos(me, lambda i: 0.018 if vnode[i].startswith("Hand") else 0.05)
        elif amt: push_pos(me, lambda i: amt)
        for n in ORDER:
            bm = bmesh.new(); bm.from_mesh(me); bm.verts.ensure_lookup_table()
            kill = []
            for f in bm.faces:
                votes = {}
                for v in f.verts: votes[vnode[v.index]] = votes.get(vnode[v.index], 0) + 1
                if max(votes, key=votes.get) != n: kill.append(f)
            bmesh.ops.delete(bm, geom=kill, context="FACES")
            if not bm.faces: bm.free(); continue
            m2 = bpy.data.meshes.new(f"{n}_{mat}"); bm.to_mesh(m2); bm.free()
            ob = bpy.data.objects.new(m2.name, m2); bpy.context.scene.collection.objects.link(ob)
            ob["node"] = n; ob["src"] = mat; pieces.append(ob)
    for o in [o for o in bpy.data.objects if o.type in ("MESH", "ARMATURE", "EMPTY") and "node" not in o]:
        bpy.data.objects.remove(o, do_unlink=True)
    return pieces

def fit_limbs(pieces, J):
    """Stretch each limb piece so its joints land exactly on the frame's pivots."""
    T = {}
    for s in "LR":
        T["UprArm" + s] = seg_map(J["sh" + s], J["el" + s], P("UprArm" + s), P("LwrArm" + s))
        T["LwrArm" + s] = seg_map(J["el" + s], J["wr" + s], P("LwrArm" + s), P("Hand" + s))
        T["Hand" + s] = Matrix.Translation(P("Hand" + s)) @ T["LwrArm" + s].to_3x3().normalized().to_4x4() @ Matrix.Translation(-J["wr" + s])
        T["UprLeg" + s] = seg_map(J["hp" + s], J["kn" + s], P("UprLeg" + s), P("LwrLeg" + s))
        T["LwrLeg" + s] = seg_map(J["kn" + s], J["an" + s], P("LwrLeg" + s), P("Foot" + s))
        T["Foot" + s] = Matrix.Translation(P("Foot" + s) - J["an" + s])
    t = (P("UprLegL") + P("UprLegR")) / 2 - (J["hpL"] + J["hpR"]) / 2
    NECK[:] = [J["head"] + t]
    for n in ("Pelvis", "LwrTorso", "UprTorso", "Head"): T[n] = Matrix.Translation(t)
    for o in pieces:
        if o["node"] in T: o.data.transform(T[o["node"]])

# ---------------- kit ----------------
def axis_of(n):
    kids = [c for c, p in PARENT.items() if p == n and c[:3] == n[:3]] or [c for c, p in PARENT.items() if p == n]
    a = P(n)
    if n.startswith("Hand"): return a, a + (a - P(PARENT[n])).normalized() * 0.6
    if n.startswith("Foot"): return a, a + Vector((0, 0, -0.33))
    if n in ("Pelvis", "LwrTorso", "UprTorso"): return Vector((0, P(n).y, 2.6)), Vector((0, P(n).y, 5.0))
    if n == "Head": return P("Head"), P("Head") + Vector((0, 0, 1))
    return a, P(kids[0])

def cyl_uv(co, a, b, seam_front=0.25):
    d = (b - a); L = d.length; d = d / L; fwd = Vector((0, 1, 0)); fwd = (fwd - d * fwd.dot(d))
    if fwd.length < 1e-4: fwd = Vector((0, 0, 1)) - d * d.z
    fwd.normalize(); side = d.cross(fwd)
    p = co - a; t = p.dot(d) / L; q = p - d * p.dot(d)
    ang = math.atan2(q.dot(side), q.dot(fwd)); return (seam_front + ang / (2 * math.pi)) % 1.0, t

def map_faces(me, fn):
    uv = me.uv_layers[0] if me.uv_layers else me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        st = [fn(me.vertices[me.loops[li].vertex_index].co, poly) for li in poly.loop_indices]
        us = [x[0] for x in st]
        if max(us) - min(us) > 0.5: st = [((u + 1) if u < 0.5 else u, t) for u, t in st]; st = [(min(u, 1.0), t) for u, t in st]
        for li, (u, t) in zip(poly.loop_indices, st): uv.data[li].uv = u, t

def push_pos(me, amount):
    """Inflate along normals averaged over coincident vertices, so split seams stay closed."""
    acc = {}
    for v in me.vertices:
        k = (round(v.co.x * 2000), round(v.co.y * 2000), round(v.co.z * 2000)); acc[k] = acc.get(k, Vector()) + v.normal
    for v in me.vertices:
        k = (round(v.co.x * 2000), round(v.co.y * 2000), round(v.co.z * 2000)); n = acc[k]
        if n.length > 1e-8: v.co += n.normalized() * amount(v.index)

def weld(me, keep=None):
    bm = bmesh.new(); bm.from_mesh(me)
    if keep:
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if not keep(f)], context="FACES")
    bm.to_mesh(me); bm.free(); me.update()

def push(me, amount, mask=None):
    nrm = [v.normal.copy() for v in me.vertices]
    for v, n in zip(me.vertices, nrm):
        if mask is None or mask(v.co): v.co += n * amount

def reloft(me, a, b, nr=18, ns=36, pct=75, trim=(3, 97), grow=0.0, pad=0.04, smooth=2, neck=None):
    """Replace a clothing piece by a clean tube that follows its shape:
    the radius around the a->b axis per (height, angle) bin, outliers
    (buttons, collar, ragged hems) cut by a percentile and smoothed.
    Opening ends are trimmed straight."""
    import numpy as np
    d = (b - a); L = d.length; d = d / L
    fwd = Vector((0, 1, 0)) - d * d.y
    if fwd.length < 1e-4: fwd = Vector((0, 0, 1)) - d * d.z
    fwd.normalize(); side = d.cross(fwd)
    V = np.array([v.co[:] for v in me.vertices]) if me.vertices else None
    if V is None or len(V) < 30: return False
    A = np.array(a[:]); D = np.array(d[:]); F = np.array(fwd[:]); S = np.array(side[:])
    P_ = V - A; t = P_ @ D; Q = P_ - np.outer(t, D)
    ang = np.arctan2(Q @ S, Q @ F) % (2 * np.pi); r = np.hypot(Q @ S, Q @ F)
    t0, t1 = np.percentile(t, trim[0]) - pad, np.percentile(t, trim[1]) + pad
    R = np.full((nr, ns), np.nan)
    ti = np.clip(((t - t0) / (t1 - t0) * nr).astype(int), 0, nr - 1); ai = (ang / (2 * np.pi) * ns).astype(int) % ns
    for i in range(nr):
        for j in range(ns):
            m = (ti == i) & (ai == j)
            if m.sum(): R[i, j] = np.percentile(r[m], pct)
    for i in range(nr):                          # fill gaps around each ring
        row = R[i]; ok = ~np.isnan(row)
        if ok.sum() >= 3:
            xs = np.arange(ns); R[i] = np.interp(xs, xs[ok], row[ok], period=ns)
    for j in range(ns):                          # then along the tube
        col = R[:, j]; ok = ~np.isnan(col)
        if ok.sum(): R[:, j] = np.interp(np.arange(nr), np.arange(nr)[ok], col[ok])
        else: R[:, j] = np.nanmean(R)
    for _ in range(smooth):
        R = (R + np.roll(R, 1, 1) + np.roll(R, -1, 1)) / 3
        R[1:-1] = (R[1:-1] * 2 + R[:-2] + R[2:]) / 4
    R += grow
    if neck:                                     # slope the shoulders in to the neck
        for k, w in zip(range(nr - 4, nr), (0.15, 0.4, 0.65, 0.85)): R[k] = R[k] * (1 - w) + neck * w
    bm = bmesh.new(); rings = []
    for i in range(nr + 1):
        ii = min(i, nr - 1); tt = t0 + (t1 - t0) * i / nr
        ring = []
        for j in range(ns):
            th = 2 * np.pi * (j + 0.5) / ns
            ring.append(bm.verts.new(a + d * tt + (fwd * math.cos(th) + side * math.sin(th)) * float(R[ii, j] if i < nr else R[nr - 1, j])))
        rings.append(ring)
    for i in range(nr):
        for j in range(ns):
            bm.faces.new((rings[i][j], rings[i][(j + 1) % ns], rings[i + 1][(j + 1) % ns], rings[i + 1][j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    c = bm.faces[0].calc_center_median(); n0 = bm.faces[0].normal
    axis_pt = a + d * (c - a).dot(d)
    if n0.dot(c - axis_pt) < 0:
        for f in bm.faces: f.normal_flip()
    me.clear_geometry(); bm.to_mesh(me); bm.free()
    for poly in me.polygons: poly.use_smooth = True
    if not me.uv_layers: me.uv_layers.new(name="UVMap")
    return True

def dress(pieces, O, dark):
    for o in pieces:
        me = o.data; n = o["node"]; src = o["src"]
        while len(me.uv_layers) > 1: me.uv_layers.remove(me.uv_layers[-1])
        if me.uv_layers: me.uv_layers[0].name = "UVMap"
        me.materials.clear(); me.materials.append(O)
        a, b = axis_of(n)
        nk = NECK[0]
        if src == "Plaid_Punk_Shirt" and n == "Head":
            me.clear_geometry(); continue
        if src == "Plaid_Punk_Shirt":
            weld(me, lambda f: not (f.calc_center_median().z > nk.z - 0.42 and math.hypot(f.calc_center_median().x, f.calc_center_median().y - nk.y) < 0.42))
        elif src == "Std_Skin_Head":
            weld(me, lambda f: f.calc_center_median().z < nk.z - 0.05)
        else:
            weld(me)
        if not me.polygons: continue
        if os.environ.get("FP_RELOFT", "1") == "1":
            if src == "Plaid_Punk_Shirt" and n.startswith(("UprLeg", "LwrLeg", "Pelvis")):
                me.clear_geometry(); continue          # loose shirt tails over the thighs
            if src == "Plaid_Punk_Shirt" and n in ("LwrTorso", "UprTorso"):
                cx = sum(v.co.x for v in me.vertices) / len(me.vertices); cy = sum(v.co.y for v in me.vertices) / len(me.vertices)
                reloft(me, Vector((0, cy, 2.0)), Vector((0, cy, 5.6)), nr=16 if n != "UprTorso" else 22, ns=40, trim=(0, 98) if n != "UprTorso" else (2, 99.5), pad=0.04 if n == "UprTorso" else 0.12, neck=0.42 if n == "UprTorso" else None)
            elif src == "Plaid_Punk_Shirt" and n.startswith(("UprArm", "LwrArm")):
                ka, kb = axis_of(n); reloft(me, ka, kb, nr=12, ns=24)
            elif src == "Std_Skin_Arm" and n.startswith("LwrArm"):
                ka, kb = axis_of(n); reloft(me, ka, kb, nr=12, ns=24)
            elif src == "Slim_Jeans" and n.startswith(("UprLeg", "LwrLeg")):
                ka, kb = axis_of(n); reloft(me, ka, kb, nr=14, ns=28, pct=85)
        if src == "Plaid_Punk_Shirt":
            if not n.startswith(("UprArm", "LwrArm", "Hand")):
                lo, hi = 2.85, 4.95
                map_faces(me, lambda co, p: reg_uv("jersey", cyl_uv(co, Vector((0, -0.4, 0)), Vector((0, -0.4, 1)))[0], min(max((co.z - lo) / (hi - lo), 0.0), 1.0)))
            else:
                L = (b - a).length if not n.startswith("Lwr") else None
                sh, wr = P("UprArm" + n[-1]), P("Hand" + n[-1])
                map_faces(me, lambda co, p: reg_uv("sleeve", cyl_uv(co, sh, wr)[0], 1 - min(max(cyl_uv(co, sh, wr)[1], 0), 1)))
        elif src == "Std_Skin_Arm":
            if n.startswith("Hand"):
                wr = P("Hand" + n[-1])
                map_faces(me, lambda co, p: reg_uv("glove", cyl_uv(co, wr, wr + (wr - P("LwrArm" + n[-1])).normalized() * 0.65)[0], 1 - min(max(cyl_uv(co, wr, wr + (wr - P("LwrArm" + n[-1])).normalized() * 0.65)[1], 0), 1)))
            else:   # bare forearm: long jersey sleeve over it
                sh, wr = P("UprArm" + n[-1]), P("Hand" + n[-1])
                map_faces(me, lambda co, p: reg_uv("sleeve", cyl_uv(co, sh, wr)[0], 1 - min(max(cyl_uv(co, sh, wr)[1], 0), 1)))
        elif src == "Slim_Jeans":
            s = n[-1] if n[-1] in "LR" else ("L" if 0 else "R")
            hp, an = P("UprLeg" + (n[-1] if n[-1] in "LR" else "L")), P("Foot" + (n[-1] if n[-1] in "LR" else "L"))
            boot_top = P("Foot" + (n[-1] if n[-1] in "LR" else "L")).z + 1.25
            def pants(co, p, hp=hp, an=an, bt=boot_top):
                hpx = Vector((hp.x if co.x * hp.x > 0 else -hp.x, hp.y, hp.z)); anx = Vector((an.x if co.x * an.x > 0 else -an.x, an.y, an.z))
                u, t = cyl_uv(co, hpx, anx)
                if co.z < bt: return cell_uv("white") if False else reg_uv("boot", u, (co.z - an.z) / (bt - an.z))
                return reg_uv("pants", u, 1 - min(max(t, 0), 1))
            if n.startswith("LwrLeg"):     # boot shaft: rigid, wider tube from ankle to below the knee
                ax0, ax1 = P("Foot" + n[-1]), P("LwrLeg" + n[-1])
                d = (ax1 - ax0).normalized()
                for v in me.vertices:
                    if v.co.z < boot_top:
                        c = ax0 + d * (v.co - ax0).dot(d); r = v.co - c; rl = r.length
                        k = min(max((boot_top - v.co.z) / 0.25, 0), 1)
                        tgt = max(rl * 1.12 + 0.05, 0.24 if v.co.z > ax0.z + 0.35 else 0.0)
                        if rl > 1e-4: v.co = c + r * ((rl + (tgt - rl) * k) / rl)
            map_faces(me, pants)
        elif src == "Sport_sneakers":   # boot foot: chunkier with a dark sole
            an = P("Foot" + n[-1])
            for v in me.vertices: v.co = an + Matrix.Diagonal((1.12, 1.06, 1.0)) @ (v.co - an)
            def boot(co, p, an=an):
                if p.normal.z < -0.6 or co.z < 0.07: return cell_uv("sole")
                u, _ = cyl_uv(co, an + Vector((0, 0, 0.4)), an + Vector((0, 0, -0.4)))
                return reg_uv("boot", u, min(max((co.z - (an.z - 0.33)) / 1.6, 0), 1) * 0.3)
            map_faces(me, boot)
        elif src == "Std_Skin_Head":
            for f in me.polygons: f.use_smooth = True
            map_faces(me, lambda co, p: cell_uv("skin"))
        else:   # neck skin under the brace
            map_faces(me, lambda co, p: cell_uv("dark"))

def lofted(node, ring_fn, n_rings, n_seg, mats, uv_fn, smooth=True, name="part"):
    """Mesh from rings: ring_fn(i) -> list of n_seg Vectors."""
    bm = bmesh.new(); rings = [[bm.verts.new(p) for p in ring_fn(i)] for i in range(n_rings)]
    for i in range(n_rings - 1):
        for j in range(n_seg):
            a, b = rings[i][j], rings[i][(j + 1) % n_seg]; c, d = rings[i + 1][(j + 1) % n_seg], rings[i + 1][j]
            try: bm.faces.new((a, b, c, d))
            except ValueError: pass
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = smooth
    for m in mats: me.materials.append(m)
    uv = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        for li in poly.loop_indices: uv.data[li].uv = uv_fn(me.vertices[me.loops[li].vertex_index].co, poly)
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); o["node"] = node; o["src"] = name
    return o

def helmet(O, lens, dark):
    """Off-road helmet: long shell, chin bar, peak, goggles and strap."""
    hc = NECK[0] + Vector((0, 0.1, 0.22))          # centre of the head
    rx, ry, rz = 0.5, 0.64, 0.58
    seg, rings = 48, 28; parts = []
    def shell_pt(th, ph):    # th: polar from top, ph: azimuth (0 = front)
        x = math.sin(th) * math.sin(ph); y = math.sin(th) * math.cos(ph); z = math.cos(th)
        fy = max(0.0, y)
        y *= 1.0 + 0.1 * fy
        y += 0.8 * fy ** 3 * max(0.0, 0.05 - z) * (1.0 - 0.5 * abs(x))   # chin bar beak juts forward
        zz = z
        if z < -0.35:                                                 # lower edge: short at the back, chin drops at the front
            zz = -0.35 + (z + 0.35) * (0.5 + 0.6 * fy)
        return hc + Vector((x * rx, y * ry, zz * rz))
    def huv(co, p):
        q = co - hc
        if q.y > 0.55 * ry and -0.12 * rz < q.z < 0.34 * rz and abs(q.x) < 0.62 * rx: return cell_uv("dark")   # eye port
        q = co - hc; ang = math.atan2(q.x, q.y); u = (0.5 + ang / (2 * math.pi)) % 1.0
        return reg_uv("helmet", u, min(max((q.z / rz + 0.6) / 1.6, 0), 1))
    sh = lofted("Head", lambda i: [shell_pt(0.02 + (math.pi * 0.86) * i / (rings - 1), 2 * math.pi * j / seg) for j in range(seg)], rings, seg, [O], huv, name="helmet")
    # close the top cap
    bm = bmesh.new(); bm.from_mesh(sh.data); bm.verts.ensure_lookup_table()
    top = [v for v in bm.verts if (v.co - hc).z > rz * 0.97]
    if len(top) >= 3: bmesh.ops.contextual_create(bm, geom=top)
    bm.to_mesh(sh.data); bm.free(); parts.append(sh)
    # vents on the chin bar
    for k in (-1, 0, 1):
        bpy.ops.mesh.primitive_cube_add(size=1, location=hc + Vector((k * 0.1, ry * 1.32, -0.3 * rz)))
        v = bpy.context.active_object; v.scale = (0.07, 0.05, 0.12); v.rotation_euler = (0.5, 0, 0); v["node"] = "Head"; v["src"] = "vent"
        v.data.materials.append(dark); bpy.ops.object.transform_apply(location=False, rotation=True, scale=True); parts.append(v)
    # peak: curved visor over the forehead
    bm = bmesh.new(); PR = []
    for i in range(6):
        t = i / 5; row = []
        for j in range(17):
            a = math.pi * (0.22 + 0.56 * j / 16)
            w = rx * (1.02 - 0.15 * t); x = math.cos(a) * w
            y = math.sin(a) * ry * 0.95 + 0.38 * t
            z = 0.4 * rz + 0.2 * t - 0.12 * (x / rx) ** 2
            row.append(bm.verts.new(hc + Vector((x, y, z))))
        PR.append(row)
    for i in range(5):
        for j in range(16): bm.faces.new((PR[i][j], PR[i][j + 1], PR[i + 1][j + 1], PR[i + 1][j]))
    me = bpy.data.meshes.new("peak"); bm.to_mesh(me); bm.free(); me.materials.append(O)
    pk = bpy.data.objects.new("peak", me); bpy.context.scene.collection.objects.link(pk); pk["node"] = "Head"; pk["src"] = "peak"
    m = pk.modifiers.new("s", "SOLIDIFY"); m.thickness = 0.035
    uv = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        for li in poly.loop_indices:
            co = me.vertices[me.loops[li].vertex_index].co
            uv.data[li].uv = cell_uv("accent") if (co - hc).y > ry * 1.25 else huv(co, poly)
    for p in me.polygons: p.use_smooth = True
    parts.append(pk)
    # goggles: frame, lens and strap
    def gog(i, rr, zc, h):
        t = i / 1
        return [hc + Vector((math.cos(a) * rx * rr, math.sin(a) * ry * rr, zc + (t - 0.5) * h)) for a in [math.pi * (0.18 + 0.64 * j / 23) for j in range(24)]]
    for name, rr, h, mt, cell in (("goggle_frame", 1.05, 0.34, dark, "dark"), ("goggle_lens", 1.08, 0.25, lens, "dark")):
        bm = bmesh.new(); R = [[bm.verts.new(p) for p in gog(i, rr, 0.12 * rz, h)] for i in range(2)]
        for j in range(23): bm.faces.new((R[0][j], R[0][j + 1], R[1][j + 1], R[1][j]))
        me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free(); me.materials.append(mt)
        uv = me.uv_layers.new(name="UVMap")
        for poly in me.polygons:
            for li in poly.loop_indices: uv.data[li].uv = cell_uv(cell)
        for p in me.polygons: p.use_smooth = True
        g = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(g); g["node"] = "Head"; g["src"] = name
        s = g.modifiers.new("s", "SOLIDIFY"); s.thickness = 0.03 if mt is dark else 0.01; parts.append(g)
    def strap(i):
        return [hc + Vector((math.cos(a) * rx * 1.01, math.sin(a) * ry * 1.0, 0.12 * rz + (i - 0.5) * 0.12)) for a in [math.pi * (-0.82 + 0.82 * 2 * j / 39 * 0.0 + 0) + 0 for j in range(1)]] if False else \
               [hc + Vector((math.cos(a) * rx * 1.015, math.sin(a) * ry * 1.005, 0.12 * rz + (i - 0.5) * 0.12)) for a in [math.pi * (0.82 + 1.36 * j / 39) for j in range(40)]]
    bm = bmesh.new(); R = [[bm.verts.new(p) for p in strap(i)] for i in range(2)]
    for j in range(39): bm.faces.new((R[0][j], R[0][j + 1], R[1][j + 1], R[1][j]))
    me = bpy.data.meshes.new("strap"); bm.to_mesh(me); bm.free(); me.materials.append(O)
    uv = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        for li in poly.loop_indices: uv.data[li].uv = cell_uv("strap")
    st = bpy.data.objects.new("strap", me); bpy.context.scene.collection.objects.link(st); st["node"] = "Head"; st["src"] = "strap"
    st.modifiers.new("s", "SOLIDIFY").thickness = 0.015; parts.append(st)
    # neck brace on the shoulders
    nb = NECK[0] + Vector((0, -0.03, -0.25))
    bpy.ops.mesh.primitive_torus_add(major_radius=0.34, minor_radius=0.075, major_segments=32, minor_segments=10, location=nb)
    t = bpy.context.active_object; t.scale = (1.0, 1.08, 0.8); t.rotation_euler = (-0.2, 0, 0); t["node"] = "UprTorso"; t["src"] = "brace"
    t.data.materials.append(O); bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
    uv = t.data.uv_layers[0]
    for li in range(len(uv.data)): uv.data[li].uv = cell_uv("brace")
    for p in t.data.polygons: p.use_smooth = True
    parts.append(t)
    return parts

def paint(name):
    """The shared outfit picture, with our helmet layout on top."""
    from PIL import Image, ImageDraw
    p = paint_outfit(name); im = Image.open(p).convert("RGB"); d = ImageDraw.Draw(im)
    main, acc, pants, helm, boot, num = OUTFITS[name]
    C = lambda c: tuple(int(v * 255) for v in c)
    x0, y0, x1, y1 = REG["jersey"]          # plain collar and hem so loose fabric edges don't flash the accent
    d.rectangle([x0, y0, x1, y0 + 70], fill=C(main)); d.rectangle([x0, y1 - 75, x1, y1], fill=C(main))
    x0, y0, x1, y1 = REG["helmet"]; W = x1 - x0; H = y1 - y0
    # u: 0.5 front, 0.25 / 0.75 the sides, 0 / 1 the back; rows: top of the shell at y0
    import random as _r
    rr = _r.Random(name); two = sum(abs(a_ - b_) for a_, b_ in zip(main, helm)) > 0.3
    panel = main if two else acc
    d.rectangle([x0, y0, x1, y1], fill=C(helm))
    X = lambda u: x0 + int(u * W); Y = lambda t: y1 - int(t * H)      # t: 0 bottom edge, 1 crown
    for c in (0.25, 0.75):                                             # big swept side panels, rising toward the back
        sgn = 1 if c < 0.5 else -1
        pts = [(X(c - 0.2 * sgn), Y(0.0)), (X(c + 0.22 * sgn), Y(0.0)), (X(c + 0.2 * sgn), Y(0.18)), (X(c - 0.05 * sgn), Y(0.62)), (X(c - 0.2 * sgn), Y(0.72))]
        d.polygon(pts, fill=C(panel))
        d.line(pts[2:] + [pts[2]], fill=C(acc), width=10)                 # pinstripe along the panel edge
        d.line([(X(c + 0.12 * sgn), Y(0.06)), (X(c - 0.1 * sgn), Y(0.5))], fill=C(acc), width=16)
    d.rectangle([X(0.47), y0, X(0.53), Y(0.25)], fill=C(panel))          # centre stripe over the crown
    d.rectangle([X(0.485), y0, X(0.515), Y(0.25)], fill=C(acc))
    d.rectangle([x0, Y(0.08), x1, y1], fill=(24, 24, 26))                  # rubber edge trim
    from PIL import ImageFont as _F
    f = _F.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 24)
    for c in (0.25, 0.75):                                             # made-up maker name on each side
        txt = rr.choice(["KESTREL", "HALCYON", "RIDGEBACK", "COPPERHEAD", "NORTHFORK"])
        tw = d.textlength(txt, font=f); d.text((X(c) - tw / 2, Y(0.36)), txt, font=f, fill=C(acc) if two else (20, 20, 22))
    tw = d.textlength(num, font=_F.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 70))
    d.text((X(0.0) + 20, Y(0.9)), num, font=_F.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 70), fill=C(panel))
    # boots: shell colour, dark cuff, three buckle straps wrapping round from the front
    x0, y0, x1, y1 = REG["boot"]; W = x1 - x0; H = y1 - y0
    d.rectangle([x0, y0, x1, y1], fill=C(boot))
    d.rectangle([x0, y0, x1, y0 + int(H * 0.07)], fill=(22, 22, 24))
    d.rectangle([x0, y1 - int(H * 0.06), x1, y1], fill=(16, 16, 16))
    for t in (0.33, 0.55, 0.77):
        yy = y1 - int(t * H)
        for ua, ub in ((0.30, 0.56), (0.94, 1.0), (0.0, 0.2)):
            d.rectangle([x0 + int(ua * W), yy - 9, x0 + int(ub * W), yy + 9], fill=(30, 30, 32))
        for ub in (0.56, 0.94):
            xb = x0 + int(ub * W); d.rectangle([xb - 12, yy - 13, xb + 12, yy + 13], fill=(150, 152, 156))
    im.save(p); return p

def assemble():
    bpy.context.view_layer.update()
    for o in [o for o in bpy.context.scene.objects if o.type == "MESH"]:
        bpy.ops.object.select_all(action="DESELECT"); o.select_set(True); bpy.context.view_layer.objects.active = o
        if o.modifiers: bpy.ops.object.convert(target="MESH")
    nodes = {}
    for n in ORDER:
        ps = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.get("node") == n]; piv = P(n)
        bpy.ops.object.select_all(action="DESELECT")
        for o in ps: o.select_set(True)
        bpy.context.view_layer.objects.active = ps[0]
        if len(ps) > 1: bpy.ops.object.join()
        o = bpy.context.view_layer.objects.active; o.name = n; o.data.name = n
        while len(o.data.uv_layers) > 1: o.data.uv_layers.remove(o.data.uv_layers[-1])
        o.data.uv_layers[0].name = "UVMap"; o.data.uv_layers[0].active = True; o.data.uv_layers[0].active_render = True
        for a in [a for a in o.data.color_attributes]: o.data.color_attributes.remove(a)
        o.data.transform(Matrix.Translation(-piv)); o.location = piv; nodes[n] = o
    bpy.context.view_layer.update()
    for n, p in PARENT.items():
        c = nodes[n]; mw = c.matrix_world.copy(); c.parent = nodes[p]; c.matrix_world = mw
    return sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in nodes.values())

def mk(name, rgb, rough, metal=0.0, spec=0.5):
    m = bpy.data.materials.new(name); m.use_nodes = True; b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1); b.inputs["Roughness"].default_value = rough; b.inputs["Metallic"].default_value = metal
    b.inputs["Specular IOR Level"].default_value = spec; return m

if __name__ == "__main__":
    src = sys.argv[-1]
    pngs = {n: paint(n) for n in OUTFITS}
    arm = load(src); J = pose(arm)
    pieces = bake_and_split(arm); fit_limbs(pieces, J)
    if os.environ.get("DBG"):
        for o in pieces:
            zs = [v.co.z for v in o.data.vertices]; ys = [v.co.y for v in o.data.vertices]
            print("PIECE", o["node"], o["src"], len(o.data.polygons), "z", round(min(zs), 2), round(max(zs), 2), "y", round(min(ys), 2), round(max(ys), 2))
    O = outfit_mat(pngs[os.environ.get("OUTFIT", "red")]); nt = O.node_tree
    uvn = nt.nodes.new("ShaderNodeUVMap"); uvn.uv_map = "UVMap"; nt.links.new(uvn.outputs["UV"], next(x for x in nt.nodes if x.type == "TEX_IMAGE").inputs["Vector"]); dark = mk("rubber", (0.03, 0.03, 0.03), 0.6); lens = mk("lens", (0.25, 0.12, 0.03), 0.05, 0.6, 1.0)
    dress(pieces, O, dark); helmet(O, lens, dark)
    tris = assemble()
    bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "fp_rider.glb"), export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
    print("rider tris", tris)
