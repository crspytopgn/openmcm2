"""Shared helpers for the Free Pack's Blender build scripts.

Model space: build in Blender with X = game x, Y = game z, Z = game y (up),
in the game's units (feet). The glTF exporter's +Y-up conversion then gives
glTF (x, y, z) = game (x, y, -z), the same flip as the engine's from_game.
"""
import bpy, bmesh, math, os
from mathutils import Vector

FREEPACK = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.path.join(FREEPACK, "cache")
BUILD = os.path.join(FREEPACK, "build")
TEX_SIZE = int(os.environ.get("FP_TEX", "1024"))

def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)

def _img(path, non_color):
    key = (path, non_color)
    im = bpy.data.images.load(path, check_existing=True)
    if non_color: im.colorspace_settings.name = "Non-Color"
    if max(im.size) > TEX_SIZE and not im.get("fp_scaled"):
        im.scale(TEX_SIZE, TEX_SIZE); im["fp_scaled"] = True
    return im

def derived_color(mid, tint=None, ramp=None):
    """glTF keeps only plain image textures, so colour changes are baked
    into a new image: tint multiplies, ramp maps grey to a dark/light pair."""
    import numpy as np
    from PIL import Image
    src = os.path.join(CACHE, "ambientcg", mid, f"{mid}_2K-JPG_Color.jpg")
    if not tint and not ramp: return src
    key = f"{mid}_{abs(hash((tint, ramp))) % 10**8}"
    out = os.path.join(BUILD, "tex", key + ".jpg")
    if os.path.exists(out): return out
    a = np.asarray(Image.open(src).convert("RGB")).astype(np.float32) / 255
    lin = a ** 2.2
    if ramp:
        g = (lin @ np.array([0.2126, 0.7152, 0.0722], np.float32))
        lo_, hi_ = np.percentile(g, 2), np.percentile(g, 98)
        t = np.clip((g - lo_) / max(hi_ - lo_, 1e-4), 0, 1)[..., None]
        lin = np.array(ramp[0], np.float32) * (1 - t) + np.array(ramp[1], np.float32) * t
    if tint: lin = lin * np.array(tint, np.float32)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    Image.fromarray((np.clip(lin, 0, 1) ** (1 / 2.2) * 255 + 0.5).astype(np.uint8)).save(out, quality=92)
    return out

def acg(mid, scale=1.0, tint=None, rough=None, metal=0.0, name=None, box=False, sat=1.0, ramp=None):
    """Material from an ambientCG set, UV (or box-projected object) mapped."""
    name = name or f"{mid}_{scale}_{tint}"
    if name in bpy.data.materials: return bpy.data.materials[name]
    m = bpy.data.materials.new(name); m.use_nodes = True; nt = m.node_tree
    b = nt.nodes["Principled BSDF"]
    base = os.path.join(CACHE, "ambientcg", mid, f"{mid}_2K-JPG")
    tc = nt.nodes.new("ShaderNodeTexCoord"); mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (scale, scale, scale)
    nt.links.new(tc.outputs["UV"], mp.inputs["Vector"])
    def tex(suf, nc):
        p = f"{base}_{suf}.jpg"
        if not os.path.exists(p): return None
        t = nt.nodes.new("ShaderNodeTexImage"); t.image = _img(p, nc)
        nt.links.new(mp.outputs["Vector"], t.inputs["Vector"]); return t
    c = nt.nodes.new("ShaderNodeTexImage"); c.image = _img(derived_color(mid, tint, ramp), False)
    nt.links.new(mp.outputs["Vector"], c.inputs["Vector"])
    nt.links.new(c.outputs["Color"], b.inputs["Base Color"])
    r = tex("Roughness", True)
    if rough is not None or r is None: b.inputs["Roughness"].default_value = rough if rough is not None else 0.8
    else: nt.links.new(r.outputs["Color"], b.inputs["Roughness"])
    n = tex("NormalGL", True)
    if n:
        nm = nt.nodes.new("ShaderNodeNormalMap"); nt.links.new(n.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], b.inputs["Normal"])
    b.inputs["Metallic"].default_value = metal
    b.inputs["Specular IOR Level"].default_value = 0.3
    return m

def image_mat(path, name, rough=0.75):
    if name in bpy.data.materials: return bpy.data.materials[name]
    m = bpy.data.materials.new(name); m.use_nodes = True; nt = m.node_tree; b = nt.nodes["Principled BSDF"]
    t = nt.nodes.new("ShaderNodeTexImage"); t.image = _img(path, False)
    nt.links.new(t.outputs["Color"], b.inputs["Base Color"]); b.inputs["Roughness"].default_value = rough
    b.inputs["Specular IOR Level"].default_value = 0.25
    return m

def flat(name, rgb, rough=0.7, metal=0.0):
    if name in bpy.data.materials: return bpy.data.materials[name]
    m = bpy.data.materials.new(name); m.use_nodes = True; b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1); b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal; b.inputs["Specular IOR Level"].default_value = 0.3
    return m

def box(lo, hi, mat, bevel=0.0, uv_scale=1.0, name="box"):
    """Axis box from corner lo to hi (Blender coords), world-size UVs."""
    lo, hi = Vector(lo), Vector(hi)
    bm = bmesh.new(); bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts: v.co = lo + Vector(((v.co.x + .5) * (hi.x - lo.x), (v.co.y + .5) * (hi.y - lo.y), (v.co.z + .5) * (hi.z - lo.z)))
    if bevel > 0:
        bmesh.ops.bevel(bm, geom=list(bm.edges), offset=bevel, segments=2, affect="EDGES", profile=0.5)
    _box_uv(bm, uv_scale)
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat)
    return o

def _box_uv(bm, s):
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        n = f.normal; ax = max(range(3), key=lambda i: abs(n[i]))
        for l in f.loops:
            c = l.vert.co
            u, v = [(c.y, c.z), (c.x, c.z), (c.x, c.y)][ax]
            l[uv].uv = (u / s, v / s)

def cyl(a, b, r, mat, seg=16, name="cyl", r2=None, cap=True, uv_scale=1.0):
    a, b = Vector(a), Vector(b); d = b - a
    bm = bmesh.new(); bmesh.ops.create_cone(bm, cap_ends=cap, segments=seg, radius1=r, radius2=r if r2 is None else r2, depth=d.length)
    _cyl_uv(bm, r, d.length, uv_scale)
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    for p in me.polygons: p.use_smooth = abs(p.normal.z) < 0.9
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); me.materials.append(mat)
    o.location = (a + b) / 2; o.rotation_euler = d.to_track_quat("Z", "Y").to_euler()
    return o

def _cyl_uv(bm, r, h, s):
    uv = bm.loops.layers.uv.verify()
    for f in bm.faces:
        for l in f.loops:
            c = l.vert.co
            if abs(f.normal.z) > 0.9: l[uv].uv = (c.x / s, c.y / s)
            else:
                a = math.atan2(c.y, c.x); l[uv].uv = (a * r / s, (c.z + h / 2) / s)

def join_all(name):
    objs = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in objs: o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    for o in objs:
        for m in o.modifiers:
            bpy.context.view_layer.objects.active = o
            bpy.ops.object.modifier_apply(modifier=m.name)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join(); o = bpy.context.view_layer.objects.active; o.name = name
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return o

def export(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", export_image_format="JPEG",
                              export_jpeg_quality=88, export_apply=True, export_yup=True)

def bounds():
    bpy.context.view_layer.update()
    lo = Vector((1e9,) * 3); hi = Vector((-1e9,) * 3)
    for o in bpy.context.scene.objects:
        if o.type != "MESH": continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
    return lo, hi
