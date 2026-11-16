"""Fit a third-party CC0/CC-BY model into a stock object's bounds.

    python blender/fit_import.py SRC.glb STOCKNAME [--rot=DEG] [--cap=FACES] [--tex=1024] [--stretch]

Joins the model, optionally thins it to FACES, turns it DEG about the vertical
(by default its long side follows the bounds' long side), scales it to sit in
the stock bounds (uniform unless --stretch) and stands it on the bounds floor.
Writes build/objects/STOCKNAME.glb. Only sizes come from the stock list.
"""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
from mathutils import Vector, Matrix
from common import reset, BUILD, export

FREEPACK = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def opt(k, d=None):
    return next((a.split("=", 1)[1] for a in sys.argv if a.startswith(f"--{k}=")), d)

def stock_bounds(name):
    for line in open(os.path.join(FREEPACK, "stock_sizes.tsv")):
        if line.startswith("#") or not line.strip(): continue
        n, *v = line.split("\t")
        if n == name:
            v = list(map(float, v[:6])); return Vector((v[0], v[2], v[1])), Vector((v[3], v[5], v[4]))
    raise SystemExit(f"{name} not in stock_sizes.tsv")

def main(src, name):
    lo, hi = stock_bounds(name)
    reset(); bpy.ops.import_scene.gltf(filepath=src)
    ms = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in ms: o.select_set(True)
    bpy.context.view_layer.objects.active = ms[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM"); bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for o in ms:
        for m in list(o.modifiers): o.modifiers.remove(m)
    bpy.ops.object.join(); o = bpy.context.view_layer.objects.active; o.name = name
    for e in [x for x in bpy.context.scene.objects if x is not o]: bpy.data.objects.remove(e)
    cap = int(opt("cap", "0"))
    if cap and len(o.data.polygons) > cap:
        d = o.modifiers.new("d", "DECIMATE"); d.ratio = cap / len(o.data.polygons); bpy.ops.object.modifier_apply(modifier="d")
    me = o.data
    vs = [v.co for v in me.vertices]; a = Vector(map(min, *vs)); b = Vector(map(max, *vs)); size = b - a
    rot = opt("rot")
    if rot is None:   # long side to long side
        rot = 90 if ((size.x >= size.y) != ((hi.x - lo.x) >= (hi.y - lo.y))) else 0
    me.transform(Matrix.Rotation(math.radians(float(rot)), 4, "Z"))
    vs = [v.co for v in me.vertices]; a = Vector(map(min, *vs)); b = Vector(map(max, *vs)); size = b - a
    tgt = hi - lo
    if "--stretch" in sys.argv:
        S = Matrix.Diagonal((tgt.x / size.x, tgt.y / size.y, tgt.z / size.z, 1))
    elif opt("by"):   # match one axis only (x, y or z), e.g. a tower's height
        i = "xyz".index(opt("by")); s = tgt[i] / size[i]; S = Matrix.Scale(s, 4)
    else:
        s = min(tgt.x / size.x, tgt.y / size.y, tgt.z / size.z); S = Matrix.Scale(s, 4)
    me.transform(S)
    vs = [v.co for v in me.vertices]; a = Vector(map(min, *vs)); b = Vector(map(max, *vs))
    c = Vector(((lo.x + hi.x - a.x - b.x) / 2, (lo.y + hi.y - a.y - b.y) / 2, lo.z - a.z))
    me.transform(Matrix.Translation(c))
    tex = int(opt("tex", "1024"))
    for im in bpy.data.images:
        if im.size[0] > tex or im.size[1] > tex:
            k = tex / max(im.size); im.scale(max(1, int(im.size[0] * k)), max(1, int(im.size[1] * k)))
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    print("fit", name, "faces", len(me.polygons), "size", tuple(round(x, 1) for x in (b - a)), "bounds", tuple(round(x, 1) for x in tgt))

if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    main(a[0], a[1])
