"""Free Supercross video boards: a centre-hung four-sided jumbotron and two
end-zone screens, placed for fp_arena (origin at the floor centre).
Every screen face uses the material "screen" so the game can swap what is
shown; the stock picture is a made-up live-timing page.

    python blender/videoboard.py       -> build/objects/fp_videoboard.glb
"""
import os, sys, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy, bmesh
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from mathutils import Vector
from common import reset, BUILD, image_mat, derived_color, box, cyl, join_all, export

TEX = os.path.join(BUILD, "tex")
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
FONTC = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
HX, HZ = 375.0, 187.5

def screen_png(path):
    """Live timing on a dark broadcast background, LED pitch baked in."""
    W, H = 2048, 1024; im = Image.new("RGB", (W, H)); d = ImageDraw.Draw(im)
    for y in range(H):
        t = y / H; d.line([(0, y), (W, y)], fill=(int(10 + 18 * t), int(14 + 10 * t), int(30 + 30 * (1 - t))))
    f = lambda s, c=False: ImageFont.truetype(FONTC if c else FONT, s)
    d.rectangle([0, 0, W, 120], fill=(200, 28, 34)); d.text((40, 18), "OPEN SX  ·  450 MAIN EVENT", font=f(80), fill=(255, 255, 255))
    d.text((W - 520, 22), "LAP 12/20", font=f(76), fill=(255, 230, 120))
    r = random.Random(4)
    names = ["K. HALVERSON", "D. OKAFOR", "J. RENNICK", "M. LAVALLE", "T. BRANDT", "A. SORVINO", "R. TAKEDA", "C. WHITLOCK", "L. MARQUEZ", "P. JANSSEN"]
    cols = [(220, 40, 40), (40, 90, 220), (240, 200, 40), (40, 170, 70), (250, 130, 30), (200, 200, 200), (150, 60, 200), (30, 30, 30), (20, 160, 200), (230, 90, 150)]
    for i, n in enumerate(names):
        y = 150 + i * 86; bg = (24, 28, 44) if i % 2 else (32, 36, 56); d.rectangle([30, y, 1200, y + 78], fill=bg)
        d.rectangle([30, y, 120, y + 78], fill=(255, 255, 255)); d.text((48, y + 6), str(i + 1), font=f(60), fill=(20, 20, 20))
        d.rectangle([132, y + 10, 150, y + 68], fill=cols[i]); d.text((170, y + 8), n, font=f(58, True), fill=(240, 240, 240))
        d.text((640, y + 8), "#%d" % r.randint(2, 99), font=f(58, True), fill=(180, 190, 210))
        d.text((900, y + 8), "LEADER" if i == 0 else "+%.3f" % (i * 1.7 + r.random() * 1.5), font=f(58, True), fill=(255, 220, 90) if i == 0 else (220, 220, 220))
    d.rectangle([1260, 150, W - 30, 990], fill=(18, 20, 30)); d.text((1300, 180), "FASTEST LAP", font=f(56), fill=(255, 220, 90))
    d.text((1300, 260), "48.213", font=f(170), fill=(255, 255, 255)); d.text((1300, 470), "D. OKAFOR  #27", font=f(64, True), fill=(200, 210, 230))
    d.text((1300, 620), "RACE TIME", font=f(56), fill=(255, 220, 90)); d.text((1300, 700), "10:41", font=f(170), fill=(255, 255, 255))
    a = np.asarray(im).astype(np.float32) / 255
    yy, xx = np.mgrid[0:H, 0:W]; hot = (a.min(2) > 0.92)[..., None]   # white text stays full white (glows at night)
    a = np.where(hot, a, a * (0.8 + 0.2 * ((xx % 3 < 2) & (yy % 3 < 2)))[..., None])
    Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).save(path)

def screen_face(bm, uvl, c, right, up, w, h, mi):
    """Quad facing along right x up (counter-clockwise seen from the front)."""
    vs = [bm.verts.new(c + right * sx * w / 2 + up * sy * h / 2) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    f = bm.faces.new(vs); f.material_index = mi
    for l, uv in zip(f.loops, ((0, 0), (1, 0), (1, 1), (0, 1))): l[uvl].uv = uv

def build():
    reset()
    sp = os.path.join(TEX, "fp_videoboard_screen.png"); screen_png(sp)
    scr = image_mat(sp, "screen", 0.3)
    frame = image_mat(derived_color("PaintedMetal004", ramp=((0.05, 0.05, 0.06), (0.16, 0.16, 0.17))), "vb_frame", 0.6)
    steel = image_mat(derived_color("PaintedMetal004", ramp=((0.18, 0.19, 0.2), (0.36, 0.37, 0.38))), "a2_steel", 0.6)
    # centre-hung: long faces 60 x 34 ft toward the sides, end faces 34 x 34 ft, bottom at 95 ft
    Z0, L, Wd, Hh = 95.0, 60.0, 34.0, 34.0
    box(Vector((-L / 2, -Wd / 2, Z0 - 6)), Vector((L / 2, Wd / 2, Z0 + Hh + 4)), frame)        # body with a lip below and a cap above
    box(Vector((-L / 2 - 2, -Wd / 2 - 2, Z0 + Hh + 4)), Vector((L / 2 + 2, Wd / 2 + 2, Z0 + Hh + 6)), frame)
    box(Vector((-L / 2 + 2, -Wd / 2 + 2, Z0 - 10)), Vector((L / 2 - 2, Wd / 2 - 2, Z0 - 6)), frame)   # ring board strip under
    for sx in (-1, 1):
        for sy in (-1, 1):
            cyl((sx * L / 3, sy * Wd / 3, Z0 + Hh + 6), (sx * L / 3, sy * Wd / 3, 175.0), 0.35, steel, 6)   # hoist cables to the roof
    bm = bmesh.new(); uvl = bm.loops.layers.uv.new(); e = 0.25
    X, Y, Zc = Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1))
    screen_face(bm, uvl, Vector((0, -Wd / 2 - e, Z0 + Hh / 2)), X, Zc, L - 3, Hh - 3, 0)
    screen_face(bm, uvl, Vector((0, Wd / 2 + e, Z0 + Hh / 2)), -X, Zc, L - 3, Hh - 3, 0)
    screen_face(bm, uvl, Vector((L / 2 + e, 0, Z0 + Hh / 2)), Y, Zc, Wd - 3, Hh - 3, 0)
    screen_face(bm, uvl, Vector((-L / 2 - e, 0, Z0 + Hh / 2)), -Y, Zc, Wd - 3, Hh - 3, 0)
    # end-zone screens hung in front of the end upper decks, 120 x 58 ft
    for s in (-1, 1):
        x = s * (HX + 95.0)
        box(Vector((x + (0 if s > 0 else -3), -62, 90)), Vector((x + (3 if s > 0 else 0), 62, 152)), frame)
        for yy in (-40, 40): cyl((x + 1.5 * s, yy, 152), (x + 1.5 * s, yy, 172), 0.35, steel, 6)
        screen_face(bm, uvl, Vector((x - 0.3 * s, 0, 121)), Y * (-s), Zc, 120, 58, 0)
    me = bpy.data.meshes.new("screens"); bm.to_mesh(me); bm.free(); me.materials.append(scr)
    ob = bpy.data.objects.new("screens", me); bpy.context.scene.collection.objects.link(ob)
    ob = join_all("fp_videoboard")
    export(os.path.join(BUILD, "objects", "fp_videoboard.glb"))
    print("fp_videoboard tris", sum(len(p.vertices) - 2 for p in ob.data.polygons))

if __name__ == "__main__":
    build()
