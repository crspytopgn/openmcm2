"""Transfer textures from an HQ framed glb to a decimated one with a fresh shared UV layout.
usage: -- hq.glb low.glb out.glb RES name=img.png [name=img.png ...]  (first image is put on the exported material as base colour)"""
import bpy, sys, os
a = sys.argv[sys.argv.index('--') + 1:]
hq, low, out, R = a[0], a[1], a[2], int(a[3]); maps = [x.split('=', 1) for x in a[4:]]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=hq)
H = {}
for o in list(bpy.data.objects):
    if o.type == 'MESH': o.name = o.name + '_hq'; H[o.name[:-3]] = o
hqmat = bpy.data.materials.new('hqm'); hqmat.use_nodes = True
for o in H.values(): o.data.materials.clear(); o.data.materials.append(hqmat)
import bmesh
Lo = {}
if low.startswith('ratio:'):
    r = float(low[6:])
    for nd, h in H.items():
        me = h.data.copy()
        for uv in list(me.uv_layers): me.uv_layers.remove(uv)
        bm = bmesh.new(); bm.from_mesh(me); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new(nd, me); bpy.context.scene.collection.objects.link(o)
        o.matrix_world = h.matrix_world.copy()
        if len(me.polygons) > 60:
            m = o.modifiers.new('d', 'DECIMATE'); m.ratio = r; m.use_collapse_triangulate = True
            bpy.context.view_layer.objects.active = o; bpy.ops.object.modifier_apply(modifier='d')
        Lo[nd] = o
    # rebuild hierarchy like the HQ one
    for nd, h in H.items():
        if h.parent is not None and h.parent.name[:-3] in Lo:
            mw = Lo[nd].matrix_world.copy(); Lo[nd].parent = Lo[h.parent.name[:-3]]; Lo[nd].matrix_world = mw
    # empties (e.g. NullFrame) are kept from the HQ import
else:
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=low)
    Lo = {o.name: o for o in bpy.data.objects if o not in before and o.type == 'MESH' and o.name in H}
# shared UV layout across all low parts
for o in bpy.data.objects: o.select_set(False)
for o in Lo.values():
    for uv in list(o.data.uv_layers): o.data.uv_layers.remove(uv)
    bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4); bmesh.ops.recalc_face_normals(bm, faces=bm.faces); bm.to_mesh(o.data); bm.free()
for o in Lo.values():
    o.select_set(True); o.data.uv_layers.new(name='bake')
    o.data.uv_layers.active = o.data.uv_layers['bake']; o.data.uv_layers['bake'].active_render = True
saved = {}
for o in Lo.values():                     # unwrap on a smoothed copy of the surface (TRELLIS normals are noisy), then restore
    saved[o.name] = [v.co.copy() for v in o.data.vertices]
    bm = bmesh.new(); bm.from_mesh(o.data)
    for _ in range(25): bmesh.ops.smooth_vert(bm, verts=bm.verts, factor=0.5, use_axis_x=True, use_axis_y=True, use_axis_z=True)
    bm.to_mesh(o.data); bm.free()
bpy.context.view_layer.objects.active = next(iter(Lo.values()))
bpy.ops.object.mode_set(mode='EDIT'); bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.003, area_weight=0.0)
bpy.ops.object.mode_set(mode='OBJECT')
for o in Lo.values():
    for v, c in zip(o.data.vertices, saved[o.name]): v.co = c
    o.data.update()
bpy.context.view_layer.objects.active = next(iter(Lo.values()))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
sc = bpy.context.scene; sc.render.engine = 'CYCLES'; sc.cycles.samples = 4
sc.render.bake.use_selected_to_active = True; sc.render.bake.cage_extrusion = 0.035; sc.render.bake.max_ray_distance = 0.12; sc.render.bake.margin = 6
lowmat = bpy.data.materials.new('lowm'); lowmat.use_nodes = True
for o in Lo.values(): o.data.materials.clear(); o.data.materials.append(lowmat)
outimgs = {}
for name, path in maps:
    src = bpy.data.images.load(path); src.colorspace_settings.name = 'Non-Color'
    nt = hqmat.node_tree
    for n in list(nt.nodes):
        if n.name.startswith('B_'): nt.nodes.remove(n)
    t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = src
    e = nt.nodes.new('ShaderNodeEmission'); e.name = 'B_e'
    nt.links.new(t.outputs[0], e.inputs[0]); nt.links.new(e.outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    img = bpy.data.images.new(name, R, R, alpha=False); img.colorspace_settings.name = 'Non-Color'
    lt = lowmat.node_tree
    for n in list(lt.nodes):
        if n.name.startswith('B_'): lt.nodes.remove(n)
    tt = lt.nodes.new('ShaderNodeTexImage'); tt.name = 'B_t'; tt.image = img; lt.nodes.active = tt
    for nd, lo in Lo.items():
        for o in bpy.data.objects: o.select_set(False)
        H[nd].select_set(True); lo.select_set(True); bpy.context.view_layer.objects.active = lo
        bpy.ops.object.bake(type='EMIT', use_clear=False)
    img.filepath_raw = os.path.join(os.path.dirname(out), f'{os.path.basename(out)[:-4]}_{name}.png'); img.file_format = 'PNG'; img.save()
    outimgs[name] = img; print('baked', name)
# export low with base colour = first map
for o in H.values(): bpy.data.objects.remove(o)
lt = lowmat.node_tree
for n in list(lt.nodes):
    if n.name.startswith('B_'): lt.nodes.remove(n)
first = outimgs[maps[0][0]]; first.colorspace_settings.name = 'sRGB'
tt = lt.nodes.new('ShaderNodeTexImage'); tt.image = first; lt.links.new(tt.outputs[0], lt.nodes['Principled BSDF'].inputs['Base Color'])
for o in Lo.values():
    for uv in [u for u in o.data.uv_layers if u.name != 'bake']: o.data.uv_layers.remove(uv)
bpy.ops.export_scene.gltf(filepath=out, export_format='GLB', export_yup=True, export_materials='EXPORT')
print('OUT', out, 'tris', sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in Lo.values()))
