"""Fit a third-party CC0/CC-BY model into a stock object's bounds.

    python blender/fit_import.py SRC.glb STOCKNAME [--rot=DEG] [--cap=FACES] [--tex=1024] [--stretch]
                                 [--by=x|y|z] [--pick=NODE] [--rx=DEG] [--hue=LO:HI:TO[:SAT]/...] [--blot=IMG:X0,Y0,X1,Y1/...] [--keep-alpha]

Joins the model, optionally thins it to FACES, turns it DEG about the vertical
(by default its long side follows the bounds' long side), scales it to sit in
the stock bounds (uniform unless --stretch) and stands it on the bounds floor.
Materials are made opaque, --hue moves a maker's paint colours and --blot paints
over maker decals. Writes build/objects/STOCKNAME.glb. Only sizes come from the stock list.
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

def opaque():
    """Imports often flag solid parts as alpha-hashed; the game draws them solid."""
    for m in bpy.data.materials:
        if not m.node_tree: continue
        glass = any(k in m.name.lower() for k in ("glass", "transp", "window"))
        for n in m.node_tree.nodes:
            if n.type == "BSDF_PRINCIPLED":
                for l in list(n.inputs["Alpha"].links): m.node_tree.links.remove(l)
                n.inputs["Alpha"].default_value = 1.0
                if glass:   # see-through glass becomes dark tinted glass, drawn solid
                    for l in list(n.inputs["Base Color"].links): m.node_tree.links.remove(l)
                    n.inputs["Base Color"].default_value = (0.03, 0.035, 0.04, 1); n.inputs["Roughness"].default_value = 0.08
                    n.inputs["Metallic"].default_value = 0.0
        if hasattr(m, "blend_method"): m.blend_method = "OPAQUE"

def base_images():
    out = []
    for m in bpy.data.materials:
        if not m.node_tree: continue
        b = next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        l = b and b.inputs["Base Color"].links
        node = l[0].from_node if l else None
        seen = 0
        while node is not None and node.type != "TEX_IMAGE" and seen < 6:     # through mix / multiply nodes
            nxt = [k.from_node for i in node.inputs for k in i.links]
            node = next((x for x in nxt if x.type == "TEX_IMAGE"), nxt[0] if nxt else None); seen += 1
        if node is not None and node.type == "TEX_IMAGE" and node.image not in out: out.append(node.image)
    return out

def pixels(im):
    import numpy as np
    a = np.empty(im.size[0] * im.size[1] * 4, np.float32); im.pixels.foreach_get(a)
    return a.reshape(im.size[1], im.size[0], 4)

def store(im, a):
    im.pixels.foreach_set(a.ravel()); im.update(); im.pack()

def recolor(spec):
    """--hue=LO:HI:TO[:SAT]/... moves hues in LO..HI degrees to TO (and scales
    saturation by SAT), to take a real maker's paint scheme off a model."""
    import numpy as np, colorsys
    rules = [list(map(float, r.split(":"))) for r in spec.split("/")]
    for m in bpy.data.materials:   # plain paint colours (no texture on Base Color)
        b = m.node_tree and next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if not b or b.inputs["Base Color"].links: continue
        r, g, bl, a = b.inputs["Base Color"].default_value; h, sat, v = colorsys.rgb_to_hsv(r, g, bl); h *= 360
        for lo, hi, to, *k in rules:
            if lo <= h <= hi and sat > 0.15: h = to; sat = min(1, sat * (k[0] if k else 1)); break
        b.inputs["Base Color"].default_value = (*colorsys.hsv_to_rgb(h / 360, sat, v), a)
    for im in base_images():
        a = pixels(im); rgb = a[..., :3]
        mx = rgb.max(-1); mn = rgb.min(-1); d = mx - mn + 1e-6
        r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
        h = np.where(mx == r, (g - b) / d % 6, np.where(mx == g, (b - r) / d + 2, (r - g) / d + 4)) * 60
        s = np.where(mx > 0, (mx - mn) / (mx + 1e-6), 0); v = mx
        for lo, hi, to, *k in rules:
            m = (h >= lo) & (h <= hi) & (s > 0.15)
            h = np.where(m, to, h); s = np.where(m, np.clip(s * (k[0] if k else 1), 0, 1), s)
        c = v * s; x = c * (1 - abs((h / 60) % 2 - 1)); z = np.zeros_like(c); i = (h // 60).astype(int) % 6
        rr = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5], [c, x, z, z, x, c])
        gg = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5], [x, c, c, x, z, z])
        bb = np.select([i == 0, i == 1, i == 2, i == 3, i == 4, i == 5], [z, z, x, c, c, x])
        a[..., 0], a[..., 1], a[..., 2] = rr + v - c, gg + v - c, bb + v - c
        store(im, a)

def blot(spec):
    """--blot=IMG:X0,Y0,X1,Y1/... paints over a maker's decal (box in 0..1
    image coords, y down) with the colour around it."""
    import numpy as np
    ims = {im.name: im for im in base_images()}
    for r in spec.split("/"):
        nm, box = r.split(":"); im = list(ims.values())[int(nm)] if nm.isdigit() else ims[nm]
        a = pixels(im); H, W = a.shape[:2]
        x0, y0, x1, y1 = [float(t) for t in box.split(",")]
        c0, c1 = int(x0 * W), int(x1 * W); r1, r0 = H - int(y0 * H), H - int(y1 * H)   # pixels are bottom-up
        p = 3; ring = np.concatenate([a[max(r0 - p, 0):r0, c0:c1].reshape(-1, 4), a[r1:r1 + p, c0:c1].reshape(-1, 4),
                                     a[r0:r1, max(c0 - p, 0):c0].reshape(-1, 4), a[r0:r1, c1:c1 + p].reshape(-1, 4)])
        a[r0:r1, c0:c1] = np.median(ring, 0)
        noise = (np.random.default_rng(0).random((r1 - r0, c1 - c0, 1)) - 0.5) * 0.04
        a[r0:r1, c0:c1, :3] += noise
        store(im, a)

def main(src, name):
    lo, hi = stock_bounds(name)
    reset(); bpy.ops.import_scene.gltf(filepath=src)
    if opt("pick"):   # a pack of models: keep the one under the node named PICK
        root = next(o for o in bpy.context.scene.objects if o.name == opt("pick") or o.name.startswith(opt("pick") + "."))
        keep = {root, *root.children_recursive}
        for o in [o for o in bpy.context.scene.objects if o.type == "MESH" and o not in keep]: bpy.data.objects.remove(o)
    for o in [o for o in bpy.context.scene.objects if o.type == "MESH" and any(k in o.name.lower() for k in ("shadow", "flare"))]:
        bpy.data.objects.remove(o)   # baked ground shadows and light flares are display tricks, not parts
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
    if opt("rx"): me.transform(Matrix.Rotation(math.radians(float(opt("rx"))), 4, "X"))   # a model stood on its end
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
    if "--keep-alpha" not in sys.argv: opaque()
    if opt("hue"): recolor(opt("hue"))
    if opt("blot"): blot(opt("blot"))
    tex = int(opt("tex", "1024"))
    for im in bpy.data.images:
        if im.size[0] > tex or im.size[1] > tex:
            k = tex / max(im.size); im.scale(max(1, int(im.size[0] * k)), max(1, int(im.size[1] * k)))
    path = os.path.join(BUILD, "objects", name + ".glb"); export(path)
    print("fit", name, "faces", len(me.polygons), "size", tuple(round(x, 1) for x in (b - a)), "bounds", tuple(round(x, 1) for x in tgt))

if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    main(a[0], a[1])
