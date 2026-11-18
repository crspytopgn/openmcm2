"""HQ free rider in MX kit (our own design), rigid parts on the original 16-node rider frame tree.
Metres; X right, Y forward, Z up; bind pose standing, arms down (as the original Rider)."""
import math, bpy, bmesh
from mathutils import Vector as V, Matrix, noise
from geo import *
K = 3.355
def G(x, y, z): return V((x / K, z / K, y / K))         # game feet (x, y up, z fwd) -> metres (x, fwd, up)
PIV = {"Pelvis": G(0, 3.177, -0.290), "LwrTorso": G(0, 3.355, -0.437), "UprLegL": G(-0.276, 3.177, -0.290), "UprLegR": G(0.276, 3.177, -0.290),
       "UprTorso": G(0, 3.976, -0.437), "Head": G(0, 5.193, -0.437), "UprArmL": G(-0.650, 4.901, -0.375), "UprArmR": G(0.650, 4.901, -0.375),
       "LwrArmL": G(-0.841, 3.812, -0.385), "HandL": G(-0.992, 2.950, -0.318), "LwrArmR": G(0.841, 3.812, -0.385), "HandR": G(0.992, 2.950, -0.318),
       "LwrLegL": G(-0.275, 1.707, -0.157), "FootL": G(-0.274, 0.330, -0.300), "LwrLegR": G(0.275, 1.707, -0.157), "FootR": G(0.274, 0.330, -0.300)}
ORDER = ["Pelvis", "LwrTorso", "UprLegL", "UprLegR", "UprTorso", "Head", "UprArmL", "UprArmR", "LwrArmL", "HandL", "LwrArmR", "HandR", "LwrLegL", "FootL", "LwrLegR", "FootR"]
PARENT = {"LwrTorso": "Pelvis", "UprLegL": "Pelvis", "UprLegR": "Pelvis", "UprTorso": "LwrTorso", "Head": "UprTorso", "UprArmL": "UprTorso", "UprArmR": "UprTorso",
          "LwrArmL": "UprArmL", "HandL": "LwrArmL", "LwrArmR": "UprArmR", "HandR": "LwrArmR", "LwrLegL": "UprLegL", "FootL": "LwrLegL", "LwrLegR": "UprLegR", "FootR": "LwrLegR"}

def fold(p, amp, freq, seed):
    return noise.noise(V((p.x * freq + seed, p.y * freq, p.z * freq * 0.35))) * amp

def limb(a, b, prof, node, tag, sides=20, n=14, ext0=0.0, ext1=0.0, wrinkle=0.0, seed=0.0, fwd=V((0, 1, 0)), name="limb", ell=1.0):
    """Loft along a->b. prof(t)->(rx, ry) radii (x side, y forward) at t in [0,1]; ends extended and capped round."""
    a, b = V(a), V(b); ax = (b - a); L = ax.length; ax.normalize()
    side = ax.cross(fwd).normalized(); f2 = side.cross(ax).normalized()
    rings = []
    T = [-ext0 / L] + [k / (n - 1) for k in range(n)] + [1 + ext1 / L]
    for i, t in enumerate(T):
        tc = min(1, max(0, t)); rx, ry = prof(tc)
        if i == 0 or i == len(T) - 1:
            rx *= 0.55; ry *= 0.55
        c = a + ax * (t * L); ring = []
        for j in range(sides):
            u = 2 * math.pi * j / sides
            p = c + side * math.cos(u) * rx + f2 * math.sin(u) * ry
            if wrinkle:
                p += (side * math.cos(u) + f2 * math.sin(u)) * fold(p, wrinkle, 38, seed)
            ring.append(p)
        rings.append(ring)
    # close ends with a centre point
    o = loft(rings, closed=True, cap=True, name=name, tag=tag, node=node)
    subsurf(o, 1)
    return o

