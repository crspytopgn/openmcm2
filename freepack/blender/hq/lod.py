"""Decimate a framed glb per node. usage: -- in.glb out.glb ratio [Node,Node,...]  (nodes listed are kept whole)"""
import bpy, sys
a = sys.argv[sys.argv.index('--') + 1:]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=a[0])
r = float(a[2]); tris = 0; KEEP = a[3].split(',') if len(a) > 3 else []
for o in bpy.data.objects:
    if o.type != 'MESH' or len(o.data.polygons) < 50 or o.name.split('.')[0] in KEEP: continue
    m = o.modifiers.new('d', 'DECIMATE'); m.ratio = r; m.use_collapse_triangulate = True
    bpy.context.view_layer.objects.active = o; bpy.ops.object.modifier_apply(modifier='d')
for o in bpy.data.objects:
    if o.type == 'MESH': tris += sum(len(p.vertices) - 2 for p in o.data.polygons)
bpy.ops.export_scene.gltf(filepath=a[1], export_format='GLB', export_yup=True, export_materials='EXPORT')
print('LOD', a[1], 'tris', tris)
