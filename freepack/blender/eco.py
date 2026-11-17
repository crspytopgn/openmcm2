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
    # the atlas has no alpha: seed heads sit on black, so key alpha from brightness
    for m in bpy.data.materials:
        if not m.use_nodes: continue
        nt = m.node_tree; P = nt.nodes.get("Principled BSDF"); t = [n for n in nt.nodes if n.type == "TEX_IMAGE" and n.image and "diff" in n.image.name]
        if not P or not t: continue
        bw = nt.nodes.new("ShaderNodeRGBToBW"); mr = nt.nodes.new("ShaderNodeMapRange")
        mr.inputs["From Min"].default_value = 0.015; mr.inputs["From Max"].default_value = 0.06
        nt.links.new(t[0].outputs["Color"], bw.inputs["Color"]); nt.links.new(bw.outputs["Val"], mr.inputs["Value"]); nt.links.new(mr.outputs["Result"], P.inputs["Alpha"])
    for o in tufts: o.location = (rnd.uniform(-0.12, 0.12), rnd.uniform(-0.12, 0.12), 0); o.rotation_euler.z = rnd.uniform(0, 6.3)

def mat_rgb(name, rgb, rough=0.8):
    return flat(name, rgb, rough)

def _cactus_mat():
    """Saguaro skin: dark in the rib valleys, lighter waxy ridges, tan spine
    clusters dotted along each ridge (the "ridge" attribute is set per vertex)."""
    m = bpy.data.materials.new("cactus"); m.use_nodes = True; nt = m.node_tree; P = nt.nodes["Principled BSDF"]
    at = nt.nodes.new("ShaderNodeAttribute"); at.attribute_name = "ridge"
    tc = nt.nodes.new("ShaderNodeTexCoord"); n = nt.nodes.new("ShaderNodeTexNoise"); n.inputs["Scale"].default_value = 60.0; n.inputs["Detail"].default_value = 6
    nt.links.new(tc.outputs["Object"], n.inputs["Vector"])
    skin = nt.nodes.new("ShaderNodeValToRGB"); e = skin.color_ramp.elements
    e[0].position = 0.0; e[0].color = (0.035, 0.06, 0.03, 1); e[1].position = 1.0; e[1].color = (0.2, 0.26, 0.14, 1)
    mixf = nt.nodes.new("ShaderNodeMath"); mixf.operation = "MULTIPLY_ADD"
    nt.links.new(at.outputs["Fac"], mixf.inputs[0]); mixf.inputs[1].default_value = 0.85; nt.links.new(n.outputs["Fac"], mixf.inputs[2])
    nt.links.new(mixf.outputs["Value"], skin.inputs["Fac"])
    # spines: fine dots, only on the ridges
    sp = nt.nodes.new("ShaderNodeTexVoronoi"); sp.inputs["Scale"].default_value = 220.0; nt.links.new(tc.outputs["Object"], sp.inputs["Vector"])
    dot = nt.nodes.new("ShaderNodeMath"); dot.operation = "LESS_THAN"; dot.inputs[1].default_value = 0.12; nt.links.new(sp.outputs["Distance"], dot.inputs[0])
    rg = nt.nodes.new("ShaderNodeMath"); rg.operation = "GREATER_THAN"; rg.inputs[1].default_value = 0.8; nt.links.new(at.outputs["Fac"], rg.inputs[0])
    both = nt.nodes.new("ShaderNodeMath"); both.operation = "MULTIPLY"; nt.links.new(dot.outputs["Value"], both.inputs[0]); nt.links.new(rg.outputs["Value"], both.inputs[1])
    mx = nt.nodes.new("ShaderNodeMix"); mx.data_type = "RGBA"; nt.links.new(both.outputs["Value"], mx.inputs["Factor"])
    nt.links.new(skin.outputs["Color"], mx.inputs[6]); mx.inputs[7].default_value = (0.55, 0.5, 0.36, 1)
    nt.links.new(mx.outputs[2], P.inputs["Base Color"])
    b = nt.nodes.new("ShaderNodeBump"); b.inputs["Strength"].default_value = 0.25; nt.links.new(n.outputs["Fac"], b.inputs["Height"]); nt.links.new(b.outputs["Normal"], P.inputs["Normal"])
    P.inputs["Roughness"].default_value = 0.6; return m

