"""Free Supercross stadium, version 2: a closed-roof NFL-style bowl around
a 750 x 375 ft floor (the floor is the track's ground, so none is built).
Lower bowl, a ring of lit glass suites under an LED ribbon, an upper
deck, aisles, railings and tunnel mouths, and a closed roof with steel
trusses and a catwalk light ring. Fans are rendered 3D people
(crowd_render.py); brands are the pack's made-up ones.

    python blender/arena2.py            -> build/objects/fp_arena.glb

Game feet; Blender X = game x, Y = game z, Z = up; origin at the floor centre.
"""
import os, sys, math, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont
from mathutils import Vector
from common import reset, BUILD, CACHE, image_mat, derived_color, box, cyl, join_all, export

TEX = os.path.join(BUILD, "tex"); os.makedirs(TEX, exist_ok=True)
HX, HZ = 375.0, 187.5          # floor half sizes, inside the wall
APRON = 14.0                   # concrete apron between the track wall and the stands
CORNER = 80.0                  # bowl corner radius
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
CROWD_W, CROWD_ROWS, RUN, RISE = 40.0, 15, 2.8, 1.6     # as crowd_render.py
UP_RUN, UP_RISE = 2.6, 2.1

# ---------------- pictures ----------------
def grime(a, rnd, amount=0.06):
    h, w = a.shape[:2]
    n = np.asarray(Image.fromarray((np.random.RandomState(rnd).rand(h // 8 + 1, w // 8 + 1) * 255).astype(np.uint8)).resize((w, h), Image.BICUBIC)) / 255.0
    return np.clip(a * (1 - amount + amount * 2 * n[..., None]), 0, 1)

def led_png(path, seed):
    """LED ribbon board, 200 ft per repeat: made-up brands on dark panels."""
    W, H = 4096, 160; im = Image.new("RGB", (W, H), (8, 9, 12)); d = ImageDraw.Draw(im)
    bdir = os.path.join(BUILD, "brands"); brands = sorted(f for f in os.listdir(bdir) if f.endswith(".png") and "_" not in f)
    r = random.Random(seed); x = 0
    while x < W:
        w = r.choice((512, 640, 768)); w = min(w, W - x)
        bg = r.choice([(10, 12, 18), (170, 20, 24), (14, 40, 110), (230, 230, 225), (20, 20, 22), (240, 160, 20)])
        d.rectangle([x, 0, x + w - 1, H], fill=bg)
        b = Image.open(os.path.join(bdir, r.choice(brands))).convert("RGB"); b.thumbnail((w - 60, H - 30))
        im.paste(b, (x + (w - b.width) // 2, (H - b.height) // 2)); x += w
    a = np.asarray(im).astype(np.float32) / 255
    # LED pixel pitch and a soft bloom-free glow: rows of dots
    yy, xx = np.mgrid[0:H, 0:W]; dots = 0.82 + 0.18 * ((xx % 4 < 3) & (yy % 4 < 3))
    a = a * dots[..., None]
    Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).save(path)

def suites_png(path, seed=7):
    """A row of glass suites, 16 ft per box, 4 boxes per picture: warm-lit
    rooms, mullions, people at the glass, some blinds down."""
    W, H = 2048, 448; r = random.Random(seed); im = Image.new("RGB", (W, H), (20, 20, 22)); d = ImageDraw.Draw(im)
    bw = W // 4
    for i in range(4):
        x0 = i * bw
        lit = r.random() < 0.85
        top = (150, 120, 85) if lit else (40, 42, 46); bot = (70, 52, 36) if lit else (22, 23, 26)
        for y in range(20, H - 30):
            t = (y - 20) / (H - 50); c = tuple(int(top[k] * (1 - t) + bot[k] * t) for k in range(3)); d.line([(x0 + 8, y), (x0 + bw - 8, y)], fill=c)
        if lit:   # ceiling lights, back wall TV, people
            for k in range(5): d.ellipse([x0 + 30 + k * 95, 26, x0 + 60 + k * 95, 36], fill=(255, 235, 200))
            d.rectangle([x0 + bw // 2 - 60, 110, x0 + bw // 2 + 60, 180], fill=(50, 90, 120))
            for k in range(r.randint(2, 7)):
                px = x0 + r.randint(30, bw - 40); ph = r.randint(150, 210)
                sh = r.choice([(30, 30, 34), (90, 30, 28), (200, 200, 195), (40, 50, 80), (120, 110, 90)])
                d.rounded_rectangle([px - 22, H - 30 - ph + 50, px + 22, H - 30], 12, fill=sh)
                d.ellipse([px - 14, H - 30 - ph + 10, px + 14, H - 30 - ph + 48], fill=r.choice([(190, 150, 120), (140, 100, 75), (90, 62, 45)]))
        else:
            for y in range(24, H - 34, 10): d.line([(x0 + 10, y), (x0 + bw - 10, y)], fill=(70, 72, 76), width=6)
        # glass reflections: a faint diagonal sheen, then mullions and the sill
        d.rectangle([x0, 0, x0 + 8, H], fill=(60, 62, 66)); d.rectangle([x0 + bw // 2 - 3, 0, x0 + bw // 2 + 3, H], fill=(60, 62, 66))
        d.rectangle([x0, 0, x0 + bw, 18], fill=(60, 62, 66)); d.rectangle([x0, H - 30, x0 + bw, H], fill=(48, 49, 52))
    a = np.asarray(im).astype(np.float32) / 255
    yy, xx = np.mgrid[0:H, 0:W]; sheen = np.clip(1 - np.abs(((xx + yy * 1.3) % 700) - 200) / 120, 0, 1) * 0.06
    a = np.clip(a * 0.92 + sheen[..., None], 0, 1)
    Image.fromarray((grime(a, seed, 0.04) * 255).astype(np.uint8)).save(path)

def roof_png(path):
    """Underside of the roof: acoustic deck panels between purlins, 40 ft per repeat."""
    W = 1024; a = np.full((W, W, 3), 0.16, np.float32); yy, xx = np.mgrid[0:W, 0:W]
    a *= (0.9 + 0.1 * ((xx // 4) % 2))[..., None]                          # deck ribs
    a[(yy % 128) < 6] = 0.1                                                 # purlins
    a[(xx % 256) < 4] = 0.11
    Image.fromarray((grime(a, 3, 0.15) * 255).astype(np.uint8)).save(path)

def lamp_png(path):
    W, H = 256, 64; im = Image.new("RGB", (W, H), (40, 42, 46)); d = ImageDraw.Draw(im)
    for k in range(8): d.ellipse([6 + k * 31, 10, 30 + k * 31, 54], fill=(255, 250, 235)); d.ellipse([10 + k * 31, 14, 26 + k * 31, 50], fill=(255, 255, 250))
    im.save(path)

def tunnel_png(path):
    W, H = 512, 256; a = np.zeros((H, W, 3), np.float32)
    yy, xx = np.mgrid[0:H, 0:W]; depth = np.clip(1 - (np.abs(xx - W / 2) / (W / 2)) * 0.2 - yy / H * 0.1, 0, 1)
    a[:] = (0.05 + 0.08 * (1 - depth))[..., None]
    a[(yy % 64) < 3] *= 0.6
    for k in range(3): a[20:26, 60 + k * 160:120 + k * 160] = 0.95                     # ceiling lights deep inside
    Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).save(path)

def stairs_png(path):
    """Aisle stairs: concrete treads with a yellow nosing, one row per 64 px."""
    src = derived_color("Concrete034", ramp=((0.22, 0.22, 0.22), (0.5, 0.5, 0.49)))
    c = np.asarray(Image.open(src).convert("RGB").resize((256, 1024))).astype(np.float32) / 255
    yy = np.arange(1024)[:, None]
    c[(yy % 64 < 5).repeat(256, 1)] = (0.75, 0.62, 0.12)
    c[((yy % 64 > 5) & (yy % 64 < 12)).repeat(256, 1)] *= 0.55
    Image.fromarray((c * 255).astype(np.uint8)).save(path)

# ---------------- the bowl ----------------
def path_pts():
    """Rounded rectangle on the wall line (floor edge) with corner radius
    CORNER; points, outward normals, running length."""
    P, N = [], []
    ix, iz = HX - CORNER, HZ - CORNER
    corners = [(ix, iz, 0), (-ix, iz, 90), (-ix, -iz, 180), (ix, -iz, 270)]
    for k, (cx, cz, a0) in enumerate(corners):
        for j in range(13):
            a = math.radians(a0 + 90 * j / 12); n = Vector((math.cos(a), math.sin(a), 0))
            P.append(Vector((cx, cz, 0)) + n * CORNER); N.append(n)
        nx, nz = corners[(k + 1) % 4][:2]; a = math.radians(a0 + 90); n = Vector((math.cos(a), math.sin(a), 0))
        L = math.hypot(nx - cx, nz - cz); steps = max(1, round(L / CROWD_W))
        for j in range(1, steps):
            t = j / steps; P.append(Vector((cx + (nx - cx) * t, cz + (nz - cz) * t, 0)) + n * CORNER); N.append(n)
    S = [0.0]
    for i in range(1, len(P) + 1): S.append(S[-1] + (P[i % len(P)] - P[i - 1]).length)
    return P, N, S

def build():
    reset(); rnd = random.Random(11)
    paths = {k: os.path.join(TEX, f"fp_arena2_{k}.png") for k in ("led0", "led1", "suites", "roof", "lamp", "tunnel", "stairs")}
    led_png(paths["led0"], 1); led_png(paths["led1"], 2); suites_png(paths["suites"]); roof_png(paths["roof"])
    lamp_png(paths["lamp"]); tunnel_png(paths["tunnel"]); stairs_png(paths["stairs"])
    crowd = [os.path.join(TEX, f"fp_crowd2_{i}.png") for i in range(4)]
    crowd = [p for p in crowd if os.path.exists(p)] or [os.path.join(TEX, "fp_crowd.png")]
    M = {
        "concrete": image_mat(derived_color("Concrete034", ramp=((0.25, 0.25, 0.24), (0.55, 0.55, 0.53))), "a2_concrete", 0.9),
        "darkcon": image_mat(derived_color("Concrete034", ramp=((0.1, 0.1, 0.1), (0.24, 0.24, 0.24))), "a2_darkcon", 0.9),
        "steel": image_mat(derived_color("PaintedMetal004", ramp=((0.18, 0.19, 0.2), (0.36, 0.37, 0.38))), "a2_steel", 0.6),
        "rail": image_mat(derived_color("PaintedMetal004", ramp=((0.25, 0.25, 0.26), (0.5, 0.5, 0.5))), "a2_rail", 0.5),
        "led0": image_mat(paths["led0"], "a2_led0", 0.4), "led1": image_mat(paths["led1"], "a2_led1", 0.4),
        "suites": image_mat(paths["suites"], "a2_suites", 0.2), "roof": image_mat(paths["roof"], "a2_roof", 0.8),
        "lamp": image_mat(paths["lamp"], "a2_lamp", 0.3), "tunnel": image_mat(paths["tunnel"], "a2_tunnel", 0.9),
        "stairs": image_mat(paths["stairs"], "a2_stairs", 0.9),
    }
    for i, p in enumerate(crowd): M[f"crowd{i}"] = image_mat(p, f"a2_crowd{i}", 0.9)
    P, N, S = path_pts(); n = len(P)
    bm = bmesh.new(); uvl = bm.loops.layers.uv.new()
    mats = list(M.values()); midx = {m.name: i for i, m in enumerate(mats)}
    def quad(i, a, b, kind, uv=None):
        """Strip between profile points a and b (out, up) from path point i to i+1."""
        j = (i + 1) % n
        vs = [bm.verts.new(P[i] + N[i] * a[0] + Vector((0, 0, a[1]))), bm.verts.new(P[j] + N[j] * a[0] + Vector((0, 0, a[1]))),
              bm.verts.new(P[j] + N[j] * b[0] + Vector((0, 0, b[1]))), bm.verts.new(P[i] + N[i] * b[0] + Vector((0, 0, b[1])))]
        f = bm.faces.new(vs); f.material_index = midx[M[kind].name]
        if uv is None:
            span = math.hypot(b[0] - a[0], b[1] - a[1]); u0, u1 = S[i] / 24, S[i + 1] / 24
            uv = [(u0, 0), (u1, 0), (u1, span / 24), (u0, span / 24)]
        for l, t in zip(f.loops, uv): l[uvl].uv = t
        return f
    # profile, out from the wall line: apron, lower front wall (with ribbon), lower bowl, suites, fascia ribbon, upper deck, back
    o, h = APRON, 0.0
    lower_rows, upper_rows = 2 * CROWD_ROWS, 2 * CROWD_ROWS
    lower = ((o, 12.0), (o + lower_rows * RUN, 12.0 + lower_rows * RISE))
    s_base = lower[1]; suite_back = (s_base[0] + 10, s_base[1]); suite_top = s_base[1] + 16
    upper0 = (s_base[0] + 4, suite_top + 9.0)
    upper = (upper0, (upper0[0] + upper_rows * UP_RUN, upper0[1] + upper_rows * UP_RISE))
    back_top = upper[1][1] + 10; roof_h = back_top + 4
    crowd_len_lo = math.hypot(lower_rows * RUN, lower_rows * RISE); crowd_len_up = math.hypot(upper_rows * UP_RUN, upper_rows * UP_RISE)
    for i in range(n):
        u0, u1 = S[i] / 24, S[i + 1] / 24
        quad(i, (0.0, 0.0), (APRON, 0.0), "concrete")                                         # apron floor
        quad(i, (APRON, 0.0), (APRON, 9.0), "darkcon")                                        # front wall
        lu0, lu1 = S[i] / 200, S[i + 1] / 200
        quad(i, (APRON - 0.3, 9.0), (APRON - 0.3, 12.0), "led0", [(lu0, 1), (lu1, 1), (lu1, 0), (lu0, 0)])   # LED ribbon (top of the front wall)
        quad(i, (APRON - 0.3, 12.0), (APRON, 12.0), "concrete")
        c = f"crowd{rnd.randrange(len(crowd))}"
        cu0, cu1 = S[i] / CROWD_W, S[i + 1] / CROWD_W; cv = crowd_len_lo / (CROWD_ROWS * math.hypot(RUN, RISE))
        quad(i, lower[0], lower[1], c, [(cu0, 0), (cu1, 0), (cu1, cv), (cu0, cv)])           # lower bowl fans
        # suites: floor slab lip, recessed glass, ceiling, fascia ribbon
        quad(i, s_base, (s_base[0] + 3, s_base[1]), "concrete")
        quad(i, (s_base[0] + 3, s_base[1]), (s_base[0] + 3, s_base[1] + 1.2), "rail")
        quad(i, (s_base[0] + 3, s_base[1] + 1.2), suite_back, "concrete")
        su0, su1 = S[i] / 64, S[i + 1] / 64
        quad(i, suite_back, (suite_back[0], suite_top), "suites", [(su0, 1), (su1, 1), (su1, 0), (su0, 0)])
        quad(i, (suite_back[0], suite_top), (s_base[0] + 2, suite_top), "darkcon")
        quad(i, (s_base[0] + 2, suite_top), (s_base[0] + 2, suite_top + 6), "led1", [(lu0, 1), (lu1, 1), (lu1, 0), (lu0, 0)])
        quad(i, (s_base[0] + 2, suite_top + 6), (upper0[0], suite_top + 9), "concrete")
        uu0, uu1 = S[i] / CROWD_W, S[i + 1] / CROWD_W; uv_ = crowd_len_up / (CROWD_ROWS * math.hypot(RUN, RISE))
        c = f"crowd{rnd.randrange(len(crowd))}"
        quad(i, upper[0], upper[1], c, [(uu0, 0), (uu1, 0), (uu1, uv_), (uu0, uv_)])          # upper deck fans
        quad(i, upper[1], (upper[1][0], back_top), "darkcon")                                 # back wall
        quad(i, (upper[1][0], back_top), (upper[1][0] + 2, roof_h), "darkcon")
        quad(i, (upper[1][0] + 2, roof_h), (upper[1][0] + 2, 0.0), "darkcon")                # outside face
    # aisles: stair strips up both decks every CROWD_W along the path, and tunnel mouths at the corners' ends
    def strip(i, a, b, width, kind, rows):
        t = Vector((-N[i].y, N[i].x, 0)); c = P[i]
        vs = [c + N[i] * a[0] + Vector((0, 0, a[1] + 0.15)) - t * width / 2, c + N[i] * a[0] + Vector((0, 0, a[1] + 0.15)) + t * width / 2,
              c + N[i] * b[0] + Vector((0, 0, b[1] + 0.15)) + t * width / 2, c + N[i] * b[0] + Vector((0, 0, b[1] + 0.15)) - t * width / 2]
        f = bm.faces.new([bm.verts.new(v) for v in vs]); f.material_index = midx[M[kind].name]
        for l, uvv in zip(f.loops, [(0, 0), (1, 0), (1, rows / 16), (0, rows / 16)]): l[uvl].uv = uvv
    for i in range(0, n, 2):
        strip(i, lower[0], lower[1], 4.0, "stairs", lower_rows)
        strip(i, upper[0], upper[1], 4.0, "stairs", upper_rows)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.01)
    me = bpy.data.meshes.new("bowl"); bm.to_mesh(me); bm.free()
    for m in mats: me.materials.append(m)
    ob = bpy.data.objects.new("bowl", me); bpy.context.scene.collection.objects.link(ob)
    # face the inside: flip faces whose normal points away from the arena centre and the sky
    bm = bmesh.new(); bm.from_mesh(me)
    for f in bm.faces:
        c = f.calc_center_median(); out = Vector((c.x, c.y, 0)).normalized()
        if f.normal.dot(-out) + f.normal.z * 0.5 < -0.05 and f.material_index != midx[M["darkcon"].name]: f.normal_flip()
    bm.to_mesh(me); bm.free()
    # railings along the front of each deck
    for i in range(n):
        j = (i + 1) % n
        for (o_, h_) in ((APRON, 12.0), (upper0[0], upper0[1])):
            a = P[i] + N[i] * (o_ + 0.4) + Vector((0, 0, h_ + 3.3)); b = P[j] + N[j] * (o_ + 0.4) + Vector((0, 0, h_ + 3.3))
            cyl(a, b, 0.12, M["rail"], 6)
    # tunnel mouths: four at the ends of the long sides
    for sx in (-1, 1):
        for sz in (-1, 1):
            x = sx * (HX - CORNER - 40); y = sz * (HZ + APRON)
            box(Vector((x - 12, y - (0 if sz > 0 else 0.6), 0)) if sz > 0 else Vector((x - 12, y - 0.6, 0)), Vector((x + 12, y + 0.6, 11)), M["darkcon"])
            box(Vector((x - 10, y - 0.8 * sz - 0.05, 0)), Vector((x + 10, y - 0.8 * sz + 0.05, 9.5)), M["tunnel"])
    # closed roof: a shallow pitched deck, trusses across the short span, catwalk light ring
    RX, RZ = HX + upper[1][0] + 4, HZ + upper[1][0] + 4
    rise = 26.0
    rb = bmesh.new(); ruv = rb.loops.layers.uv.new()
    grid = [[rb.verts.new(Vector((x, y, roof_h + rise * (1 - (y / RZ) ** 2)))) for x in np.linspace(-RX, RX, 9)] for y in np.linspace(-RZ, RZ, 9)]
    for a in range(8):
        for b in range(8):
            f = rb.faces.new([grid[a][b], grid[a][b + 1], grid[a + 1][b + 1], grid[a + 1][b]])
            for l in f.loops: l[ruv].uv = (l.vert.co.x / 40, l.vert.co.y / 40)
    rme = bpy.data.meshes.new("roof"); rb.to_mesh(rme); rb.free(); rme.materials.append(M["roof"])
    for p in rme.polygons: p.flip()
    bpy.data.objects.new("roof", rme); bpy.context.scene.collection.objects.link(bpy.data.objects["roof"])
    def truss(x):
        """A 14 ft deep box truss across the short span at x."""
        segs = 16
        for k in range(segs):
            y0 = -RZ + 2 * RZ * k / segs; y1 = -RZ + 2 * RZ * (k + 1) / segs
            z0 = roof_h + rise * (1 - (y0 / RZ) ** 2) - 2; z1 = roof_h + rise * (1 - (y1 / RZ) ** 2) - 2
            for dx in (-3, 3):
                cyl((x + dx, y0, z0), (x + dx, y1, z1), 0.5, M["steel"], 6)                   # top chords
                cyl((x + dx, y0, z0 - 14), (x + dx, y1, z1 - 14), 0.5, M["steel"], 6)         # bottom chords
                cyl((x + dx, y0, z0 - 14), (x + dx, y1, z1), 0.25, M["steel"], 4)             # diagonals
                cyl((x + dx, y0, z0 - 14), (x + dx, y0, z0), 0.25, M["steel"], 4)             # verticals
    for x in np.arange(-RX + 50, RX - 20, 60): truss(float(x))
    # catwalk light ring under the trusses, lamps aimed at the floor
    cz = roof_h + rise * 0.55 - 22; cx_, cy_ = HX + 30, HZ + 30
    for (a, b) in (((-cx_, -cy_), (cx_, -cy_)), ((cx_, -cy_), (cx_, cy_)), ((cx_, cy_), (-cx_, cy_)), ((-cx_, cy_), (-cx_, -cy_))):
        A = Vector((a[0], a[1], cz)); B = Vector((b[0], b[1], cz)); d = (B - A); L = d.length; t = d.normalized()
        side = Vector((-t.y, t.x, 0)) * 2.5
        cyl(A + side, B + side, 0.3, M["rail"], 6); cyl(A - side, B - side, 0.3, M["rail"], 6)
        box(Vector((min(A.x, B.x) - 2.5 * abs(t.y), min(A.y, B.y) - 2.5 * abs(t.x), cz - 0.6)),
            Vector((max(A.x, B.x) + 2.5 * abs(t.y), max(A.y, B.y) + 2.5 * abs(t.x), cz - 0.3)), M["steel"])
        for s in np.arange(10, L - 5, 16):
            c = A + t * s; lb = box(c - Vector((4.5, 4.5, 6.0)), c + Vector((4.5, 4.5, -0.6)), M["lamp"])
    ob = join_all("fp_arena")
    tris = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    export(os.path.join(BUILD, "objects", "fp_arena.glb"))
    print("fp_arena v2 tris", tris, "outer", HX + upper[1][0] + 2, HZ + upper[1][0] + 2, "roof", roof_h + rise)

if __name__ == "__main__":
    build()
