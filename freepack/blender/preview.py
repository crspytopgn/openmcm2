"""Render a contact sheet of built .glb files: python blender/preview.py out.png a.glb b.glb ..."""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
from mathutils import Vector
from common import *
out = sys.argv[1]; files = sys.argv[2:]
reset(); sc = bpy.context.scene
x = 0.0; H = 0.0; D = 0.0
for i, f in enumerate(files):
    before = set(bpy.data.objects); bpy.ops.import_scene.gltf(filepath=f)
    objs = [o for o in bpy.data.objects if o not in before]
    bpy.context.view_layer.update()
    lo, hi = Vector((1e9,) * 3), Vector((-1e9,) * 3)
    for o in objs:
        if o.type != "MESH": continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c); lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
    base = 0.0 if lo.z > -1 else 0.0
    off = Vector((x - lo.x, -(lo.y + hi.y) / 2, -lo.z if lo.z > -1 else 0))
    for o in objs:
        if o.parent is None: o.location += off
    x += (hi.x - lo.x) + 6; H = max(H, hi.z); D = max(D, hi.y - lo.y)
W = x - 6
bpy.ops.mesh.primitive_plane_add(size=1, location=(W / 2, 0, 0)); g = bpy.context.object; g.scale = (W * 4, W * 4, 1)
g.data.materials.append(acg("Ground051", 0.05, name="ground"))
w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True; nt = w.node_tree
env = nt.nodes.new("ShaderNodeTexEnvironment"); env.image = bpy.data.images.load(os.path.join(CACHE, "hdri", "kloofendal_48d_partly_cloudy_puresky_2k.hdr"))
nt.links.new(env.outputs["Color"], nt.nodes["Background"].inputs["Color"])
cam = bpy.data.objects.new("c", bpy.data.cameras.new("c")); sc.collection.objects.link(cam); sc.camera = cam
tgt = Vector((W / 2, 0, H * 0.35)); dist = max(W * 0.95, H * 2.2)
cam.location = tgt + Vector((-0.12 * dist, -0.95 * dist, 0.32 * dist)); cam.rotation_euler = (tgt - cam.location).to_track_quat("-Z", "Y").to_euler(); cam.data.lens = 40
sc.render.engine = "CYCLES"; sc.cycles.samples = int(os.environ.get("SAMPLES", "24")); sc.cycles.use_denoising = True
sc.render.resolution_x = 1600; sc.render.resolution_y = 900
sc.view_settings.view_transform = "AgX"; sc.view_settings.look = "AgX - Medium Low Contrast"
sc.render.filepath = out; bpy.ops.render.render(write_still=True)
