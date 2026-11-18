"""Record per-node transform rest->pose (metres, rider frame) for a pose JSON. usage: -- pose.json rider.glb out.json"""
import bpy, sys, json
from mathutils import Matrix, Vector as V
a = sys.argv[sys.argv.index('--') + 1:]
pose = json.load(open(a[0]))
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=a[1])
bpy.context.view_layer.update()
rest = {n: o.matrix_world.copy() for n, o in ((o.name, o) for o in bpy.data.objects) if n in pose}
C = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
for n, k in pose.items():
    o = bpy.data.objects.get(n)
    if not o: continue
    look = V(k['look']); up = V(k['up']); right = up.cross(look).normalized(); up = look.cross(right)
    M = Matrix(((right.x, up.x, look.x, k['position'][0]), (right.y, up.y, look.y, k['position'][1]), (right.z, up.z, look.z, k['position'][2]), (0, 0, 0, 1)))
    o.matrix_basis = C @ M @ C
bpy.context.view_layer.update()
K = 3.355; S = Matrix.Scale(K, 4)
out = {}
for n in rest:
    T = bpy.data.objects[n].matrix_world @ rest[n].inverted()
    T = S.inverted() @ T @ S
    out[n] = [list(r) for r in T]
json.dump(out, open(a[2], 'w'))
print('nodes', len(out))