def proc_saguaro(v):
    rnd = random.Random(v); g = _cactus_mat(); RIBS = 12
    def ribbed(pts, r0, r1):
        """A ribbed stem along a polyline, rounded off at the top."""
        bm = bmesh.new(); ridge = bm.verts.layers.float.new("ridge"); rings = []; seg = RIBS * 6
        n = len(pts); ax = None
        for i, c in enumerate(pts):
            c = Vector(c); d = (Vector(pts[min(i + 1, n - 1)]) - Vector(pts[max(i - 1, 0)])).normalized()
            if ax is None: ax = d.cross(Vector((0, 0, 1)) if abs(d.z) < 0.9 else Vector((1, 0, 0))).normalized()
            else: ax = (ax - d * ax.dot(d)).normalized()   # carry the frame along so rings don't twist
            ay = d.cross(ax)
            t = i / (n - 1); r = r0 + (r1 - r0) * t
            if t > 0.85: r *= math.sqrt(max(0.0, 1 - ((t - 0.85) / 0.15) ** 2)) * 0.9 + 0.1
            ring = []
            for j in range(seg):
                a = 2 * math.pi * j / seg; k = abs(math.cos(a * RIBS / 2)) ** 0.6
                vt = bm.verts.new(c + (ax * math.cos(a) + ay * math.sin(a)) * r * (0.86 + 0.14 * k)); vt[ridge] = k; ring.append(vt)
            rings.append(ring)
        for i in range(n - 1):
            for j in range(seg): bm.faces.new((rings[i][j], rings[i][(j + 1) % seg], rings[i + 1][(j + 1) % seg], rings[i + 1][j]))
        bm.faces.new(list(reversed(rings[-1])))
        me = bpy.data.meshes.new("saguaro"); bm.to_mesh(me); bm.free()
        attr = me.attributes.get("ridge")
        if attr is None or attr.domain != "POINT":   # bmesh float layers come through as point attributes
            pass
        for p in me.polygons: p.use_smooth = True
        o = bpy.data.objects.new("saguaro", me); bpy.context.scene.collection.objects.link(o); me.materials.append(g); return o
    H = 1.0; ribbed([(0, 0, H * i / 12) for i in range(13)], 0.09, 0.085)
    for i in range(rnd.randint(1, 3)):
        z = rnd.uniform(0.3, 0.55); out = rnd.uniform(0.12, 0.18); top = z + rnd.uniform(0.38, 0.58); ang = rnd.uniform(0, 2 * math.pi)
        dx, dy = math.cos(ang), math.sin(ang); pts = []
        for k in range(7):   # out from the trunk, a quarter-circle elbow, then straight up
            t = k / 6 * math.pi / 2; pts.append((dx * (0.05 + out * math.sin(t)), dy * (0.05 + out * math.sin(t)), z + out * (1 - math.cos(t))))
        e = z + out
        for k in range(1, 6): pts.append((dx * (0.05 + out), dy * (0.05 + out), e + (top - e) * k / 5 if top > e else e + 0.08 * k))
        ribbed(pts, 0.06, 0.056)

def proc_ocotillo(v):
    rnd = random.Random(v); g = mat_rgb("ocotillo", (0.2, 0.18, 0.12))
    leaf = mat_rgb("ocotillo_leaf", (0.2, 0.32, 0.1)); flower = mat_rgb("ocotillo_flower", (0.75, 0.12, 0.05), 0.6)
    for i in range(22):
        a = rnd.uniform(0, 2 * math.pi); lean = rnd.uniform(0.15, 0.4); L = rnd.uniform(0.7, 1.0)
        tip = Vector((math.cos(a) * lean * L, math.sin(a) * lean * L, L))
        cyl((0, 0, 0), tip, 0.012, g, 6, r2=0.005)
        for k in range(10):   # little leaf clusters up the cane, a red flower spike at the tip
            p = tip * rnd.uniform(0.15, 0.85)
            bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.014, location=p); o = bpy.context.object; o.scale = (1, 1, 0.5); o.data.materials.append(leaf)
        if rnd.random() < 0.7: cyl(tip, tip + (tip.normalized() * 0.035), 0.008, flower, 6, r2=0.002)

