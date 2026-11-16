"""Free stand-ins for the stock eco-system plants (trees, bushes, cacti,
grass) that tracks scatter by species name. Each is a light, camera-facing
cross of three cards textured with a render of a detailed CC0 model (Poly
Haven) or a procedural plant, fitted to the stock species' size.

    python blender/eco.py [species ...]   ->  build/eco/<species>.glb
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
from mathutils import Vector
from common import *

PH = os.path.join(CACHE, "polyhaven")
# species stem -> (source, colour treatment)
SPECIES = {
    "pine": ("ph:fir_tree_01", None), "snowpine": ("ph:fir_tree_01", "snow"),
    "cypress": ("ph:pine_sapling_small", None), "snowcypress": ("ph:pine_sapling_small", "snow"),
    "aspen": ("ph:tree_small_02", None), "snowaspen": ("ph:tree_small_02", "bare"),
    "tree": ("ph:island_tree_02", None), "wideleaf": ("ph:jacaranda_tree", "green"),
    "orchard": ("ph:tree_small_02", None), "citrus": ("ph:tree_small_02", None), "snowcitrus": ("ph:tree_small_02", "snow"),
    "crunchberry": ("ph:searsia_lucida", None), "shrub": ("ph:searsia_lucida", None),
    "mesquite": ("ph:island_tree_02", None), "drymesquite": ("ph:tree_small_02", "dry"),
    "saguaro": ("proc:saguaro", None), "drysaguaro": ("proc:saguaro", "dry"),
    "ocotillo": ("proc:ocotillo", None), "dryocotillo": ("proc:ocotillo", "dry"),
    "palm": ("proc:palm", None), "smpalm": ("proc:palm", None), "palmetto": ("proc:palmetto", None),
    "tumbleweed": ("proc:tumbleweed", None), "tumblweed": ("proc:tumbleweed", None), "drytumbleweed": ("proc:tumbleweed", None),
    "grass": ("ph:grass_medium_01", "clump"),
}

def stem(name):
    s = name.lower().split(".")[0]
    return s.rstrip("0123456789"), int("".join(c for c in s if c.isdigit()) or 1)

def import_ph(asset, variant):
    d = os.path.join(PH, asset); g = [f for f in os.listdir(d) if f.endswith(".gltf")][0]
    before = set(bpy.data.objects); bpy.ops.import_scene.gltf(filepath=os.path.join(d, g))
    objs = [o for o in bpy.data.objects if o not in before]
    roots = [o for o in objs if o.parent is None]
    while len(roots) == 1 and roots[0].type != "MESH" and len(roots[0].children) > 0:
        roots = list(roots[0].children)
    roots = [r for r in roots if "LOD" not in r.name or "LOD0" in r.name]
    if len(roots) > 1:
        keep = roots[(variant - 1) % len(roots)]
        keepset = {keep} | set(keep.children_recursive)
        bpy.context.view_layer.update(); mw = keep.matrix_world.copy()
        for o in objs:
            if o not in keepset: bpy.data.objects.remove(o, do_unlink=True)
        keep.parent = None; keep.matrix_world = mw; keep.location.x = keep.location.y = 0
    # skip LOD1+ meshes if present
    for o in list(bpy.context.scene.objects):
        if "LOD" in o.name and "LOD0" not in o.name: bpy.data.objects.remove(o, do_unlink=True)

def import_clump(asset, variant):
    d = os.path.join(PH, asset); g = [f for f in os.listdir(d) if f.endswith(".gltf")][0]
    bpy.ops.import_scene.gltf(filepath=os.path.join(d, g))
    rnd = random.Random(variant)
    tufts = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.parent is None]
    for o in tufts: o.location = (rnd.uniform(-0.12, 0.12), rnd.uniform(-0.12, 0.12), 0); o.rotation_euler.z = rnd.uniform(0, 6.3)

def mat_rgb(name, rgb, rough=0.8):
    return flat(name, rgb, rough)

def proc_saguaro(v):
    rnd = random.Random(v); g = mat_rgb("cactus", (0.07, 0.12, 0.05))
    def ribbed(a, b, r):
        o = cyl(a, b, r, g, 24, r2=r * 0.9)
        me = o.data; bm = bmesh.new(); bm.from_mesh(me)
        for vt in bm.verts:
            ang = math.atan2(vt.co.y, vt.co.x); k = 1 + 0.08 * math.cos(ang * 12)
            vt.co.x *= k; vt.co.y *= k
        bm.to_mesh(me); bm.free()
        for p in me.polygons: p.use_smooth = True
    H = 1.0; ribbed((0, 0, 0), (0, 0, H), 0.09); bpy.ops.mesh.primitive_uv_sphere_add(radius=0.088, location=(0, 0, H)); bpy.context.object.data.materials.append(g)
    for i in range(rnd.randint(1, 3)):
        side = rnd.choice((-1, 1)); z = rnd.uniform(0.35, 0.6); out = rnd.uniform(0.16, 0.24); top = z + rnd.uniform(0.2, 0.35)
        ang = rnd.uniform(0, math.pi)
        dx, dy = math.cos(ang) * side, math.sin(ang) * side
        ribbed((0, 0, z), (dx * out, dy * out, z + 0.03), 0.06)
        ribbed((dx * out, dy * out, z), (dx * out, dy * out, top), 0.06)
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.058, location=(dx * out, dy * out, top)); bpy.context.object.data.materials.append(g)

def proc_ocotillo(v):
    rnd = random.Random(v); g = mat_rgb("ocotillo", (0.2, 0.18, 0.12))
    for i in range(22):
        a = rnd.uniform(0, 2 * math.pi); lean = rnd.uniform(0.15, 0.4); L = rnd.uniform(0.7, 1.0)
        tip = (math.cos(a) * lean * L, math.sin(a) * lean * L, L)
        cyl((0, 0, 0), tip, 0.012, g, 6, r2=0.005)

def _frond_texture():
    from PIL import Image, ImageDraw
    p = os.path.join(BUILD, "tex", "frond.png")
    if os.path.exists(p): return p
    W, H = 256, 1024; im = Image.new("RGBA", (W, H), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.line([(W / 2, 0), (W / 2, H)], fill=(80, 70, 35, 255), width=6)
    rnd = random.Random(3)
    for y in range(10, H, 14):
        L = (W / 2 - 6) * math.sin(math.pi * y / H) ** 0.6
        for s in (-1, 1):
            col = (int(rnd.uniform(45, 70)), int(rnd.uniform(70, 95)), int(rnd.uniform(30, 45)), 255)
            d.polygon([(W / 2, y), (W / 2 + s * L, y + 40), (W / 2 + s * L * 0.95, y + 46), (W / 2, y + 8)], fill=col)
    os.makedirs(os.path.dirname(p), exist_ok=True); im.save(p); return p

def _frond_mat():
    m = image_mat(_frond_texture(), "frond", 0.8)
    nt = m.node_tree; t = [n for n in nt.nodes if n.type == "TEX_IMAGE"][0]
    nt.links.new(t.outputs["Alpha"], nt.nodes["Principled BSDF"].inputs["Alpha"]); return m

def proc_palm(v, trunk=True):
    rnd = random.Random(v); bark = acg("Wood049", 2.0, ramp=((0.08, 0.06, 0.04), (0.35, 0.28, 0.2)), name="palmbark")
    H = 1.0 if trunk else 0.08
    if trunk:
        pts = [Vector((0.06 * math.sin(t * 2.5) * rnd.uniform(0.5, 1), 0, t * H)) for t in [i / 8 for i in range(9)]]
        for a, b in zip(pts, pts[1:]): cyl(a, b, 0.045 - 0.015 * a.z, bark, 10)
    top = Vector((0.03, 0, H)); fm = _frond_mat()
    n = 14 if trunk else 10
    for i in range(n):
        a = 2 * math.pi * i / n + rnd.uniform(-0.2, 0.2); L = rnd.uniform(0.45, 0.6) * (1 if trunk else 0.7); droop = rnd.uniform(0.2, 0.5)
        bm = bmesh.new(); uv = bm.loops.layers.uv.verify(); rows = 6; vs = []
        for k in range(rows + 1):
            t = k / rows; r = L * t; z = (0.12 * t - droop * t * t) * (1 if trunk else 0.2) + (0 if trunk else 0.3 * t)
            c = top + Vector((math.cos(a) * r, math.sin(a) * r, z)); w = 0.12 * math.sin(math.pi * min(t + 0.1, 1)) + 0.02
            px = Vector((-math.sin(a), math.cos(a), 0)) * w
            vs.append((bm.verts.new(c - px), bm.verts.new(c + px)))
        for k in range(rows):
            f = bm.faces.new((vs[k][0], vs[k][1], vs[k + 1][1], vs[k + 1][0]))
            for l, (u, vv) in zip(f.loops, ((0, k / rows), (1, k / rows), (1, (k + 1) / rows), (0, (k + 1) / rows))): l[uv].uv = (u, 1 - vv)
        me = bpy.data.meshes.new("frond"); bm.to_mesh(me); bm.free()
        o = bpy.data.objects.new("frond", me); bpy.context.scene.collection.objects.link(o); me.materials.append(fm)

def proc_palmetto(v):
    proc_palm(v, trunk=False)

def proc_tumbleweed(v):
    rnd = random.Random(v); g = mat_rgb("tumble", (0.42, 0.33, 0.2))
    for i in range(140):
        a = Vector((rnd.gauss(0, 1), rnd.gauss(0, 1), rnd.gauss(0, 1))).normalized() * rnd.uniform(0.1, 0.5)
        b = a + Vector((rnd.gauss(0, 1), rnd.gauss(0, 1), rnd.gauss(0, 1))).normalized() * rnd.uniform(0.15, 0.35)
        b = b.normalized() * min(b.length, 0.5)
        cyl(a + Vector((0, 0, 0.5)), b + Vector((0, 0, 0.5)), 0.006, g, 4)

def treat(kind):
    """Bake a colour treatment into the source's base-colour images."""
    if not kind: return
    import numpy as np
    for im in list(bpy.data.images):
        if im.colorspace_settings.name == "Non-Color" or im.size[0] == 0: continue
        px = np.array(im.pixels[:], np.float32).reshape(-1, 4)
        c = px[:, :3]; g = c @ np.array([0.3, 0.59, 0.11], np.float32)
        if kind == "dry": c = g[:, None] * np.array([1.15, 0.95, 0.7], np.float32)
        elif kind == "snow": c = c * 0.55 + np.array([0.85, 0.87, 0.9], np.float32) * 0.45
        elif kind == "bare": c = g[:, None] * np.array([1.05, 0.95, 0.85], np.float32)
        elif kind == "green": c = g[:, None] * np.array([0.6, 1.15, 0.45], np.float32)
        px[:, :3] = np.clip(c, 0, 1); im.pixels[:] = px.ravel()

