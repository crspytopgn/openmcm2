"""TRELLIS rider (from Michael's kit sheet) fitted onto the original 16-node rider frame tree.
build() -> one object per node, tag 'keep' (keeps the generated material)."""
import math, numpy as np, bpy, bmesh
from mathutils import Vector as V, Matrix
from geo import link
from rider_geo import PIV, PARENT

SRC = '/home/user/work/trellis/in/rider_t2.glb'
EXTRA = 1.0
POSE_T = '/home/user/work/pose/seated_T.json'

# mesh joints (x, z) on the right side after prep (sole at 0, centred, facing +Y), before EXTRA
MJ = {'hip': (0.10, 0.90), 'knee': (0.125, 0.50), 'ankle': (0.15, 0.11), 'toe': (0.15, 0.03),
      'shoulder': (0.20, 1.42), 'elbow': (0.28, 1.13), 'wrist': (0.335, 0.92), 'fingers': (0.35, 0.76)}

def load():
    bpy.ops.import_scene.gltf(filepath=SRC)
    o = [o for o in bpy.data.objects if o.type == 'MESH'][0]
    me = o.data; me.transform(o.matrix_world); o.matrix_world = Matrix()
    me.transform(Matrix.Rotation(math.pi, 4, 'Z'))
    P = np.array([v.co[:] for v in me.vertices])
    me.transform(Matrix.Scale(1.79 / (P[:, 2].max() - P[:, 2].min()), 4))
    P = np.array([v.co[:] for v in me.vertices])
    me.transform(Matrix.Translation(V((-(P[:, 0].max() + P[:, 0].min()) / 2, -np.median(P[:, 1]), -P[:, 2].min()))))
    me.transform(Matrix.Scale(EXTRA, 4))
    return o

def seg_dist(P, a, b):
    ab = b - a; t = np.clip(((P - a) @ ab) / (ab @ ab), 0, 1)
    return np.linalg.norm(P - (a[None] + t[:, None] * ab[None]), axis=1)