def _frond_texture():
    from PIL import Image, ImageDraw
    p = os.path.join(BUILD, "tex", "frond.png")
    if os.path.exists(p): return p
    W, H = 256, 1024; im = Image.new("RGBA", (W, H), (0, 0, 0, 0)); d = ImageDraw.Draw(im)
    d.line([(W / 2, 0), (W / 2, H)], fill=(80, 70, 35, 255), width=6)
    rnd = random.Random(3)
    for y in range(16, H, 9):
        L = (W / 2 - 6) * math.sin(math.pi * y / H) ** 0.5 * rnd.uniform(0.85, 1.0)
        for s in (-1, 1):
            if rnd.random() < 0.12: continue
            col = (int(rnd.uniform(55, 85)), int(rnd.uniform(75, 100)), int(rnd.uniform(35, 50)), 255)
            if rnd.random() < 0.15: col = (110, 95, 55, 255)
            d.polygon([(W / 2, y), (W / 2 + s * L, y + 70), (W / 2 + s * L * 0.97, y + 74), (W / 2, y + 4)], fill=col)
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
    rnd = random.Random(v); g = mat_rgb("tumble", (0.48, 0.40, 0.27))
    cu = bpy.data.curves.new("tw", "CURVE"); cu.dimensions = "3D"; cu.bevel_depth = 0.0035; cu.bevel_resolution = 1
    for i in range(420):
        c = Vector((rnd.gauss(0, 1), rnd.gauss(0, 1), rnd.gauss(0, 1))).normalized() * rnd.uniform(0.15, 0.45)
        d = Vector((rnd.gauss(0, 1), rnd.gauss(0, 1), rnd.gauss(0, 1))).normalized()
        sp = cu.splines.new("POLY"); k = 6; sp.points.add(k - 1); L = rnd.uniform(0.12, 0.3); bend = Vector((rnd.gauss(0, 1), rnd.gauss(0, 1), rnd.gauss(0, 1))) * 0.06
        for j in range(k):
            t = j / (k - 1) - 0.5; p = c + d * L * t + bend * (1 - 4 * t * t)
            if p.length > 0.48: p = p.normalized() * 0.48
            sp.points[j].co = (p.x, p.y, p.z * 0.85 + 0.42, 1)
    o = bpy.data.objects.new("tw", cu); bpy.context.scene.collection.objects.link(o); cu.materials.append(g)
    bpy.context.view_layer.objects.active = o; o.select_set(True); bpy.ops.object.convert(target="MESH")

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
        elif kind == "seedheads":
            dark = (g < 0.16)[:, None]; c = np.where(dark, np.array([0.42, 0.36, 0.22], np.float32) * (0.6 + 2.0 * g[:, None]), c)
        elif kind == "green": c = g[:, None] * np.array([0.6, 1.15, 0.45], np.float32)
        px[:, :3] = np.clip(c, 0, 1); im.pixels[:] = px.ravel()

def render_card(path, res_w, res_h):
    """Orthographic front render of the scene, transparent background, soft
    even sky light so the card carries gentle shading but no hard shadows."""
    sc = bpy.context.scene; lo, hi = bounds()
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam")); sc.collection.objects.link(cam); sc.camera = cam
    cam.data.type = "ORTHO"; wx, hz = hi.x - lo.x, hi.z - lo.z
    size = (max(wx, hz * res_w / res_h) if res_w >= res_h else max(hz, wx * res_h / res_w)) * 1.02
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