def ellipsoid(c, r, node, tag, segs=24, rings=16, deform=None, name="ell"):
    verts = []; faces = []
    for i in range(rings + 1):
        th = math.pi * i / rings
        for j in range(segs):
            ph = 2 * math.pi * j / segs
            p = V((math.sin(th) * math.cos(ph) * r[0], math.sin(th) * math.sin(ph) * r[1], math.cos(th) * r[2]))
            if deform: p = deform(p)
            verts.append(V(c) + p)
    for i in range(rings):
        for j in range(segs):
            a = i * segs + j; b = i * segs + (j + 1) % segs; faces.append((a, b, b + segs, a + segs))
    me = mesh_from(verts, faces, name); o = link(name, me, tag, node); weld(o, 1e-5); fix_normals(o); return o

# ------------------------------------------------------------------ parts
def helmet():
    objs = []
    hc = V((0, PIV["Head"].y + 0.015, 1.705))       # helmet centre
    def shell(p):
        q = p.copy()
        # flatten the back-bottom, push the chin forward-down
        if q.z < 0 and q.y > 0: q.y += 0.05 * (-q.z / 0.15) * (q.y / 0.13)
        if q.z < -0.05: q.z = -0.05 + (q.z + 0.05) * 0.7
        q.x *= 1.0 - 0.10 * max(0, q.y / 0.15) * max(0, -q.z / 0.15)
        return q
    sh = ellipsoid(hc, (0.142, 0.165, 0.152), "Head", "helmet", 40, 26, shell, "helmet")
    # carve the eye port: push vertices of the face opening inward (visual only, goggles sit over it)
    for v in sh.data.vertices:
        p = v.co - hc
        if p.y > 0.09 and -0.035 < p.z < 0.045 and abs(p.x) < 0.11:
            v.co.y -= 0.035 * (1 - abs(p.x) / 0.11) ** 0.5
    subsurf(sh, 1); objs.append(sh)
    # chin bar (forward jaw), loft of arcs
    rings = []
    for k in range(9):
        t = k / 8; ang = math.radians(-70 + 140 * t)
        ring = []
        for j in range(10):
            u = j / 9
            r = 0.135 + 0.01 * math.sin(math.pi * u)
            z = -0.11 + 0.075 * u - 0.02 * math.cos(ang) ** 6
            y = 0.03 + math.cos(ang) * r + 0.06 * (1 - abs(ang) / math.radians(70)) ** 2 + 0.02 * (1 - u)
            ring.append(hc + V((math.sin(ang) * r * 0.95, y, z)))
        rings.append(ring)
    cb = loft(rings, closed=False, name="chin", tag="helmet", node="Head"); s = cb.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.02; subsurf(cb, 1); objs.append(cb)
    # mouthpiece vent grille
    objs.append(box(hc + V((0, 0.215, -0.085)), (0.05, 0.02, 0.045), 0.6, 1, "vent", "helmet_trim", "Head"))
    # peak / visor
    rings = []
    for k in range(9):
        t = k / 8; ang = math.radians(-58 + 116 * t)
        ring = []
        for j in range(7):
            u = j / 6; r = 0.150 + 0.11 * u * (1 - 0.35 * abs(math.sin(ang)) ** 2)
            y = math.cos(ang) * r + 0.025; x = math.sin(ang) * r * 0.92
            z = 0.075 - 0.02 * u + 0.04 * u * u - 0.03 * abs(math.sin(ang)) ** 2
            ring.append(hc + V((x, y, z)))
        rings.append(ring)
    pk = loft(rings, closed=False, name="peak", tag="helmet", node="Head"); s = pk.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.006; subsurf(pk, 1); objs.append(pk)
    # goggles: frame + lens + strap
    gc = hc + V((0, 0.128, 0.012))
    rings = []
    for k in range(21):
        ang = math.radians(-62 + 124 * k / 20)
        ring = []
        for j, (dr, dz) in enumerate(((0.0, 0.035), (0.022, 0.034), (0.026, 0.0), (0.022, -0.034), (0.0, -0.035))):
            r = 0.14 + dr
            ring.append(hc + V((math.sin(ang) * r, math.cos(ang) * r + 0.035, 0.012 + dz * (1 - 0.25 * abs(math.sin(ang))) - (0.012 if abs(dz) > 0 and dz < 0 and abs(ang) < 0.2 else 0))))
        rings.append(ring)
    gf = loft(rings, closed=False, name="goggleframe", tag="goggle", node="Head"); subsurf(gf, 1); objs.append(gf)
    rings = []
    for k in range(17):
        ang = math.radians(-55 + 110 * k / 16)
        ring = []
        for j in range(5):
            dz = 0.028 - 0.056 * j / 4
            r = 0.163 + 0.004 * (1 - (dz / 0.028) ** 2)
            ring.append(hc + V((math.sin(ang) * r, math.cos(ang) * r + 0.035, 0.012 + dz)))
        rings.append(ring)
    gl = loft(rings, closed=False, name="lens", tag="lens", node="Head"); subsurf(gl, 1); objs.append(gl)
    # strap around helmet
    rings = []
    for k in range(25):
        ang = math.radians(60 + 240 * k / 24)
        ring = []
        for dz in (0.022, -0.022):
            ring.append(hc + V((math.sin(ang) * 0.149, math.cos(ang) * 0.172 + 0.0, 0.025 + dz)))
        rings.append(ring)
    st = loft(rings, closed=False, name="strap", tag="strap", node="Head"); s = st.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.003; objs.append(st)
    # neck (balaclava) going into the torso
    objs.append(limb(V((0, -0.125, 1.60)), V((0, -0.135, 1.47)), lambda t: (0.062, 0.066), "Head", "kit_dark", 16, 5, name="neck"))
    return objs