def build():
    o = load(); me = o.data
    P = np.array([v.co[:] for v in me.vertices])
    def jy(x, z):                 # mean y of the mesh around a joint (xz)
        m = (np.abs(P[:, 0] - x) < 0.04) & (np.abs(P[:, 2] - z) < 0.04)
        return float(P[m, 1].mean()) if m.any() else 0.0
    J = {}
    for k, (x, z) in MJ.items():
        for s, sx in (('R', 1), ('L', -1)):
            X, Z = sx * x * EXTRA, z * EXTRA
            J[k + s] = np.array([X, jy(X, Z), Z])
    for s in 'RL':
        J['toe' + s][1] += 0.12
    yt = jy(0, 1.2 * EXTRA)
    # segments: node -> (a, b, radius)
    SEG = {'Head': (np.array([0, yt, 1.53 * EXTRA]), np.array([0, yt, 1.78 * EXTRA]), 0.13),
           'UprTorso': (np.array([0, yt, 1.17 * EXTRA]), np.array([0, yt, 1.50 * EXTRA]), 0.17),
           'LwrTorso': (np.array([0, yt, 0.99 * EXTRA]), np.array([0, yt, 1.17 * EXTRA]), 0.16),
           'Pelvis': (np.array([0, yt, 0.80 * EXTRA]), np.array([0, yt, 0.99 * EXTRA]), 0.16)}
    for s in 'RL':
        SEG['UprArm' + s] = (J['shoulder' + s], J['elbow' + s], 0.065)
        SEG['LwrArm' + s] = (J['elbow' + s], J['wrist' + s], 0.055)
        SEG['Hand' + s] = (J['wrist' + s], J['fingers' + s], 0.05)
        SEG['UprLeg' + s] = (J['hip' + s], J['knee' + s], 0.09)
        SEG['LwrLeg' + s] = (J['knee' + s], J['ankle' + s], 0.075)
        SEG['Foot' + s] = (J['ankle' + s], J['toe' + s], 0.07)
    names = list(SEG)
    D = np.stack([seg_dist(P, a, b) / r for a, b, r in SEG.values()], 1)
    # torso nodes split by height, not distance (avoids zig-zags at the waist)
    lab = D.argmin(1)
    tors = np.isin(lab, [names.index(n) for n in ('Head', 'UprTorso', 'LwrTorso', 'Pelvis')])
    z = P[:, 2]
    for n, lo, hi in (('Pelvis', -1, 0.99), ('LwrTorso', 0.99, 1.17), ('UprTorso', 1.17, 1.47), ('Head', 1.47, 9)):
        lab[tors & (z >= lo * EXTRA) & (z < hi * EXTRA)] = names.index(n)
    # game joint targets for each segment's proximal/distal point
    G = {k: np.array(tuple(v)) for k, v in PIV.items()}
    def tgt(n):
        s = n[-1]
        if n.startswith('UprArm'): return G['UprArm' + s], G['LwrArm' + s]
        if n.startswith('LwrArm'): return G['LwrArm' + s], G['Hand' + s]
        if n.startswith('Hand'): return G['Hand' + s], G['Hand' + s] + (J['fingers' + s] - J['wrist' + s])
        if n.startswith('UprLeg'): return G['UprLeg' + s], G['LwrLeg' + s]
        if n.startswith('LwrLeg'): return G['LwrLeg' + s], G['Foot' + s]
        if n.startswith('Foot'): return G['Foot' + s], G['Foot' + s] + (J['toe' + s] - J['ankle' + s])
        return None
    TZ = {'Pelvis': 0.047, 'LwrTorso': 0.044, 'UprTorso': 0.041, 'Head': -0.015}
    # rest-fit matrix per node (mesh -> game rest, metres)
    MN = {}
    for n in names:
        a, b, _ = SEG[n]; t = tgt(n)
        if t is None:
            MN[n] = np.array(Matrix.Translation(V((0, G['Pelvis'][1] - yt, TZ[n]))))
        else:
            ga, gb = t
            q = V(tuple(b - a)).rotation_difference(V(tuple(gb - ga)))
            d = (b - a) / np.linalg.norm(b - a); k = np.linalg.norm(gb - ga) / np.linalg.norm(b - a)
            S = np.eye(4); S[:3, :3] += (k - 1) * np.outer(d, d)
            MN[n] = np.array(Matrix.Translation(V(tuple(ga))) @ q.to_matrix().to_4x4() @ Matrix(S.tolist()) @ Matrix.Translation(-V(tuple(a))))
    # smooth skin weights: softmax of -D over the owner bone and its neighbours; torso bands blended by height
    nb = {n: {n} for n in names}
    for c, p in PARENT.items(): nb[c].add(p); nb[p].add(c)
    W = np.zeros_like(D)
    for i, n in enumerate(names):
        sel = lab == i
        if not sel.any(): continue
        js = [names.index(m) for m in nb[n]]
        e = np.exp(-(D[np.ix_(sel, js)] - D[sel, i][:, None]) / 0.18)
        W[np.ix_(sel, js)] = e / e.sum(1, keepdims=True)
    TOR = ['Pelvis', 'LwrTorso', 'UprTorso', 'Head']; ti = [names.index(m) for m in TOR]
    cuts = [0.99, 1.17, 1.47]
    tw = W[:, ti].sum(1)
    band = np.zeros((len(z), 4))
    prev = np.ones(len(z))
    for j, c in enumerate(cuts):
        h = 0.035 if j < 2 else 0.02
        up = np.clip((z - (c - h)) / (2 * h), 0, 1); up = up * up * (3 - 2 * up)
        band[:, j] = prev * (1 - up); prev = prev * up
    band[:, 3] = prev
    W[:, ti] = band * tw[:, None]
    # pose: T_n (rest -> seated) in metres; skin to seated, then each node keeps its faces in its own rest frame
    import json
    TJ = json.load(open(POSE_T)); T = {n: np.array(TJ[n]) for n in names}
    A = np.stack([T[n] @ MN[n] for n in names])          # mesh -> seated, per bone
    Ph = np.c_[P, np.ones(len(P))]
    Vs = np.einsum('vb,bij,vj->vi', W, A, Ph)[:, :3]
    Tinv = {n: np.linalg.inv(T[n]) for n in names}
    Vr = np.einsum('vb,bij,vj->vi', W, np.stack([MN[n] for n in names]), Ph)[:, :3]
    np.savez('/home/user/work/trellis/paint/vattr.npz', rest=Vr, seat=Vs, lab=lab, names=np.array(names), W=W)
    bm = bmesh.new(); bm.from_mesh(me); bm.verts.ensure_lookup_table()
    uv = bm.loops.layers.uv.active
    groups = {}
    for f in bm.faces:
        ls = [lab[v.index] for v in f.verts]; groups.setdefault(names[max(set(ls), key=ls.count)], []).append(f)
    out = []
    mat = me.materials[0]
    for n, fs in groups.items():
        Ti = Tinv[n]
        nbm = bmesh.new(); uvl = nbm.loops.layers.uv.new('UVMap'); vmap = {}
        for f in fs:
            vs = []
            for v in f.verts:
                if v.index not in vmap:
                    p = Ti @ np.r_[Vs[v.index], 1.0]
                    vmap[v.index] = nbm.verts.new(V(tuple(p[:3])))
                vs.append(vmap[v.index])
            try: nf = nbm.faces.new(vs)
            except ValueError: continue
            for l, ol in zip(nf.loops, f.loops): l[uvl].uv = ol[uv].uv
        m2 = bpy.data.meshes.new(n); nbm.to_mesh(m2); nbm.free(); m2.materials.append(mat)
        ob = link(n, m2, 'keep', n, smooth=True); out.append(ob)
    bm.free(); bpy.data.objects.remove(o)
    return out
