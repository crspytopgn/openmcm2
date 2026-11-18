"""Give a framed free-pack glb fresh UVs for its painted (textured) faces, carrying textures across, and bake
position (metres), node, normal and AO on the new layout.
usage: -- in.glb out.glb outdir RES name=old.png [name=old.png ...]"""
import bpy, bmesh, numpy as np, sys, os
a = sys.argv[sys.argv.index('--') + 1:]
src, dst, out, R = a[0], a[1], a[2], int(a[3]); maps = [x.split('=', 1) for x in a[4:]]
os.makedirs(out, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src); bpy.context.view_layer.update()
meshes = [o for o in bpy.data.objects if o.type == 'MESH']
if os.environ.get('WELD'):
    # faceted meshes come in with every face split; weld (keeping the custom normals) so unwrap islands are whole
    for o in meshes:
        bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-6); bm.to_mesh(o.data); bm.free()
names = sorted({o.name.split('.')[0] for o in meshes})
texmat = {m.name for m in bpy.data.materials if m.node_tree and any(n.type == 'TEX_IMAGE' for n in m.node_tree.nodes)}
livm = next(m for m in bpy.data.materials if m.name in texmat)
# plain number-plate faces join the painted layout (flagged node+100) so boards can be painted as clean shapes
for o in meshes:
    me = o.data
    pl = [i for i, m in enumerate(me.materials) if m and m.name.startswith('plate')]
    if not pl: continue
    if livm.name not in [m.name for m in me.materials if m]: me.materials.append(livm)
    li = [m.name if m else '' for m in me.materials].index(livm.name)
    flag = np.zeros(len(me.polygons), bool)
    for f in me.polygons:
        if f.material_index in pl: flag[f.index] = True; f.material_index = li
    o['plate'] = flag.tolist()
# TAGS=npz (from hd_fix.py): modelled parts carry a part tag (G channel of the node bake, 0 = none)
TG = np.load(os.environ['TAGS']) if os.environ.get('TAGS') else None
if TG is not None:
    from scipy.spatial import cKDTree
    kd = cKDTree(TG['cent']); PAINT_T = list(TG['tags']).index('paint') + 1
    open(os.path.join(out, 'tags.txt'), 'w').write('\n'.join(TG['tags']))
for o in meshes:
    me = o.data; W = o.matrix_world
    tg = np.zeros(len(me.polygons), np.int32)
    if TG is not None:
        Pv = np.array([tuple(W @ v.co) for v in me.vertices]) / 3.355
        d, i = kd.query(Pv); vt = np.where(d < 1e-4, TG['tid'][i] + 1, 0)
        for f in me.polygons:   # a face of a modelled part has every corner on it
            c_ = vt[list(f.vertices)]
            if (c_ > 0).all(): tg[f.index] = np.bincount(c_).argmax()
    o['tg'] = tg.tolist()
for o in meshes:
    me = o.data; k = names.index(o.name.split('.')[0]); W = o.matrix_world
    P = np.array([tuple(W @ v.co) for v in me.vertices], np.float32) / 3.355
    at = me.color_attributes.new('pm', 'FLOAT_COLOR', 'POINT'); at.data.foreach_set('color', np.c_[P, np.ones(len(P))].astype(np.float32).ravel())
    nl = len(me.loops); kk = np.full(nl, float(k), np.float32)
    if 'plate' in o.keys():
        fl = np.array(o['plate'], bool)
        for f in me.polygons:
            if fl[f.index]: kk[f.loop_start:f.loop_start + f.loop_total] = k + 100
    tl = np.zeros(nl, np.float32); tgf = np.array(o['tg'])
    for f in me.polygons: tl[f.loop_start:f.loop_start + f.loop_total] = tgf[f.index]
    b = me.color_attributes.new('nd', 'FLOAT_COLOR', 'CORNER'); b.data.foreach_set('color', np.c_[kk, tl, kk, np.ones(nl)].astype(np.float32).ravel())
    old = me.uv_layers[0]; old.name = 'old'
    new = me.uv_layers.new(name='UVMap'); me.uv_layers.active = new
