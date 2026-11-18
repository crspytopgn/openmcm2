"""Make a pack-ready glb: one material per mesh named 'paint' with a tiny placeholder base colour image
(the livery/outfit replaces it), no metal/rough maps. usage: -- in.glb out.glb"""
import bpy, sys
a = sys.argv[sys.argv.index('--') + 1:]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=a[0])
img = bpy.data.images.new('paint', 4, 4); img.pixels = [0.6] * 64
m = bpy.data.materials.new('paint'); m.use_nodes = True; nt = m.node_tree; b = nt.nodes['Principled BSDF']
t = nt.nodes.new('ShaderNodeTexImage'); t.image = img; nt.links.new(t.outputs[0], b.inputs['Base Color'])
b.inputs['Roughness'].default_value = 0.6
for o in bpy.data.objects:
    if o.type == 'MESH':
        o.data.materials.clear(); o.data.materials.append(m)
bpy.ops.export_scene.gltf(filepath=a[1], export_format='GLB', export_image_format='AUTO', export_yup=True)
import os; print('PACKED', a[1], os.path.getsize(a[1]) // 1024, 'KB')
