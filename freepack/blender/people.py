"""Track-side people (flag marshal, 30-second board man) from the CC-BY base body
used for the rider (see CREDITS), in its own clothes, arms posed for the job.

    python blender/people.py SRC.glb [flaggirl 30secman]
"""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy
from mathutils import Vector, Matrix
from common import reset, BUILD, export, flat, box, cyl
import rider_import as R
from fit_import import stock_bounds

DROP = ("Tearline", "Eye_Occlusion", "Cornea")   # thin transparent layers that render as grey film

def pose_person(arm, kind):
    sh = lambda s: R.wpos(arm, R.bone(arm, f"CC_Base_{s}_Upperarm"))
    for s, sx in (("L", -1), ("R", 1)):
        ua, fa = R.bone(arm, f"CC_Base_{s}_Upperarm"), R.bone(arm, f"CC_Base_{s}_Forearm")
        p = sh(s)
        if kind == "30secman":     # both arms up holding the board overhead
            R.aim(arm, ua, p + Vector((sx * 0.35, 0.15, 1.0))); R.aim(arm, fa, R.wpos(arm, fa) + Vector((-sx * 0.25, 0.1, 0.9)))
        elif kind == "flaggirl" and s == "R":   # flag arm raised out to the side
            R.aim(arm, ua, p + Vector((sx * 0.8, 0.2, 0.6))); R.aim(arm, fa, R.wpos(arm, fa) + Vector((sx * 0.6, 0.1, 0.8)))
        else:                       # relaxed, hanging
            R.aim(arm, ua, p + Vector((sx * 0.18, 0.02, -1.0))); R.aim(arm, fa, R.wpos(arm, fa) + Vector((sx * 0.05, 0.12, -1.0)))
    return R.wpos(arm, R.bone(arm, "CC_Base_R_Hand")), R.wpos(arm, R.bone(arm, "CC_Base_L_Hand"))

def build(src, name):
    lo, hi = stock_bounds(name)
    reset(); bpy.ops.import_scene.gltf(filepath=src)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    for o in list(bpy.data.objects):
        if o.type == "MESH" and (o.name == "Icosphere" or any(k in (o.data.materials[0].name if o.data.materials else "") for k in DROP)):
            bpy.data.objects.remove(o, do_unlink=True)
    root = arm
    while root.parent: root = root.parent
    k = 5.8 / 1.87 * (1 / 3.28084)     # to a 5.8 ft person, in feet (the model is metres and faces -Y)
    root.matrix_world = Matrix.Translation(Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, max(lo.z, 0)))) @ Matrix.Rotation(math.pi, 4, "Z") @ Matrix.Scale(3.28084 * k * 3.28084 / 3.28084, 4) @ root.matrix_world
    bpy.context.view_layer.update()
    hr, hl = pose_person(arm, name)
    dg = bpy.context.evaluated_depsgraph_get()
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        me = bpy.data.meshes.new_from_object(o.evaluated_get(dg)); me.transform(o.matrix_world)
        n = bpy.data.objects.new(o.name + "_baked", me); bpy.context.scene.collection.objects.link(n)
    for o in [o for o in bpy.data.objects if not o.name.endswith("_baked")]: bpy.data.objects.remove(o, do_unlink=True)
    if name == "flaggirl":
        cyl(hr - Vector((0, 0, 0.5)), hr + Vector((0, 0, 3.2)), 0.04, flat("flagpole", (0.6, 0.5, 0.35), 0.6), 8)
        box(hr + Vector((0.02, -0.02, 1.9)), hr + Vector((2.2, 0.02, 3.2)), flat("flaggreen", (0.08, 0.5, 0.16), 0.8))
    else:
        c = (hr + hl) / 2
        box(c + Vector((-1.5, -0.06, 0.1)), c + Vector((1.5, 0.0, 2.1)), flat("boardwhite", (0.9, 0.9, 0.88), 0.5))
        from objects import _sign_png
    bpy.ops.object.select_all(action="SELECT"); bpy.context.view_layer.objects.active = bpy.context.selected_objects[0]
    bpy.ops.object.join()
    for im in bpy.data.images:
        if max(im.size) > 1024: im.scale(1024, 1024)
    export(os.path.join(BUILD, "objects", name + ".glb")); print("person", name)

if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    for n in a[1:] or ["flaggirl", "30secman"]: build(a[0], n)