if not os.environ.get('NOUV'):   # NOUV=1: keep the model's own UVs, just bake on them
    # painted ("hot") faces: where the old liveries differ, plates, and the rear side panels; they get far more texture room
    from PIL import Image as PI
    oldimg = {n: np.asarray(PI.open(p).convert('RGB')).astype(np.float32) / 255 for n, p in maps if n in ('blue', 'red', 'white', 'black')}
    HOT_K, COLD_K = float(os.environ.get('HOT_K', 2.4)), float(os.environ.get('COLD_K', 0.8))
    hot = {}
    for o in meshes:
        me = o.data; W = o.matrix_world
        uv = np.zeros(len(me.loops) * 2, np.float32); me.uv_layers['old'].data.foreach_get('uv', uv); uv = uv.reshape(-1, 2)
        fl = np.array(o['plate'], bool) if 'plate' in o.keys() else np.zeros(len(me.polygons), bool)
        h = np.zeros(len(me.polygons), bool); dv = np.zeros(len(me.polygons), np.float32)
        for f in me.polygons:
            mt = me.materials[f.material_index] if f.material_index < len(me.materials) else None
            if not (mt and mt.name in texmat): continue
            if fl[f.index]: h[f.index] = True; continue
            lu = uv[f.loop_start:f.loop_start + f.loop_total]; cu = lu.mean(0)
            pts = [cu] + [cu * 0.4 + p * 0.6 for p in lu]
            def px(im, u):
                R0 = im.shape[0]; return im[int(np.clip((1 - u[1] % 1) * R0, 0, R0 - 1)), int(np.clip((u[0] % 1) * R0, 0, R0 - 1))]
            ds = []
            for u in pts:
                d = 0
                if 'blue' in oldimg and 'red' in oldimg: d = max(d, np.abs(px(oldimg['blue'], u) - px(oldimg['red'], u)).max())
                if 'white' in oldimg and 'black' in oldimg: d = max(d, np.abs(px(oldimg['white'], u) - px(oldimg['black'], u)).max())
                ds.append(d)
            d = float(np.median(ds)); dv[f.index] = d
            c = (W @ f.center) / 3.355                       # metres: x right, y forward, z up (as the pos bake)
            side = o.name.split('.')[0] == 'Bike' and -0.85 < c.y < -0.25 and 0.58 < c.z < 0.92 and abs(c.x) > 0.075
            if 'enduro' in dst and c.z > 0.76 and c.y < -0.42: side = False
            h[f.index] = d > 0.08 or side
        # tidy by mesh neighbours: fill faces mostly surrounded by plastic, drop lone specks (plates stay)
        bm = bmesh.new(); bm.from_mesh(me); bm.faces.ensure_lookup_table()
        nb = [[g.index for e in f.edges for g in e.link_faces if g.index != f.index] for f in bm.faces]; bm.free()
        tex = np.array([bool(me.materials[f.material_index] and me.materials[f.material_index].name in texmat) if f.material_index < len(me.materials) else False for f in me.polygons])
        for _ in range(3):
            fr = np.array([h[n].mean() if len(n) else 0 for n in nb])
            h = np.where(fl, True, np.where(h, fr >= 0.25, (fr >= 0.6) & (dv > 0.03))) & tex
        tgf = np.array(o['tg']); h = np.where(tgf > 0, tgf == (PAINT_T if TG is not None else -1), h) & tex
        hot[o.name] = h
        at = me.color_attributes['nd']; v = np.zeros(len(me.loops) * 4, np.float32); at.data.foreach_get('color', v); v = v.reshape(-1, 4)
        for f in me.polygons:
            if h[f.index]: v[f.loop_start:f.loop_start + f.loop_total, 0] += 200; v[f.loop_start:f.loop_start + f.loop_total, 2] += 200
        at.data.foreach_set('color', v.ravel())
    for x in bpy.data.objects: x.select_set(x.type == 'MESH')
    bpy.context.view_layer.objects.active = meshes[0]
    for want in (True, False):
        bpy.ops.object.mode_set(mode='EDIT')
        for o in meshes:
            bm = bmesh.from_edit_mesh(o.data); bm.faces.ensure_lookup_table()
            for f in bm.faces:
                mt = o.data.materials[f.material_index] if f.material_index < len(o.data.materials) else None
                f.select = bool(mt and mt.name in texmat) and bool(hot[o.name][f.index]) == want
            bmesh.update_edit_mesh(o.data)
        bpy.ops.uv.smart_project(angle_limit=1.15, island_margin=0.004, area_weight=0.0, scale_to_bounds=False, correct_aspect=True)
        bpy.ops.object.mode_set(mode='OBJECT')
    # scale the two sets so the painted plastics get HOT_K/COLD_K times the texel density, then pack everything together
    for o in meshes:
        me = o.data; uvl = me.uv_layers['UVMap']
        uv = np.zeros(len(me.loops) * 2, np.float32); uvl.data.foreach_get('uv', uv); uv = uv.reshape(-1, 2)
        for f in me.polygons:
            sl = slice(f.loop_start, f.loop_start + f.loop_total)
            uv[sl] = uv[sl] * (HOT_K if hot[o.name][f.index] else COLD_K) + (0 if hot[o.name][f.index] else 5)
        uvl.data.foreach_set('uv', uv.ravel())
    bpy.ops.object.mode_set(mode='EDIT')
    for o in meshes:
        bm = bmesh.from_edit_mesh(o.data)
        for f in bm.faces:
            mt = o.data.materials[f.material_index] if f.material_index < len(o.data.materials) else None
            f.select = bool(mt and mt.name in texmat)
        bmesh.update_edit_mesh(o.data)
    bpy.ops.uv.select_all(action='SELECT')
    bpy.ops.uv.pack_islands(rotate=True, margin=0.003, scale=True)
    bpy.ops.object.mode_set(mode='OBJECT')
