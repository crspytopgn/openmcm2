"""Bake surface position + normal of the TRELLIS rider into 2048 float EXRs (UV space)."""
import bpy, sys
R = int(sys.argv[sys.argv.index('--') + 1]) if '--' in sys.argv else 2048
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='/home/user/work/trellis/in/rider_t2.glb')
o = [o for o in bpy.data.objects if o.type == 'MESH'][0]
[x.select_set(False) for x in bpy.data.objects]; bpy.context.view_layer.objects.active = o; o.select_set(True)
sc = bpy.context.scene; sc.render.engine = 'CYCLES'; sc.cycles.samples = 1; sc.cycles.device = 'CPU'
m = o.data.materials[0]; nt = m.node_tree
for what in ('Position', 'Normal'):
    img = bpy.data.images.new(what, R, R, float_buffer=True, alpha=False)
    for n in list(nt.nodes):
        if n.name.startswith('B_'): nt.nodes.remove(n)
    g = nt.nodes.new('ShaderNodeNewGeometry'); g.name = 'B_g'
    e = nt.nodes.new('ShaderNodeEmission'); e.name = 'B_e'
    t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img
    nt.links.new(g.outputs[what], e.inputs['Color'])
    out = nt.nodes['Material Output']; nt.links.new(e.outputs[0], out.inputs['Surface'])
    nt.nodes.active = t
    sc.render.bake.margin = 8
    bpy.ops.object.bake(type='EMIT')
    img.filepath_raw = f'/home/user/work/trellis/paint/{what}.exr'; img.file_format = 'OPEN_EXR'; img.save()
    print('baked', what)
