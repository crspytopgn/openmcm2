"""Free Supercross arena: an open-air stadium bowl around a 600 x 300 ft
floor (the floor itself is the track's ground, so none is built here).
Writes build/objects/fp_arena.glb. Run with Blender's Python (bpy).

The bowl is one stepped profile swept round a rounded rectangle 10 ft
outside the floor: ad wall, walkway, lower bowl, concourse, upper bowl,
canopy. Crowd and ad textures are drawn here (no third-party art).
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from mathutils import Vector
from common import *

TEX = os.path.join(BUILD, "tex"); os.makedirs(TEX, exist_ok=True)
HX, HZ, OFF = 300.0, 150.0, 10.0      # floor half sizes, gap to the wall

def crowd_png(path, seed=3):
    """One tier of seated fans, 20 ft wide: riser half (top) shows heads and
    shoulders, tread half (bottom) laps and seats. Tiles left-right."""
    r = random.Random(seed); W, H = 512, 256
    im = Image.new("RGB", (W, H), (22, 22, 26)); d = ImageDraw.Draw(im)
    shirts = [(130, 130, 125), (20, 20, 22), (30, 40, 70), (100, 24, 20), (60, 64, 70), (150, 135, 110), (35, 55, 35),
              (115, 90, 25), (12, 12, 14), (70, 85, 110), (160, 160, 155), (90, 40, 60)]
    skins = [(200, 150, 120), (165, 115, 85), (120, 80, 55), (75, 50, 35), (185, 140, 105)]
    hair = [(30, 22, 16), (60, 42, 25), (15, 12, 10), (110, 85, 55), (90, 90, 90)]
    seat = (36, 52, 92); n = 12; sw = W / n
    for i in range(n):
        x0 = i * sw
        d.rectangle([x0 + 3, 150, x0 + sw - 3, 250], fill=seat)               # seat pan + back
        if r.random() < 0.12: continue                                          # empty seat
        jx = r.uniform(-4, 4); c = r.choice(shirts); s = r.choice(skins)
        cx = x0 + sw / 2 + jx; top = r.uniform(40, 62)
        d.rounded_rectangle([cx - sw * 0.42, top + 46, cx + sw * 0.42, 200], 10, fill=c)   # torso
        d.rectangle([cx - sw * 0.36, 196, cx + sw * 0.36, 252], fill=tuple(int(v * 0.7) for v in r.choice(shirts[1:4])))  # legs
        d.ellipse([cx - 11, top, cx + 11, top + 30], fill=s)                    # head
        if r.random() < 0.8: d.chord([cx - 12, top - 2, cx + 12, top + 22], 180, 360, fill=r.choice(hair))
        if r.random() < 0.18: d.rectangle([cx - 14, top + 2, cx + 14, top + 9], fill=r.choice(shirts))  # cap
        if r.random() < 0.15:                                                   # arm up
            d.rectangle([cx + 8, top - 30, cx + 16, top + 50], fill=c); d.ellipse([cx + 6, top - 40, cx + 18, top - 28], fill=s)
    im = im.filter(ImageFilter.GaussianBlur(1.2)); im.save(path)

def ads_png(path):
    """Strip of the made-up brands on a dark wall, 8 panels."""
    bdir = os.path.join(BUILD, "brands"); fs = sorted(f for f in os.listdir(bdir) if f.endswith(".png") and "_" not in f)[:8]
    W, H = 2048, 128; im = Image.new("RGB", (W, H), (26, 27, 30)); pw = W // len(fs)
    bgs = [(30, 32, 36), (200, 196, 186), (40, 46, 40), (150, 74, 46), (28, 30, 44), (190, 186, 176), (50, 40, 34), (60, 64, 70)]
    for i, f in enumerate(fs):
        b = Image.open(os.path.join(bdir, f)).convert("RGBA"); b.thumbnail((pw - 40, H - 30))
        panel = Image.new("RGBA", (pw - 6, H - 8), bgs[i] + (255,))
        if sum(bgs[i]) > 400: b = Image.eval(b, lambda v: v)  # keep
        panel.alpha_composite(b, ((panel.width - b.width) // 2, (panel.height - b.height) // 2))
        im.paste(panel.convert("RGB"), (i * pw + 3, 4))
    im.save(path)

def path_pts(n_side=24, n_arc=10):
    """Rounded rectangle at distance OFF outside the floor; returns points,
    outward normals and running length (feet)."""
    P, N = [], []
    corners = [(HX, HZ, 0), (-HX, HZ, 90), (-HX, -HZ, 180), (HX, -HZ, 270)]
    for k, (cx, cz, a0) in enumerate(corners):
        for j in range(n_arc + 1):
            a = math.radians(a0 + 90 * j / n_arc); nrm = Vector((math.cos(a), math.sin(a), 0))
            P.append(Vector((cx, cz, 0)) + nrm * OFF); N.append(nrm)
        nx, nz = corners[(k + 1) % 4][:2]
        a = math.radians(a0 + 90); nrm = Vector((math.cos(a), math.sin(a), 0))
        for j in range(1, n_side):
            t = j / n_side; c = Vector((cx + (nx - cx) * t, cz + (nz - cz) * t, 0))
            P.append(c + nrm * OFF); N.append(nrm)
    S = [0.0]
    for i in range(1, len(P) + 1): S.append(S[-1] + (P[i % len(P)] - P[i - 1]).length)
    return P, N, S

def profile():
    """List of (segment, kind): each segment is two (out, up) points in feet."""
    segs = []
    segs.append((((0, 0), (0, 4.5)), "ads"))          # ad wall facing the floor
    segs.append((((0, 4.5), (1.5, 4.5)), "cap"))
    segs.append((((1.5, 4.5), (8, 4.5)), "walk"))      # walkway behind the wall
    o, h = 8.0, 4.5
    def tiers(n, run, rise):
        nonlocal o, h
        for _ in range(n):
            segs.append((((o, h), (o, h + rise)), "riser")); h += rise
            segs.append((((o, h), (o + run, h)), "tread")); o += run
    segs.append((((o, h), (o, h + 3)), "wall")); h += 3
    tiers(16, 3.0, 1.9)
    segs.append((((o, h), (o + 14, h)), "walk")); o += 14   # concourse
    segs.append((((o, h), (o, h + 4)), "wall")); h += 4
    tiers(17, 2.8, 2.3)
    segs.append((((o, h), (o, h + 6)), "wall")); h += 6
    segs.append((((o, h), (o + 3, h)), "cap")); o += 3
    return segs, o, h

def build(name="fp_arena", hx=300.0, hz=150.0, k=1.0):
    """k scales the stands (bigger stadiums get taller, deeper tiers)."""
    global HX, HZ
    HX, HZ = hx, hz
    reset()
    crowd = os.path.join(TEX, "fp_crowd.png"); ads = os.path.join(TEX, "fp_ads.png"); crowd_png(crowd); ads_png(ads)
    M = {"riser": image_mat(crowd, "crowd", 0.9), "tread": image_mat(crowd, "crowd", 0.9), "ads": image_mat(ads, "ads", 0.6),
         "cap": flat("cap", (0.32, 0.32, 0.33), 0.6), "walk": flat("walk", (0.28, 0.28, 0.27), 0.9),
         "wall": flat("bowlwall", (0.22, 0.23, 0.25), 0.8), "back": flat("bowlback", (0.3, 0.3, 0.31), 0.85),
         "steel": flat("steel", (0.32, 0.33, 0.35), 0.5, 0.6), "lamp": flat("lamp", (0.9, 0.9, 0.86), 0.3), "roof": flat("roof", (0.2, 0.21, 0.22), 0.6, 0.3), "fascia": image_mat(os.path.join(TEX, "fp_ads.png"), "ads", 0.6)}
    P, N, S = path_pts(); segs, OUT, TOP = profile(); n = len(P)
    segs = [(((a[0] * k, a[1] * k), (b[0] * k, b[1] * k)), kind) for (a, b), kind in segs]; OUT *= k; TOP *= k
    bm = bmesh.new(); uvl = bm.loops.layers.uv.new()
    mats = list(dict.fromkeys(M.values())); midx = {m.name: i for i, m in enumerate(mats)}
    def quad(i, a, b, kind):
        j = (i + 1) % n
        vs = [bm.verts.new(P[i] + N[i] * a[0] + Vector((0, 0, a[1]))), bm.verts.new(P[j] + N[j] * a[0] + Vector((0, 0, a[1]))),
              bm.verts.new(P[j] + N[j] * b[0] + Vector((0, 0, b[1]))), bm.verts.new(P[i] + N[i] * b[0] + Vector((0, 0, b[1])))]
        f = bm.faces.new(vs); f.material_index = midx[M[kind].name]
        if kind in ("riser", "tread"):
            u0, u1 = S[i] / (20 * k), S[i + 1] / (20 * k); v0, v1 = ((0.5, 1.0) if kind == "riser" else (0.5, 0.0))
            uvs = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)] if kind == "riser" else [(u0, 0.5), (u1, 0.5), (u1, 0.0), (u0, 0.0)]
        elif kind in ("ads", "fascia"):
            u0, u1 = S[i] / 160, S[i + 1] / 160; uvs = [(u0, 0), (u1, 0), (u1, 1), (u0, 1)]
        else:
            uvs = [(S[i] / 8, a[0] / 8 + a[1] / 8), (S[i + 1] / 8, a[0] / 8 + a[1] / 8), (S[i + 1] / 8, b[0] / 8 + b[1] / 8), (S[i] / 8, b[0] / 8 + b[1] / 8)]
        for l, t in zip(f.loops, uvs): l[uvl].uv = t
    for i in range(n):
        for (a, b), kind in segs: quad(i, a, b, kind)
        quad(i, (OUT, TOP), (OUT, 0), "back")   # outer face down to the ground
    # cantilever roof from the back of the upper bowl, rising toward the floor, with a fascia
    for i in range(n):
        quad(i, (OUT + 2 * k, TOP), (OUT - 58 * k, TOP + 14 * k), "roof")
        quad(i, (OUT - 58 * k, TOP + 9 * k), (OUT - 58 * k, TOP + 14 * k), "fascia")
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.01)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("bowl"); bm.to_mesh(me); bm.free()
    for m in mats: me.materials.append(m)
    # faces should point at the floor/sky: flip any whose normal points away from the centre and down
    o = bpy.data.objects.new("bowl", me); bpy.context.scene.collection.objects.link(o)
    me.polygons.foreach_set("use_smooth", [False] * len(me.polygons))
    # light banks hung under the fascia, and back struts
    for i in range(3, n, 8):
        c = P[i] + N[i] * (OUT - 56 * k) + Vector((0, 0, TOP + 7 * k)); t = Vector((-N[i].y, N[i].x, 0))
        box(c - t * 8 * k - N[i] * 1.2 * k - Vector((0, 0, 2 * k)), c + t * 8 * k + N[i] * 1.2 * k + Vector((0, 0, 2 * k)), M["lamp"])
    for i in range(0, n, 6):
        cyl(P[i] + N[i] * (OUT + 1 * k) + Vector((0, 0, TOP)), P[i] + N[i] * (OUT - 30 * k) + Vector((0, 0, TOP + 7.2 * k)), 0.8 * k, M["steel"], 6)
    o = join_all(name)
    tris = sum(len(p.vertices) - 2 for p in o.data.polygons)
    export(os.path.join(BUILD, "objects", name + ".glb"))
    print(name, "tris", tris, "top", TOP + 14 * k, "outer", OUT + OFF)

# stock stadium shells we stand in for: name -> (half floor x, half floor z, stand scale), sized to the stock bounds
SHELLS = {"b1b_02": (1053.5 - 10 - 1.9 * 125, 1053.5 - 10 - 1.9 * 125, 1.9),
          "euro": (6514 - 10 - 6 * 126, 6514 - 10 - 6 * 126, 6.0), "football": (6514 - 10 - 6 * 126, 6514 - 10 - 6 * 126, 6.0)}

if __name__ == "__main__":
    names = sys.argv[1:] or ["fp_arena"]
    for n in names:
        build() if n == "fp_arena" else build(n, *SHELLS[n])