def torso():
    objs = []
    # upper torso: jersey chest/back/shoulders, loft along z
    def ring_at(z, wx, dy, cy, sq=0.75, n=28, wr=0.0, seed=0.0):
        ring = []
        for j in range(n):
            u = 2 * math.pi * j / n; c, s = math.cos(u), math.sin(u)
            x = math.copysign(abs(c) ** sq, c) * wx; y = cy + math.copysign(abs(s) ** 0.9, s) * dy
            p = V((x, y, z))
            if wr: p += V((c, s, 0)) * fold(p, wr, 30, seed)
            ring.append(p)
        return ring
    cy = PIV["UprTorso"].y
    up = [(1.08, 0.165, 0.112, 0.0), (1.16, 0.168, 0.115, 0.0), (1.24, 0.172, 0.122, 0.005), (1.32, 0.178, 0.128, 0.008), (1.39, 0.190, 0.130, 0.006),
          (1.44, 0.205, 0.122, 0.0), (1.48, 0.20, 0.11, -0.005), (1.515, 0.15, 0.085, -0.01), (1.53, 0.08, 0.07, -0.01)]
    rings = [ring_at(z, wx, dy, cy + o, 0.7, 28, 0.006, 1.0) for z, wx, dy, o in up]
    o = loft(rings, closed=True, cap=True, name="chest", tag="kit", node="UprTorso"); subsurf(o, 1); objs.append(o)
    # neck brace (shoulder collar)
    rings = []
    for k in range(25):
        ang = 2 * math.pi * k / 24
        ring = []
        for (dr, dz) in ((0.0, 0.0), (0.035, 0.012), (0.075, 0.0), (0.07, -0.02), (0.02, -0.022)):
            r = 0.085 + dr
            ring.append(V((math.sin(ang) * r * 1.05, cy - 0.005 + math.cos(ang) * r * 0.95, 1.50 + dz - 0.015 * math.cos(ang))))
        rings.append(ring)
    nb = loft(rings, closed=True, name="neckbrace", tag="brace", node="UprTorso"); subsurf(nb, 1); objs.append(nb)
    # lower torso (belly + kidney belt), overlaps chest and pelvis
    lo = [(0.93, 0.16, 0.108), (0.99, 0.158, 0.106), (1.05, 0.162, 0.11), (1.12, 0.165, 0.112)]
    rings = [ring_at(z, wx, dy, PIV["LwrTorso"].y, 0.7, 28, 0.005, 2.0) for z, wx, dy in lo]
    o = loft(rings, closed=True, cap=True, name="belly", tag="kit", node="LwrTorso"); subsurf(o, 1); objs.append(o)
    # pelvis / pants waist + seat
    pv = [(0.80, 0.11, 0.09), (0.85, 0.165, 0.11), (0.90, 0.175, 0.118), (0.96, 0.168, 0.112), (1.01, 0.162, 0.108)]
    rings = [ring_at(z, wx, dy, PIV["Pelvis"].y - 0.005, 0.65, 28, 0.004, 3.0) for z, wx, dy in pv]
    o = loft(rings, closed=True, cap=True, name="hips", tag="kit", node="Pelvis"); subsurf(o, 1); objs.append(o)
    return objs

