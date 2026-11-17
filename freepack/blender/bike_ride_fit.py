"""Bring a free bike's rider contact points to the original game's bike, so
the game's own riding motions put the rider's seat and hands where they
belong: stretch the bodywork above the frame rails up to the original's
seat and plastics height, and lift and widen the bar ends to its grips.

    python blender/bike_ride_fit.py SRC.glb OUT.glb SEAT_FROM SEAT_K BAR_FROM BAR_K WIDE_K

Heights in game feet (y up); SEAT_FROM/BAR_FROM: where the stretch starts;
*_K: how much it stretches; WIDE_K: bar width factor beyond 0.4 ft out.
Measured against the original with mm2-formats' part_bounds example.
"""
import sys, bpy
a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[-7:]
src, out = a[0], a[1]; seat_from, seat_k, bar_from, bar_k, wide_k = map(float, a[2:7])
bpy.ops.wm.read_factory_settings(use_empty=True); bpy.ops.import_scene.gltf(filepath=src)
for o in bpy.data.objects:
    if o.type != "MESH": continue
    part = o.name.split(".")[0]
    if part not in ("Bike", "HandleBars"): continue
    mw = o.matrix_world; inv = mw.inverted()
    for v in o.data.vertices:
        p = mw @ v.co                   # Blender Z = game y
        if part == "Bike" and p.z > seat_from:
            p.z = seat_from + (p.z - seat_from) * seat_k
        if part == "HandleBars" and p.z > bar_from:
            p.z = bar_from + (p.z - bar_from) * bar_k
            if abs(p.x) > 0.4: p.x = (1 if p.x > 0 else -1) * (0.4 + (abs(p.x) - 0.4) * wide_k)
        v.co = inv @ p
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_image_format="JPEG", export_jpeg_quality=90, export_apply=True, export_yup=True)
print("fitted", out)
