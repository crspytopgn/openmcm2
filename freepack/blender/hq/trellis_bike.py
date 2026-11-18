"""TRELLIS bike (from Michael's sheet) scaled onto the game bike pivots and split into the 8-node frame tree.
build(full) -> one object per node, tag 'keep'. LAB env var: write per-vertex labels to npz."""
import math, os, numpy as np, bpy, bmesh
from mathutils import Vector as V, Matrix
from geo import link
from bike_geo import HP, FA, RA, SP, KS
SRC = '/home/user/work/trellis/in/bike_t2.glb'
RAW_RA = np.array([0.0, 0.3442, -0.1473]); RAW_FA = np.array([0.0, -0.3385, -0.1427])
NODES = ['Bike', 'HandleBars', 'Shocks', 'TireFront', 'KickStart', 'SwingArm', 'TireRear']

def load():
    bpy.ops.import_scene.gltf(filepath=SRC)
    o = [o for o in bpy.data.objects if o.type == 'MESH'][0]
    me = o.data; me.transform(o.matrix_world); o.matrix_world = Matrix()
    for x in [x for x in bpy.data.objects if x != o]: bpy.data.objects.remove(x)
    # raw -> metres: front is -Y raw, so rotate 180 about Z; scale by wheelbase; rear axle onto RA
    s = (FA.y - RA.y) / np.linalg.norm(RAW_FA - RAW_RA)
    ra = np.array([-RAW_RA[0], -RAW_RA[1], RAW_RA[2]]) * s
    M = Matrix.Translation(V(tuple(np.array(RA) - ra))) @ Matrix.Scale(s, 4) @ Matrix.Rotation(math.pi, 4, 'Z')
    me.transform(M)
    return o

def labels(P):
    x, y, z = P[:, 0], P[:, 1], P[:, 2]; ax = np.abs(x)
    lab = np.zeros(len(P), int)
    def wheel(c, r_out):
        d = np.hypot(y - c.y, z - c.z)
        return (d < r_out) & ((ax < 0.052) | ((d > 0.25) & (ax < 0.075)) | ((d < 0.06) & (ax < 0.08))), d
    wf, df = wheel(FA, 0.355); wr, dr = wheel(RA, 0.355)
    # fork axis FA -> HP
    a = np.array(FA); u = np.array(HP - FA); L = np.linalg.norm(u); u /= L
    rel = P - a; t = rel @ u; rad = np.linalg.norm(rel - t[:, None] * u, axis=1)
    radyz = np.linalg.norm((rel - t[:, None] * u)[:, 1:], axis=1)
    lower = (t > -0.08) & (t < 0.42) & (radyz < 0.13) & (ax > 0.03) & (ax < 0.175)
    upper = (t >= 0.42) & (t < L + 0.12) & (radyz < 0.075) & (ax < 0.115)
    bars = (((z > HP.z + 0.03) & (y > HP.y - 0.12)) | ((z > HP.z - 0.06) & (ax < 0.06) & (y > HP.y - 0.07))) & (np.abs(y - HP.y) < 0.20) & (z < HP.z + 0.25)
    grips = (z > HP.z - 0.02) & (ax > 0.14) & (np.abs(y - HP.y) < 0.32) & (z < HP.z + 0.3)
    plate = (t > 0.25) & (t < L + 0.1) & (rel @ np.array([0, u[2], -u[1]]) > 0) & (radyz < 0.20) & (ax < 0.16)
    fender = (df > 0.40) & (df < 0.78) & (z > FA.z + 0.30) & (z < FA.z + 0.70) & (ax < 0.105) & (y > FA.y - 0.30)
    lab[upper | bars | grips | plate | fender] = 1
    lab[lower] = 2
    lab[wf] = 3
    # swingarm: around the SP -> RA beam, plus chain guide; rear wheel over it
    b = np.array(SP); v = np.array(RA - SP); Lb = np.linalg.norm(v); v /= Lb
    rb = P - b; tb = rb @ v; radb = np.linalg.norm((rb - tb[:, None] * v)[:, 1:], axis=1)
    sw = (tb > -0.04) & (tb < Lb + 0.06) & (radb < 0.075) & (ax < 0.15)
    # chain runs, chain guide and the arm's top edge ride with the swingarm (drawn suspension swings it)
    nb_ = np.array([0, -v[2], v[1]]); nb_ = nb_ if nb_[2] > 0 else -nb_
    hb = rb @ nb_
    sw |= (tb > 0.06) & (tb < Lb + 0.06) & (np.abs(hb) < 0.125) & (ax < 0.15)
    lab[sw & ~wr] = 5
    lab[wr] = 6
    ks = (np.linalg.norm(P - np.array(KS), axis=1) < 0.07) & (x > 0.09)
    lab[ks & (lab == 0)] = 4
    return lab

def build(full=True):
    o = load(); me = o.data
    P = np.array([v.co[:] for v in me.vertices])
    lab = labels(P)
    if os.environ.get('LAB'): np.savez(os.environ['LAB'], lab=lab, P=P)
    bm = bmesh.new(); bm.from_mesh(me); uv = bm.loops.layers.uv.active
    groups = {}
    for f in bm.faces:
        ls = [lab[v.index] for v in f.verts]; groups.setdefault(NODES[max(set(ls), key=ls.count)], []).append(f)
    out = []; mat = me.materials[0]
    for n, fs in groups.items():
        nb = bmesh.new(); uvl = nb.loops.layers.uv.new('UVMap'); vmap = {}
        for f in fs:
            vs = []
            for v in f.verts:
                if v.index not in vmap: vmap[v.index] = nb.verts.new(v.co)
                vs.append(vmap[v.index])
            try: nf = nb.faces.new(vs)
            except ValueError: continue
            for l, ol in zip(nf.loops, f.loops): l[uvl].uv = ol[uv].uv
        m2 = bpy.data.meshes.new(n); nb.to_mesh(m2); nb.free(); m2.materials.append(mat)
        out.append(link(n, m2, 'keep', n, smooth=True))
    bm.free(); bpy.data.objects.remove(o)
    return out
