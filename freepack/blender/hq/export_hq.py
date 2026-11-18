"""Export an HQ build to a framed glb (game units, node names = original frame trees).
usage: python export_hq.py -- bike|rider MODULE OUT.glb"""
import sys, os, bpy
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mathutils import Vector as V, Matrix
kind, modname, out = sys.argv[sys.argv.index('--') + 1:][:3]
bpy.ops.wm.read_factory_settings(use_empty=True)
import importlib, mats
mod = importlib.import_module(modname)
K = 3.355
objs = mod.build(True) if kind == 'bike' else mod.build()
mats.apply(objs)
if kind == 'bike':
    from bike_geo import HP, FA, RA, SP, KS
    PIV = {'Bike': V((0, 0, 0)), 'HandleBars': HP, 'Shocks': HP, 'TireFront': FA, 'KickStart': KS, 'NullFrame': V((0, -0.333, 2.5)) / K, 'SwingArm': SP, 'TireRear': RA}
    PAR = {'HandleBars': 'Bike', 'Shocks': 'HandleBars', 'TireFront': 'Shocks', 'KickStart': 'Bike', 'NullFrame': 'Bike', 'SwingArm': 'Bike', 'TireRear': 'SwingArm'}
    ROOT = 'Bike'
else:
    from rider_geo import PIV as RP, PARENT
    PIV = dict(RP); PAR = dict(PARENT); ROOT = 'Pelvis'
dg = bpy.context.evaluated_depsgraph_get()
groups = {}
for o in objs:
    me = bpy.data.meshes.new_from_object(o.evaluated_get(dg))
    me.transform(o.matrix_world); me.transform(Matrix.Scale(K, 4))
    groups.setdefault(o['node'], []).append(me)
for o in list(bpy.data.objects): bpy.data.objects.remove(o)
made = {}
import bmesh
for nd in PIV:
    mes = groups.get(nd, [])
    bm = bmesh.new()
    mats_all = []
    for me in mes:
        remap = []
        for m in me.materials:
            if m not in mats_all: mats_all.append(m)
            remap.append(mats_all.index(m))
        tmp = bmesh.new(); tmp.from_mesh(me)
        for f in tmp.faces:
            f.material_index = remap[f.material_index] if remap else 0
        tmp.to_mesh(me); tmp.free()
        bm.from_mesh(me)
    piv = PIV[nd] * K
    bmesh.ops.translate(bm, verts=bm.verts, vec=-piv)
    mesh = bpy.data.meshes.new(nd); bm.to_mesh(mesh); bm.free()
    for m in mats_all: mesh.materials.append(m)
    ob = bpy.data.objects.new(nd, mesh); bpy.context.scene.collection.objects.link(ob)
    ob.location = piv; made[nd] = ob
bpy.context.view_layer.update()
for nd, ob in made.items():
    if nd in PAR:
        p = made[PAR[nd]]; mw = ob.matrix_world.copy(); ob.parent = p; ob.matrix_world = mw
bpy.context.view_layer.update()
bpy.ops.export_scene.gltf(filepath=out, export_format='GLB', export_yup=True, export_apply=False, export_materials='EXPORT')
tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in made.values())
print('EXPORTED', out, 'tris', tris)