def render_card(path, res_w, res_h):
    """Orthographic front render of the scene, transparent background, soft
    even sky light so the card carries gentle shading but no hard shadows."""
    sc = bpy.context.scene; lo, hi = bounds()
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam")); sc.collection.objects.link(cam); sc.camera = cam
    cam.data.type = "ORTHO"; size = max((hi.x - lo.x) * res_h / res_w, hi.z - lo.z) * 1.02
    cam.data.ortho_scale = size; cam.location = ((lo.x + hi.x) / 2, lo.y - 50, (lo.z + hi.z) / 2); cam.rotation_euler = (math.pi / 2, 0, 0)
    cam.data.clip_end = 500
    w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True
    w.node_tree.nodes["Background"].inputs["Color"].default_value = (0.9, 0.93, 1.0, 1); w.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.8
    bpy.ops.object.light_add(type="SUN", rotation=(math.radians(50), 0, math.radians(-30))); bpy.context.object.data.energy = 2.0; bpy.context.object.data.angle = math.radians(25)
    sc.render.engine = "CYCLES"; sc.cycles.samples = 24; sc.cycles.use_denoising = True; sc.render.film_transparent = True
    sc.render.resolution_x, sc.render.resolution_y = res_w, res_h
    sc.view_settings.view_transform = "Standard"; sc.render.image_settings.file_format = "PNG"; sc.render.image_settings.color_mode = "RGBA"
    sc.render.filepath = path; bpy.ops.render.render(write_still=True)
    return size