def legs():
    objs = []
    for sx, s in ((-1, "L"), (1, "R")):
        hip = PIV["UprLeg" + s]; knee = PIV["LwrLeg" + s]; ank = PIV["Foot" + s]
        objs.append(limb(hip + V((sx * 0.01, 0, 0)), knee, lambda t: (0.090 - 0.03 * t, 0.098 - 0.03 * t), "UprLeg" + s, "kit", 22, 12, 0.07, 0.04, 0.008, sx * 5, name="thigh"))
        # knee brace / pants over knee and the boot shaft
        objs.append(limb(knee + V((0, 0.0, 0.06)), knee - V((0, 0, 0.12)), lambda t: (0.068, 0.075 + 0.006 * math.sin(math.pi * t)), "LwrLeg" + s, "kit", 22, 6, 0.0, 0.02, 0.006, sx * 7, name="kneepants"))
        objs.append(limb(knee - V((0, 0, 0.07)), ank + V((0, 0.005, 0.0)), lambda t: (0.066 - 0.012 * t, 0.072 - 0.012 * t), "LwrLeg" + s, "boot", 24, 10, 0.0, 0.01, 0.0, name="bootshaft"))
        # knee cup (brace) front
        objs.append(ellipsoid(knee + V((0, 0.06, 0.0)), (0.05, 0.03, 0.065), "LwrLeg" + s, "brace", 16, 10, None, "kneecup"))
        # boot buckles on the outer shin
        for k in range(4):
            z = ank.z + 0.07 + 0.075 * k
            objs.append(box(V((ank.x + sx * 0.062, ank.y + 0.01, z)), (0.012, 0.075, 0.018), 0.5, 1, "buckle", "buckle", "LwrLeg" + s, rot=Matrix.Rotation(math.radians(-sx * 8), 3, 'Y')))
            objs.append(box(V((ank.x, ank.y + 0.005, z + 0.008)), (0.14, 0.15, 0.022), 0.6, 1, "strap", "boot_strap", "LwrLeg" + s))
        # boot foot
        rings = []
        for k in range(12):
            t = k / 11; y = ank.y - 0.11 + 0.32 * t
            ztop = ank.z + 0.08 - 0.07 * t ** 1.4 + (0.04 if t < 0.2 else 0) * (1 - t / 0.2)
            hw = 0.050 + 0.012 * math.sin(math.pi * min(1, t * 1.3)) - 0.02 * max(0, t - 0.85) / 0.15
            zb = 0.0
            ring = []
            for j in range(18):
                u = 2 * math.pi * j / 18; c, sn = math.cos(u), math.sin(u)
                px = math.copysign(abs(c) ** 0.6, c) * hw; pz = (ztop + zb) / 2 + math.copysign(abs(sn) ** 0.5, sn) * (ztop - zb) / 2
                ring.append(V((ank.x + px, y, pz)))
            rings.append(ring)
        o = loft(rings, closed=True, cap=True, name="boot", tag="boot", node="Foot" + s); subsurf(o, 1); objs.append(o)
        objs.append(box(V((ank.x, ank.y + 0.05, 0.012)), (0.12, 0.33, 0.025), 0.5, 1, "sole", "sole", "Foot" + s))
        objs.append(box(V((ank.x, ank.y + 0.17, 0.035)), (0.10, 0.06, 0.05), 0.8, 1, "toecap", "boot_strap", "Foot" + s))
    return objs

