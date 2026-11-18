"""Bake per-vertex rest position (game rest, metres) and node index into UV space."""
import bpy, numpy as np
d = np.load('/home/user/work/trellis/paint/vattr.npz')
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='/home/user/work/trellis/in/rider_t2.glb')
o = [o for o in bpy.data.objects if o.type == 'MESH'][0]
me = o.data; assert len(me.vertices) == len(d['rest']), (len(me.vertices), len(d['rest']))
for x in bpy.data.objects: x.select_set(False)
bpy.context.view_layer.objects.active = o; o.select_set(True)
a = me.color_attributes.new('restp', 'FLOAT_COLOR', 'POINT')
c = np.c_[d['rest'], np.ones(len(d['rest']))].astype(np.float32); a.data.foreach_set('color', c.ravel())
b = me.color_attributes.new('nodei', 'FLOAT_COLOR', 'CORNER')
lab = d['lab']
lv = np.zeros(len(me.loops), np.int32); me.loops.foreach_get('vertex_index', lv)
ls = np.zeros(len(me.polygons), np.int32); me.polygons.foreach_get('loop_start', ls)
tri = lab[lv].reshape(-1, 3)                                   # glTF import = triangles
own = np.where(tri[:, 1] == tri[:, 2], tri[:, 1], tri[:, 0])
fl = np.repeat(own, 3).astype(np.float32)
c = np.c_[fl, fl, fl, np.ones(len(fl))].astype(np.float32); b.data.foreach_set('color', c.ravel())
Wt = d['W']; nm = list(d['names'])
def grp(*ks): return Wt[:, [i for i, n in enumerate(nm) if n.startswith(ks)]].sum(1)
g = me.color_attributes.new('grpw', 'FLOAT_COLOR', 'POINT')
c = np.c_[grp('UprArm', 'LwrArm', 'Hand'), grp('UprLeg', 'LwrLeg', 'Foot'), grp('Head'), np.ones(len(Wt))].astype(np.float32); g.data.foreach_set('color', c.ravel())
sc = bpy.context.scene; sc.render.engine = 'CYCLES'; sc.cycles.samples = 1
nt = me.materials[0].node_tree
for what in ('nodei', 'grpw'):
    img = bpy.data.images.new(what, 2048, 2048, float_buffer=True, alpha=False)
    for n in list(nt.nodes):
        if n.name.startswith('B_'): nt.nodes.remove(n)
    at = nt.nodes.new('ShaderNodeAttribute'); at.name = 'B_a'; at.attribute_name = what
    e = nt.nodes.new('ShaderNodeEmission'); e.name = 'B_e'
    t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img
    nt.links.new(at.outputs['Color'], e.inputs['Color']); nt.links.new(e.outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    nt.nodes.active = t; sc.render.bake.margin = 8
    bpy.ops.object.bake(type='EMIT')
    px = np.array(img.pixels[:], dtype=np.float32).reshape(2048, 2048, 4)[::-1, :, :3]
    np.save(f'/home/user/work/trellis/paint/{what}.npy', px); print('baked', what)
import sys
if 'noao' in sys.argv: raise SystemExit
# AO for cavity shading
img = bpy.data.images.new('ao', 2048, 2048, float_buffer=True, alpha=False)
for n in list(nt.nodes):
    if n.name.startswith('B_'): nt.nodes.remove(n)
t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img; nt.nodes.active = t
sc.cycles.samples = 64; sc.world = bpy.data.worlds.new('w')
bpy.ops.object.bake(type='AO')
px = np.array(img.pixels[:], dtype=np.float32).reshape(2048, 2048, 4)[::-1, :, 0]
np.save('/home/user/work/trellis/paint/ao.npy', px); print('baked ao')