def bleed(path):
    """Spread edge colour into the transparent area so mipmaps don't halo."""
    import numpy as np
    from PIL import Image, ImageFilter
    im = Image.open(path).convert("RGBA"); a = np.asarray(im).astype(np.float32)
    rgb, al = a[..., :3], a[..., 3:] / 255
    acc = rgb * al; w = al.copy()
    for r in (2, 4, 8, 16, 32):
        acc_b = np.asarray(Image.fromarray(np.uint8(np.clip(acc, 0, 255))).filter(ImageFilter.BoxBlur(r))).astype(np.float32)
        w_b = np.asarray(Image.fromarray(np.uint8(w[..., 0] * 255)).filter(ImageFilter.BoxBlur(r))).astype(np.float32)[..., None] / 255
        fill = acc_b / np.maximum(w_b, 1e-3)
        rgb = np.where(al > 0.01, rgb, np.where(w_b > 0.01, fill, rgb))
    out = np.concatenate([np.clip(rgb, 0, 255), a[..., 3:]], -1).astype(np.uint8)
    Image.fromarray(out, "RGBA").save(path)

def build(name, lo_g, hi_g):
    st, v = stem(name)
    src, kind = SPECIES.get(st, ("ph:tree_small_02", None))
    reset(); sc = bpy.context.scene
    if src.startswith("ph:") and kind == "clump": import_clump(src[3:], v); kind = None
    elif src.startswith("ph:"): import_ph(src[3:], v)
    else: globals()["proc_" + src[5:]](v)
    treat(kind)
    lo, hi = bounds()
    # game bounds: height in y, footprint in x/z
    Hg = hi_g[1] - max(lo_g[1], 0) if hi_g[1] > 0 else hi_g[1] - lo_g[1]
    Wg = max(hi_g[0] - lo_g[0], hi_g[2] - lo_g[2])
    aspect = max(0.25, min(2.0, (hi.x - lo.x) / max(hi.z - lo.z, 1e-3)))
    rw, rh = (512, int(512 / aspect)) if aspect < 1 else (1024, int(1024 / aspect))
    rh = min(1024, max(256, (rh // 64) * 64)); rw = min(1024, max(256, (rw // 64) * 64))
    png = os.path.join(BUILD, "eco", "cards", name + ".png"); os.makedirs(os.path.dirname(png), exist_ok=True)
    render_card(png, rw, rh); bleed(png)
    # cards: three quads at 60 degrees, sized to the stock height, keeping the render's aspect
    reset()
    H = Hg; W = H * rw / rh
    m = image_mat(png, "card", 0.85); nt = m.node_tree; t = [n for n in nt.nodes if n.type == "TEX_IMAGE"][0]
    nt.links.new(t.outputs["Alpha"], nt.nodes["Principled BSDF"].inputs["Alpha"])
    m.surface_render_method = "DITHERED" if hasattr(m, "surface_render_method") else None
    bm = bmesh.new(); uv = bm.loops.layers.uv.verify()
    base = min(lo_g[1], 0) if lo_g[1] < 0 else 0
    for k in range(3):
        a = math.pi * k / 3; dx, dy = math.cos(a) * W / 2, math.sin(a) * W / 2
        vs = [bm.verts.new(p) for p in ((-dx, -dy, base), (dx, dy, base), (dx, dy, base + H), (-dx, -dy, base + H))]
        f = bm.faces.new(vs)
        for l, u in zip(f.loops, ((0, 0), (1, 0), (1, 1), (0, 1))): l[uv].uv = u
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o); me.materials.append(m)
    export(os.path.join(BUILD, "eco", name + ".glb"))

if __name__ == "__main__":
    sizes = {}
    for line in open(os.path.join(FREEPACK, "stock_sizes.tsv")):
        if line.startswith("#") or not line.strip(): continue
        n, *v = line.split("\t"); v = list(map(float, v[:6])); sizes[n] = (v[:3], v[3:])
    eco = [l.strip().lower().split(".")[0] for l in open(os.path.join(FREEPACK, "eco_species.txt")) if l.strip() and not l.startswith("#")]
    names = [a.lower() for a in sys.argv[1:]] or eco
    for n in names:
        build(n, *sizes[n]); print("built", n, flush=True)
