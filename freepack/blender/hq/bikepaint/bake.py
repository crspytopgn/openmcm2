"""Bake bike: metres position, face node id, normal, AO, and the TRELLIS base colour into 2048 maps."""
import bpy, numpy as np
d = np.load('/home/user/work/trellis/paint/bike_lab.npz'); P = d['P']; lab = d['lab']
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='/home/user/work/trellis/in/bike_t2.glb')
o = [o for o in bpy.data.objects if o.type == 'MESH'][0]
me = o.data; assert len(me.vertices) == len(P)
for x in bpy.data.objects: x.select_set(False)
bpy.context.view_layer.objects.active = o; o.select_set(True)
a = me.color_attributes.new('pm', 'FLOAT_COLOR', 'POINT'); a.data.foreach_set('color', np.c_[P, np.ones(len(P))].astype(np.float32).ravel())
lv = np.zeros(len(me.loops), np.int32); me.loops.foreach_get('vertex_index', lv)
tri = lab[lv].reshape(-1, 3); own = np.where(tri[:, 1] == tri[:, 2], tri[:, 1], tri[:, 0]).astype(np.float32)
fl = np.repeat(own, 3)
b = me.color_attributes.new('nd', 'FLOAT_COLOR', 'CORNER'); b.data.foreach_set('color', np.c_[fl, fl, fl, np.ones(len(fl))].astype(np.float32).ravel())
sc = bpy.context.scene; sc.render.engine = 'CYCLES'; sc.cycles.samples = 1
nt = me.materials[0].node_tree
def bake(name, src):
    img = bpy.data.images.new(name, 2048, 2048, float_buffer=True, alpha=False)
    for n in list(nt.nodes):
        if n.name.startswith('B_'): nt.nodes.remove(n)
    e = nt.nodes.new('ShaderNodeEmission'); e.name = 'B_e'
    t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img
    if src == 'geo':
        g = nt.nodes.new('ShaderNodeNewGeometry'); g.name = 'B_g'; nt.links.new(g.outputs['Normal'], e.inputs['Color'])
    else:
        at = nt.nodes.new('ShaderNodeAttribute'); at.name = 'B_a'; at.attribute_name = src; nt.links.new(at.outputs['Color'], e.inputs['Color'])
    nt.links.new(e.outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    nt.nodes.active = t; sc.render.bake.margin = 8
    bpy.ops.object.bake(type='EMIT')
    px = np.array(img.pixels[:], dtype=np.float32).reshape(2048, 2048, 4)[::-1, :, :3]
    np.save(f'{name}.npy', px); print('baked', name)
bake('pos', 'pm'); bake('node', 'nd'); bake('nrm', 'geo')
img = bpy.data.images.new('ao', 2048, 2048, float_buffer=True, alpha=False)
for n in list(nt.nodes):
    if n.name.startswith('B_'): nt.nodes.remove(n)
t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img; nt.nodes.active = t
sc.cycles.samples = 64; sc.world = bpy.data.worlds.new('w')
bpy.ops.object.bake(type='AO')
np.save('ao.npy', np.array(img.pixels[:], dtype=np.float32).reshape(2048, 2048, 4)[::-1, :, 0]); print('baked ao')
for im in bpy.data.images:
    if im.size[0] == 4096:
        im.filepath_raw = f'/home/user/work/trellis/bpaint/tex_{im.name}.png'; im.file_format = 'PNG'; im.save()