def arms():
    objs = []
    for sx, s in ((-1, "L"), (1, "R")):
        sh = PIV["UprArm" + s]; el = PIV["LwrArm" + s]; wr = PIV["Hand" + s]
        objs.append(limb(sh + V((sx * 0.01, 0, 0.0)), el, lambda t: (0.072 - 0.012 * t, 0.07 - 0.01 * t), "UprArm" + s, "kit", 20, 10, 0.05, 0.045, 0.006, sx * 11, name="sleeve"))
        objs.append(limb(el, wr + (wr - el).normalized() * -0.04, lambda t: (0.058 - 0.012 * t, 0.055 - 0.012 * t), "LwrArm" + s, "kit", 20, 10, 0.03, 0.0, 0.005, sx * 13, name="forearm"))
        objs.append(limb(wr + (wr - el).normalized() * -0.07, wr + (wr - el).normalized() * 0.01, lambda t: (0.045, 0.043), "LwrArm" + s, "glove", 18, 5, 0.0, 0.0, 0.0, name="cuff"))
        objs += glove(wr, (wr - el).normalized(), sx, "Hand" + s)
    return objs

def glove(wr, d, sx, node):
    objs = []
    side = V((sx, 0, 0)); fwd = V((0, 1, 0))
    palm_c = wr + d * 0.055
    objs.append(ellipsoid(palm_c, (0.03, 0.045, 0.05), node, "glove", 18, 12, None, "palm"))
    # fingers: curled around a virtual bar in front of the palm
    for k in range(4):
        off = (k - 1.5) * 0.021
        base = wr + d * (0.095 + 0.0) + fwd * 0.02 + d * (off * 0.0) 
        base = palm_c + d * (0.04) + V((0, 0, 0)) + (d.cross(side).normalized()) * 0.0
        # spread along the palm's long axis (d) for knuckles: use side axis for finger row
        row = palm_c + d * 0.045 + fwd * 0.01 + d * 0.0
        p0 = palm_c + d * (0.025 + 0.0) + fwd * 0.03 + d * off * 0 + V((0, 0, 0))
        k0 = palm_c + fwd * 0.035 + d * (0.03 - k * 0.0) + V((0, 0, -0.0)) 
        # knuckle positions spread along d (hand hanging): fingers stack along d
        kn = palm_c + fwd * 0.03 + d * (-0.01 + 0.022 * k - 0.0)
        pts = [kn, kn + fwd * 0.03 + d * 0.005, kn + fwd * 0.03 - side * 0.0 + d * 0.0 - fwd * 0.0 + V((0, 0, 0)) - side * sx * 0.0 + (-side * 0.03), kn - side * 0.03 - fwd * 0.005]
        objs.append(sweep(pts, [0.0105, 0.0102, 0.0098, 0.009], 10, 4, name="finger", tag="glove", node=node))
    th = palm_c + fwd * 0.03 - d * 0.03
    objs.append(sweep([th, th + fwd * 0.025 + d * 0.01, th + fwd * 0.03 + d * 0.035 - side * 0.015], [0.012, 0.011, 0.01], 10, 4, name="thumb", tag="glove", node=node))
    objs.append(box(palm_c + side * 0.028 + fwd * 0.005, (0.012, 0.06, 0.07), 0.7, 1, "knuckle", "glove_trim", node))
    return objs

def build():
    return helmet() + torso() + legs() + arms()