# bake material: emission from attribute / old texture via old UV, into new UV image
bm_mat = bpy.data.materials.new('bake'); bm_mat.use_nodes = True; nt = bm_mat.node_tree
origmats = {o.name: [m for m in o.data.materials] for o in meshes}
for o in meshes:
    me = o.data
    for i, mt in enumerate(me.materials):
        if mt and mt.name in texmat: me.materials[i] = bm_mat
sc = bpy.context.scene; sc.render.engine = 'CYCLES'; sc.render.bake.margin = 8
sc.render.bake.use_selected_to_active = False
def bake(name, src=None, img_path=None, kind='EMIT', samples=1, save_png=False):
    img = bpy.data.images.new(name, R, R, float_buffer=True, alpha=False)
    for n in list(nt.nodes):
        if n.name.startswith('B_'): nt.nodes.remove(n)
    t = nt.nodes.new('ShaderNodeTexImage'); t.name = 'B_t'; t.image = img; nt.nodes.active = t
    if kind == 'EMIT':
        e = nt.nodes.new('ShaderNodeEmission'); e.name = 'B_e'
        if src == 'geo':
            g = nt.nodes.new('ShaderNodeNewGeometry'); g.name = 'B_g'; nt.links.new(g.outputs['Normal'], e.inputs['Color'])
        elif img_path:
            ti = nt.nodes.new('ShaderNodeTexImage'); ti.name = 'B_src'; ti.image = bpy.data.images.load(img_path)
            ti.image.colorspace_settings.name = 'Non-Color'
            uvn = nt.nodes.new('ShaderNodeUVMap'); uvn.name = 'B_uv'; uvn.uv_map = 'old'; nt.links.new(uvn.outputs[0], ti.inputs[0])
            nt.links.new(ti.outputs['Color'], e.inputs['Color'])
        else:
            at = nt.nodes.new('ShaderNodeAttribute'); at.name = 'B_a'; at.attribute_name = src; nt.links.new(at.outputs['Color'], e.inputs['Color'])
        nt.links.new(e.outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    else:
        nt.links.new(nt.nodes['Principled BSDF'].outputs[0], nt.nodes['Material Output'].inputs['Surface'])
    sc.cycles.samples = samples
    bpy.ops.object.bake(type=kind)
    px = np.array(img.pixels[:], dtype=np.float32).reshape(R, R, 4)[::-1, :, :3]
    np.save(os.path.join(out, name + '.npy'), px.astype(np.float16)); print('baked', name)
for x in bpy.data.objects: x.select_set(x.type == 'MESH')
bake('pos', 'pm'); bake('node', 'nd'); bake('nrm', 'geo')
for nm, p in maps: bake(nm, img_path=p)
sc.world = bpy.data.worlds.new('w'); bake('ao', kind='AO', samples=64)
open(os.path.join(out, 'names.txt'), 'w').write('\n'.join(names))
# export: original materials back, old UV removed, a placeholder image on the livery material
for o in meshes:
    me = o.data
    for i, mt in enumerate(origmats[o.name]): me.materials[i] = mt
    me.uv_layers.remove(me.uv_layers['old'])
    for nm in ('pm', 'nd'):
        if nm in me.color_attributes: me.color_attributes.remove(me.color_attributes[nm])
ph = bpy.data.images.new('paint', 4, 4); ph.pixels = [0.6] * 64
for m in bpy.data.materials:
    if m.name in texmat:
        for n in m.node_tree.nodes:
            if n.type == 'TEX_IMAGE': n.image = ph
bpy.ops.export_scene.gltf(filepath=dst, export_format='GLB', export_yup=True)
print('EXPORTED', dst, os.path.getsize(dst) // 1024, 'KB')
