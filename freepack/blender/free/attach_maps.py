"""Put metal/roughness and normal maps on a free bike's painted material (the livery replaces only its colour).
usage: python attach_maps.py -- in.glb out.glb mr.png nrm.png [normal_strength]"""
import bpy, sys, os
a = sys.argv[sys.argv.index('--') + 1:]
src, dst, mrp, nrp = a[:4]; st = float(a[4]) if len(a) > 4 else 1.0
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
done = set()
for m in bpy.data.materials:
    if not m.node_tree or m.name in done: continue
    nt = m.node_tree; b = next((n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED'), None)
    if not b or not b.inputs['Base Color'].is_linked: continue
    mr = nt.nodes.new('ShaderNodeTexImage'); mr.image = bpy.data.images.load(mrp); mr.image.colorspace_settings.name = 'Non-Color'
    sep = nt.nodes.new('ShaderNodeSeparateColor'); nt.links.new(mr.outputs['Color'], sep.inputs['Color'])
    nt.links.new(sep.outputs['Green'], b.inputs['Roughness']); nt.links.new(sep.outputs['Blue'], b.inputs['Metallic'])
    nr = nt.nodes.new('ShaderNodeTexImage'); nr.image = bpy.data.images.load(nrp); nr.image.colorspace_settings.name = 'Non-Color'
    nm = nt.nodes.new('ShaderNodeNormalMap'); nm.inputs['Strength'].default_value = st
    nt.links.new(nr.outputs['Color'], nm.inputs['Color']); nt.links.new(nm.outputs['Normal'], b.inputs['Normal'])
    done.add(m.name)
print('maps on', done)
bpy.ops.export_scene.gltf(filepath=dst, export_format='GLB', export_image_format='AUTO', export_yup=True)
print('EXPORTED', dst, os.path.getsize(dst) // 1024, 'KB')
