"""Swap the HD (TRELLIS) bike's lumpy fork, rear shock and wheels for clean modelled ones (bike_geo).
usage: python hd_fix.py -- in.glb out.glb tags.npz
Writes tags.npz: centroids (metres, x right, y forward, z up) of every new face plus its part tag, so the
rebake can paint them by part."""
import sys, os, math, bpy, bmesh, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mathutils import Vector as V, Matrix
src, dst, tagf = sys.argv[sys.argv.index('--') + 1:][:3]
K = 3.355
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src); bpy.context.view_layer.update()
import geo, bike_geo as bg
from bike_geo import FA, RA, HP, U, FX
node = {o.name.split('.')[0]: o for o in bpy.data.objects if o.type == 'MESH'}
SH_T, SH_B = V((0, -0.19, 0.84)), V((0, -0.255, 0.37))

def seg(P, a, b):
    a = np.array(a); d = np.array(b) - a; L = np.linalg.norm(d); d /= L
    rel = P - a; t = rel @ d; r = np.linalg.norm(rel - t[:, None] * d, axis=1); return t / L, r

def cut(o, rule):
    me = o.data; W = o.matrix_world
    bm = bmesh.new(); bm.from_mesh(me); bm.faces.ensure_lookup_table()
    C = np.array([tuple(W @ f.calc_center_median()) for f in bm.faces]) / K
    kill = rule(C); bmesh.ops.delete(bm, geom=[f for f, k in zip(bm.faces, kill) if k], context='FACES')
    bm.to_mesh(me); bm.free(); me.update(); print('cut', o.name, int(kill.sum()), 'of', len(kill))

for n in ('TireFront', 'TireRear', 'Shocks'): cut(node[n], lambda C: np.ones(len(C), bool))
def bars_rule(C):
    k = np.zeros(len(C), bool)
    for sx in (-1, 1):
        a = FA + V((sx * FX, 0, 0)); t, r = seg(C, a, HP + V((sx * FX, 0, 0)))
        k |= (t > -0.1) & (t < 0.80) & (r < 0.06)
    return k
cut(node['HandleBars'], bars_rule)
def d_ra(C): return np.hypot(C[:, 1] - RA.y, C[:, 2] - RA.z)
def fork_bike(C):
    k = np.zeros(len(C), bool)
    for sx in (-1, 1):
        t, r = seg(C, FA + V((sx * FX, 0, 0)), HP + V((sx * FX, 0, 0))); k |= (t > 0.25) & (t < 0.85) & (r < 0.065)
    return k
cut(node['Bike'], lambda C: fork_bike(C) | ((d_ra(C) < 0.38) & (np.abs(C[:, 0]) < 0.12)))
# swingarm: old tyre scraps over the wheel and the lumpy axle ends / sprocket (clean axle blocks go back on)
cut(node['SwingArm'], lambda C: ((d_ra(C) > 0.2) & (d_ra(C) < 0.38) & (C[:, 2] > 0.45) & (np.abs(C[:, 0]) < 0.075)) |
    ((d_ra(C) < 0.125) & (np.abs(C[:, 0]) > 0.03) & (np.abs(C[:, 0]) < 0.12)))
def shock_rule(C):
    t, r = seg(C, SH_T, SH_B)
    return (t > -0.04) & (t < 1.04) & (r < 0.05) & (np.abs(C[:, 0]) < 0.075) | \
           (t > 0.0) & (t < 0.4) & (np.abs(C[:, 0] - 0.045) < 0.03) & (r < 0.075)   # reservoir side
cut(node['Bike'], shock_rule)

# clean parts, built in metres
geo.OBJ.clear()
parts = bg.wheel(True) + bg.wheel(False) + bg.forks() + bg.rear_suspension()
arm = (RA - bg.SP).normalized()
for sx in (-1, 1):
    blk = geo.box(RA + V((sx * 0.088, 0, 0)) - arm * 0.035, (0.026, 0.11, 0.05), 0.4, 1, 'axleblock', 'alu_cast', 'SwingArm',
                  rot=Matrix.Rotation(math.atan2(arm.z, arm.y), 3, 'X'))
    if sx > 0:   # rear caliper below the arm, ahead of the axle, on the disc side
        th = math.radians(-35); cp = RA + V((0.07, math.cos(th) * 0.1, math.sin(th) * 0.1))
        parts.append(geo.box(cp, (0.032, 0.07, 0.045), 0.6, 1, 'rcaliper', 'caliper', 'SwingArm', rot=Matrix.Rotation(th + math.pi / 2, 3, 'X')))
    parts += [blk, geo.cyl(RA + V((sx * 0.1, 0, 0)), RA + V((sx * 0.112, 0, 0)), 0.016, 6, name='axlenut', tag='alu', node='SwingArm')]
SCALE = {'TireFront': (FA, 0.349 / 0.345), 'TireRear': (RA, 0.339 / 0.334)}
dg = bpy.context.evaluated_depsgraph_get()
livm = node['Bike'].data.materials[0]
tags = sorted({p['tag'] for p in parts}); cent = []; tid = []
by = {}
for p in parts:
    me = bpy.data.meshes.new_from_object(p.evaluated_get(dg)); me.transform(p.matrix_world)
    if p['node'] in SCALE:
        c, s = SCALE[p['node']]; me.transform(Matrix.Translation(c) @ Matrix.Scale(s, 4) @ Matrix.Translation(-c))
    for v in me.vertices: cent.append(tuple(v.co)); tid.append(tags.index(p['tag']))   # glb export triangulates, so match by vertices
    me.transform(Matrix.Scale(K, 4)); me.materials.clear(); me.materials.append(livm)
    by.setdefault(p['node'], []).append(me)
for p in parts: bpy.data.objects.remove(p)
for nd, mes in by.items():
    tgt = node[nd]
    bm = bmesh.new()
    for me in mes: bm.from_mesh(me)
    m2 = bpy.data.meshes.new(nd + '_new'); bm.to_mesh(m2); bm.free()
    m2.uv_layers.new(name=tgt.data.uv_layers[0].name)
    ob = bpy.data.objects.new(nd + '_new', m2); bpy.context.scene.collection.objects.link(ob)
    bpy.ops.object.select_all(action='DESELECT'); ob.select_set(True); tgt.select_set(True)
    bpy.context.view_layer.objects.active = tgt; bpy.ops.object.join()
    me = tgt.data; li = [m.name if m else '' for m in me.materials].index(livm.name)
    for f in me.polygons: f.material_index = li
    print('joined', nd, len(me.polygons), [m.name if m else None for m in me.materials])
np.savez(tagf, cent=np.array(cent, np.float32), tid=np.array(tid, np.int32), tags=np.array(tags))
bpy.ops.export_scene.gltf(filepath=dst, export_format='GLB', export_yup=True)
print('EXPORTED', dst, os.path.getsize(dst) // 1024, 'KB', 'tags', tags)
