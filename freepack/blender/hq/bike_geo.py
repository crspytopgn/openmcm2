"""HQ free dirt bike geometry (our own design). Metres, X right, Y forward, Z up.
build(stroke) -> list of objects tagged with 'node' (frame) and 'tag' (material class)."""
import math, bpy, bmesh
from mathutils import Vector as V, Matrix
from geo import *

RA = V((0, -0.792, 0.3323)); FA = V((0, 0.688, 0.343)); HP = V((0, 0.283, 1.103)); SP = V((0, -0.218, 0.4826))
KS = V((0.1136, -0.2432, 0.4745))
U = (FA - HP).normalized()                     # steering axis, pointing down
BACK = V((0, -U.z, U.y)) * -1                  # perpendicular in YZ plane
if BACK.y > 0: BACK = -BACK                     # points rearwards
def S(s, off=0.0, x=0.0): return HP + U * s + BACK * off + V((x, 0, 0))

# ---------------------------------------------------------------- wheels
def tyre(c, R, Rbead, width, node, rear=False):
    w = width / 2
    prof = [(Rbead - 0.004, -w * 0.42), (Rbead + 0.006, -w * 0.47), (Rbead + 0.02, -w * 0.62), (R - 0.045, -w * 1.0), (R - 0.02, -w * 0.98),
            (R - 0.006, -w * 0.82), (R, -w * 0.45), (R + 0.002, 0), (R, w * 0.45), (R - 0.006, w * 0.82), (R - 0.02, w * 0.98), (R - 0.045, w * 1.0),
            (Rbead + 0.02, w * 0.62), (Rbead + 0.006, w * 0.47), (Rbead - 0.004, w * 0.42)]
    car = lathe(prof, c, (1, 0, 0), 72, "tyre", "tyre", node)
    # knobs
    rows = 46 if rear else 40
    verts = []; faces = []
    def knob(theta, xo, wid, lng, h, tilt=0.0, ang=0.0):
        # place on surface: radius at xo from profile
        rr = surf_r(abs(xo))
        nrm_tilt = tilt
        # local frame: radial dir r, tangential t, axial a
        r = V((0, math.cos(theta), math.sin(theta))); t = V((0, -math.sin(theta), math.cos(theta))); a = V((1, 0, 0))
        # tilt normal towards axis side for shoulder knobs
        n = (r * math.cos(nrm_tilt) + a * math.sin(nrm_tilt) * (1 if xo > 0 else -1)).normalized()
        aa = (a - n * a.dot(n)).normalized()
        ca, sa = math.cos(ang), math.sin(ang); t2 = t * ca + aa * sa; a2 = aa * ca - t * sa
        base = c + r * rr + a * xo - n * 0.003
        top = base + n * (h + 0.003)
        b0 = len(verts)
        for (sx, sy) in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            verts.append(base + t2 * (lng / 2 * sx) + a2 * (wid / 2 * sy))
        for (sx, sy) in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            verts.append(top + t2 * (lng / 2 * sx * 0.82) + a2 * (wid / 2 * sy * 0.85))
        faces.extend([(b0 + 4, b0 + 5, b0 + 6, b0 + 7), (b0, b0 + 1, b0 + 5, b0 + 4), (b0 + 1, b0 + 2, b0 + 6, b0 + 5), (b0 + 2, b0 + 3, b0 + 7, b0 + 6), (b0 + 3, b0, b0 + 4, b0 + 7)])
    def surf_r(xa):
        # interpolate carcass outer radius at axial |x|
        pts = sorted([(abs(a), r) for r, a in prof if a >= 0])
        for (a0, r0), (a1, r1) in zip(pts, pts[1:]):
            if a0 <= xa <= a1: return r0 + (r1 - r0) * (xa - a0) / max(1e-6, a1 - a0)
        return pts[-1][1]
    for i in range(rows):
        th = 2 * math.pi * i / rows
        if i % 2 == 0:
            knob(th, 0.0, w * 0.55, 0.022, 0.013)
            knob(th, w * 0.80, w * 0.34, 0.022, 0.012, tilt=0.55, ang=0.15)
            knob(th, -w * 0.80, w * 0.34, 0.022, 0.012, tilt=0.55, ang=-0.15)
        else:
            knob(th, w * 0.42, w * 0.36, 0.020, 0.013, tilt=0.15)
            knob(th, -w * 0.42, w * 0.36, 0.020, 0.013, tilt=0.15)
            if rear:
                knob(th + math.pi / rows, w * 0.97, w * 0.22, 0.018, 0.010, tilt=0.95)
                knob(th + math.pi / rows, -w * 0.97, w * 0.22, 0.018, 0.010, tilt=0.95)
    me = mesh_from(verts, faces, "knobs"); k = link("knobs", me, "tyre", node, smooth=False)
    return [car, k]

def rim(c, Rb, width, node):
    w = width / 2
    prof = [(Rb - 0.022, -w * 0.55), (Rb - 0.016, -w * 0.98), (Rb - 0.004, -w * 1.02), (Rb + 0.006, -w * 1.0), (Rb + 0.007, -w * 0.86), (Rb - 0.002, -w * 0.8),
            (Rb - 0.006, -w * 0.45), (Rb - 0.012, -w * 0.25), (Rb - 0.013, 0), (Rb - 0.012, w * 0.25), (Rb - 0.006, w * 0.45), (Rb - 0.002, w * 0.8),
            (Rb + 0.007, w * 0.86), (Rb + 0.006, w * 1.0), (Rb - 0.004, w * 1.02), (Rb - 0.016, w * 0.98), (Rb - 0.022, w * 0.55)]
    return lathe(prof, c, (1, 0, 0), 72, "rim", "anod", node, closed_profile=True)

def hub(c, width, fr, node, rear=False):
    w = width / 2
    prof = [(0.012, -w - 0.012), (0.022, -w - 0.01), (0.024, -w), (fr, -w + 0.002), (fr + 0.002, -w + 0.006), (fr, -w + 0.010), (0.03, -w + 0.016),
            (0.026, -w * 0.4), (0.024, 0), (0.026, w * 0.4), (0.03, w - 0.016), (fr, w - 0.010), (fr + 0.002, w - 0.006), (fr, w - 0.002), (0.024, w), (0.022, w + 0.01), (0.012, w + 0.012)]
    h = lathe(prof, c, (1, 0, 0), 32, "hub", "alu", node)
    ax = cyl(c + V((-w - 0.03, 0, 0)), c + V((w + 0.03, 0, 0)), 0.009, 12, name="axle", tag="steel", node=node)
    return [h, ax]

def spokes(c, Rn, hw, fr, node, n=36, cross=3):
    out = []; verts = []; faces = []
    for i in range(n):
        side = 1 if i % 2 == 0 else -1
        th_h = 2 * math.pi * i / n
        lead = 1 if (i // 2) % 2 == 0 else -1
        th_r = th_h + lead * math.radians(52)
        a = c + V((side * (hw - 0.006), math.cos(th_h) * fr, math.sin(th_h) * fr))
        b = c + V((side * 0.006, math.cos(th_r) * Rn, math.sin(th_r) * Rn))
        d = (b - a); L = d.length; d.normalize()
        ref = V((1, 0, 0)); e1 = d.cross(ref).normalized(); e2 = d.cross(e1)
        base = len(verts); k = 5; r = 0.0019
        for t in (0, 1):
            p = a + d * (L * t)
            for j in range(k):
                u = 2 * math.pi * j / k; verts.append(p + (e1 * math.cos(u) + e2 * math.sin(u)) * r)
        for j in range(k): faces.append((base + j, base + (j + 1) % k, base + k + (j + 1) % k, base + k + j))
        # nipple
        base = len(verts); r2 = 0.0032; p0 = b - d * 0.016; p1 = b + d * 0.002
        for p in (p0, p1):
            for j in range(6):
                u = 2 * math.pi * j / 6; verts.append(p + (e1 * math.cos(u) + e2 * math.sin(u)) * r2)
        for j in range(6): faces.append((base + j, base + (j + 1) % 6, base + 6 + (j + 1) % 6, base + 6 + j))
    me = mesh_from(verts, faces, "spokes"); return [link("spokes", me, "steel", node)]

def wave_disc(c, R, Rin, xoff, node, waves=12, holes=True):
    """wave rotor: ring with wavy outer edge, carrier spokes to hub bolts."""
    segs = waves * 8; verts = []; faces = []; th = 0.0042
    def ring_pts(rfun, x):
        return [c + V((x, math.cos(2 * math.pi * i / segs) * rfun(i), math.sin(2 * math.pi * i / segs) * rfun(i))) for i in range(segs)]
    outer = lambda i: R + 0.006 * math.cos(2 * math.pi * i * waves / segs) - 0.003
    inner = lambda i: Rin + 0.004 * math.cos(2 * math.pi * i * waves / segs + math.pi)
    rings = [ring_pts(inner, xoff - th / 2), ring_pts(outer, xoff - th / 2), ring_pts(outer, xoff + th / 2), ring_pts(inner, xoff + th / 2)]
    o = loft(rings + [rings[0]], closed=True, name="disc", tag="disc", node=node, smooth=False)
    # carrier: star with 5 arms to hub
    arms = []
    for k in range(5):
        a = 2 * math.pi * k / 5
        p0 = c + V((xoff, math.cos(a) * 0.03, math.sin(a) * 0.03)); p1 = c + V((xoff, math.cos(a + 0.25) * (Rin + 0.004), math.sin(a + 0.25) * (Rin + 0.004)))
        arms.append(sweep([p0, p1], 0.007, 6, 1, name="carrier", tag="disc", node=node, shape=rrect(0.006, 0.016, 0.5), smooth_path=False))
    hubring = lathe([(0.026, xoff - 0.003), (0.036, xoff - 0.003), (0.036, xoff + 0.003), (0.026, xoff + 0.003)], c, (1, 0, 0), 24, "carrier", "disc", node, closed_profile=True)
    return [o, hubring] + arms

def sprocket(c, R, teeth, xoff, node):
    segs = teeth * 4; verts = []
    prof_r = []
    for i in range(segs):
        ph = i % 4
        prof_r.append(R + (0.006 if ph in (1, 2) else -0.004))
    rings = []
    for x in (xoff - 0.0025, xoff + 0.0025):
        rings.append([c + V((x, math.cos(2 * math.pi * i / segs) * prof_r[i], math.sin(2 * math.pi * i / segs) * prof_r[i])) for i in range(segs)])
        rings.append([c + V((x, math.cos(2 * math.pi * i / segs) * R * 0.55, math.sin(2 * math.pi * i / segs) * R * 0.55)) for i in range(segs)])
    rings = [rings[1], rings[0], rings[2], rings[3]]
    return [loft(rings + [rings[0]], closed=True, name="sprocket", tag="sprocket", node=node)]

def wheel(front=True):
    node = "TireFront" if front else "TireRear"
    c = FA if front else RA
    if front:
        R, Rb, tw, rw = 0.330, 0.267, 0.088, 0.040
    else:
        R, Rb, tw, rw = 0.319, 0.241, 0.118, 0.054
    objs = tyre(c, R, Rb, tw, node, rear=not front) + [rim(c, Rb, rw, node)]
    hw = 0.052 if front else 0.062; fr = 0.034 if front else 0.042
    objs += hub(c, hw * 2, fr, node, rear=not front) + spokes(c, Rb - 0.022, hw, fr, node)
    if front:
        objs += wave_disc(c, 0.130, 0.098, -0.062, node)
    else:
        objs += wave_disc(c, 0.112, 0.084, 0.070, node)
        objs += sprocket(c, 0.104, 48, -0.078, node)
    return objs

# ---------------------------------------------------------------- forks / bars (HandleBars, Shocks)
FX = 0.105
def forks():
    objs = []
    for sx in (-1, 1):
        x = sx * FX
        # outer tube (top), HandleBars
        objs.append(lathe([(0.0, -0.02), (0.020, -0.02), (0.024, -0.015), (0.026, 0.0), (0.026, 0.42), (0.0305, 0.43), (0.0305, 0.47), (0.0275, 0.49), (0.025, 0.495)],
                          S(0, x=x), U, 32, "forkouter", "anod2", "HandleBars"))
        # stanchion (sliding, Shocks)
        objs.append(lathe([(0.0238, 0.40), (0.0238, 0.82), (0.026, 0.83)], S(0, x=x), U, 32, "stanchion", "gold", "Shocks"))
        # axle lug
        lug_c = S(0.88, x=x)
        objs.append(box(lug_c + U * -0.01 + V((0, 0.008, 0)), (0.05, 0.065, 0.10), 0.5, 1, "lug", "alu_cast", "Shocks",
                        rot=Matrix.Rotation(math.atan2(U.y, -U.z), 3, 'X')))
        objs.append(cyl(S(0.83, x=x), S(0.86, x=x), 0.029, 24, name="lugtop", tag="alu_cast", node="Shocks"))
        # top cap
        objs.append(cyl(S(-0.022, x=x), S(-0.012, x=x), 0.021, 16, name="cap", tag="alu", node="HandleBars"))
        # fork guard (plastic, Shocks): curved shield on front-outer side of stanchion
        rings = []
        for k in range(9):
            s = 0.50 + 0.36 * k / 8
            cc = S(s, x=x)
            ring = []
            for j in range(9):
                a = math.radians(-70 + 220 * j / 8) * (1 if sx > 0 else -1)
                # front direction is -BACK
                fwd = -BACK; side = V((sx, 0, 0))
                ang = math.radians(-40) + math.radians(150) * j / 8
                d = fwd * math.cos(ang) + side * math.sin(ang)
                rr = 0.036 + 0.004 * math.sin(math.pi * k / 8)
                ring.append(cc + d * rr)
            rings.append(ring)
        g = loft(rings, closed=False, name="forkguard", tag="paint", node="Shocks")
        sol = g.modifiers.new("t", "SOLIDIFY"); sol.thickness = 0.0035
        subsurf(g, 1); objs.append(g)
    # front axle through lugs (Shocks)
    # caliper (left, behind lug) Shocks
    th = math.radians(205)
    cp = FA + V((-0.062, math.cos(th) * 0.112, math.sin(th) * 0.112))
    objs.append(box(cp + V((-0.004, 0, 0)), (0.05, 0.085, 0.05), 0.6, 1, "caliper", "caliper", "Shocks", rot=Matrix.Rotation(th + math.pi / 2, 3, 'X')))
    objs.append(box(cp + V((-0.03, 0.02, 0.04)), (0.012, 0.06, 0.03), 0.6, 1, "calbracket", "alu_cast", "Shocks"))
    return objs

def clamps():
    objs = []
    for s, t, name in ((0.06, 0.028, "topclamp"), (0.285, 0.04, "lowclamp")):
        rings = []
        for k in range(2):
            c = S(s + (k - 0.5) * t, off=0.012)
            ring = []
            for j in range(40):
                u = 2 * math.pi * j / 40
                x = math.cos(u); y = math.sin(u)
                # superellipse wide in x, narrow front-back
                px = math.copysign(abs(x) ** 0.55, x) * (FX + 0.04); py = math.copysign(abs(y) ** 0.8, y) * 0.045 - 0.006 * (1 - abs(x))
                ring.append(c + V((px, 0, 0)) + (-BACK) * py)
            rings.append(ring)
        o = loft(rings, closed=True, cap=True, name=name, tag="alu", node="HandleBars")
        b = o.modifiers.new("b", "BEVEL"); b.width = 0.005; b.segments = 2
        objs.append(o)
    # stem nut + steering stem cap
    objs.append(cyl(S(0.04, 0.025), S(0.048, 0.025), 0.018, 6, name="stemnut", tag="alu", node="HandleBars"))
    return objs

BAR_C = V((0, 0.262, 1.118))
def bars():
    objs = []
    # risers on top clamp
    for sx in (-1, 1):
        base = S(0.046, 0.022, x=sx * 0.042)
        objs.append(cyl(base, BAR_C + V((sx * 0.042, 0, -0.012)), 0.017, 16, name="riser", tag="alu", node="HandleBars"))
        objs.append(box(BAR_C + V((sx * 0.042, 0.0, 0.004)), (0.03, 0.04, 0.03), 0.6, 1, "barclamp", "alu", "HandleBars"))
    pts = [V((-0.40, 0.212, 1.164)), V((-0.32, 0.222, 1.160)), V((-0.19, 0.252, 1.136)), V((-0.08, 0.262, 1.120)), V((0, 0.262, 1.118)),
           V((0.08, 0.262, 1.120)), V((0.19, 0.252, 1.136)), V((0.32, 0.222, 1.160)), V((0.40, 0.212, 1.164))]
    rad = [0.0111, 0.0111, 0.0125, 0.0143, 0.0143, 0.0143, 0.0125, 0.0111, 0.0111]
    objs.append(sweep(pts, rad, 16, 6, name="bar", tag="anod2", node="HandleBars"))
    # bar pad
    objs.append(sweep([V((-0.075, 0.262, 1.12)), V((0.075, 0.262, 1.12))], 0.026, 16, 4, name="barpad", tag="pad", node="HandleBars", shape=rrect(0.05, 0.05, 0.9)))
    # grips + flanges
    for sx in (-1, 1):
        a = V((sx * 0.272, 0.2205, 1.1595)); b = V((sx * 0.402, 0.2118, 1.1645))
        d = (b - a).normalized()
        prof = [(0.0, 0.0)] + [(0.0165 + 0.0012 * math.sin(i * 1.3), (b - a).length * i / 14) for i in range(15)] + [(0.0, (b - a).length)]
        objs.append(lathe([(0.0, -0.004), (0.026, -0.004), (0.026, 0.004), (0.018, 0.008)] + prof[1:], a, d, 20, "grip", "grip", "HandleBars"))
    # throttle housing (right) and switch (left)
    objs.append(box(V((0.255, 0.222, 1.158)), (0.028, 0.05, 0.05), 0.8, 1, "throttle", "blackplastic", "HandleBars"))
    objs.append(box(V((-0.235, 0.228, 1.152)), (0.03, 0.035, 0.03), 0.8, 1, "killsw", "blackplastic", "HandleBars"))
    # brake master cylinder (right) + reservoir; clutch perch (left)
    objs.append(box(V((0.195, 0.24, 1.17)), (0.05, 0.035, 0.035), 0.6, 1, "master", "alu_cast", "HandleBars"))
    objs.append(box(V((0.195, 0.235, 1.205)), (0.045, 0.03, 0.03), 0.7, 1, "reservoir", "blackplastic", "HandleBars"))
    objs.append(box(V((-0.205, 0.235, 1.148)), (0.04, 0.035, 0.03), 0.6, 1, "perch", "alu_cast", "HandleBars"))
    for sx in (-1, 1):
        p0 = V((sx * 0.215, 0.268, 1.16)); lever = [p0, V((sx * 0.25, 0.30, 1.155)), V((sx * 0.31, 0.305, 1.15)), V((sx * 0.37, 0.296, 1.146)), V((sx * 0.385, 0.29, 1.146))]
        objs.append(sweep(lever, 0.0065, 8, 4, name="lever", tag="alu", node="HandleBars", shape=rrect(0.009, 0.016, 0.6)))
    # brake line from master down to caliper (follows fork, HandleBars part only)
    objs.append(sweep([V((0.19, 0.25, 1.15)), V((0.12, 0.33, 1.05)), V((0.02, 0.40, 0.92)), V((-0.06, 0.42, 0.86)), V((-0.135, 0.44, 0.78))], 0.004, 8, 6, name="brakeline", tag="hose", node="HandleBars"))
    objs.append(sweep([V((-0.21, 0.24, 1.14)), V((-0.17, 0.22, 1.05)), V((-0.12, 0.20, 0.96)), V((-0.10, 0.15, 0.85))], 0.0035, 8, 6, name="cable", tag="hose", node="HandleBars"))
    return objs

def front_fender():
    rings = []
    # path along the tyre crown: angles from behind fork to front tip
    for k in range(14):
        a = math.radians(118 - 82 * k / 13)       # from 118 deg (rear, near fork) to 36 deg (front tip)
        rr = 0.40 + 0.012 * math.sin(math.pi * k / 13) + (0.03 if k > 9 else 0.0) * (k - 9) / 4
        cc = FA + V((0, math.cos(a) * -1 * -1, 0))
        cen = V((0, FA.y + math.cos(math.radians(180) - a) * rr * -1, FA.z + math.sin(a) * rr))
        cen = V((0, FA.y - math.cos(a) * rr * -1, FA.z + math.sin(a) * rr))
        nrm = (cen - V((0, FA.y, FA.z))).normalized()
        wd = 0.075 - 0.012 * (k / 13)
        ring = []
        for j in range(11):
            u = -1 + 2 * j / 10
            down = -nrm * (0.045 * (abs(u) ** 3))
            ring.append(cen + V((u * wd, 0, 0)) + down + nrm * 0.006 * (1 - u * u))
        rings.append(ring)
    o = loft(rings, closed=False, name="ffender", tag="paint", node="HandleBars")
    s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.0035; subsurf(o, 1)
    return [o]

def front_plate():
    # number plate in front of head stock, on clamps
    rings = []
    for k in range(10):
        s = 0.04 + 0.28 * k / 9
        c = S(s, off=-0.075)
        hw = 0.105 - 0.035 * max(0, (k - 6) / 3) ** 2
        ring = []
        for j in range(11):
            u = -1 + 2 * j / 10
            ring.append(c + V((u * hw, 0, 0)) + BACK * (0.03 * u * u))
        rings.append(ring)
    o = loft(rings, closed=False, name="fplate", tag="paint", node="HandleBars")
    s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.004; subsurf(o, 1)
    return [o]

# ---------------------------------------------------------------- frame + body (Bike)
def frame():
    objs = []
    # head tube
    objs.append(cyl(S(0.085, 0.025), S(0.275, 0.025), 0.029, 24, name="headtube", tag="alu", node="Bike"))
    for sx in (-1, 1):
        spar = [V((sx * 0.03, 0.34, 0.925)), V((sx * 0.085, 0.25, 0.905)), V((sx * 0.122, 0.06, 0.845)), V((sx * 0.135, -0.09, 0.77)), V((sx * 0.128, -0.17, 0.66)), V((sx * 0.124, -0.205, 0.53)), V((sx * 0.122, -0.20, 0.36))]
        objs.append(sweep(spar, [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], 12, 6, name="spar", tag="alu", node="Bike", shape=rrect(0.028, 0.074, 0.35), twist_ref=(1, 0, 0)))
        # pivot plates (wide flat casting)
        objs.append(box(V((sx * 0.124, -0.215, 0.49)), (0.026, 0.11, 0.24), 0.4, 1, "pivotplate", "alu", "Bike"))
        # cradle rails
        cr = [V((0, 0.36, 0.80)), V((sx * 0.02, 0.33, 0.52)), V((sx * 0.055, 0.25, 0.25)), V((sx * 0.06, 0.12, 0.165)), V((sx * 0.07, -0.08, 0.165)), V((sx * 0.11, -0.18, 0.25)), V((sx * 0.122, -0.205, 0.37))]
        objs.append(sweep(cr, 0.016, 10, 6, name="cradle", tag="alu", node="Bike"))
        # subframe
        objs.append(sweep([V((sx * 0.128, -0.10, 0.765)), V((sx * 0.10, -0.40, 0.80)), V((sx * 0.075, -0.72, 0.835))], 0.012, 10, 6, name="subframe", tag="alu", node="Bike"))
        objs.append(sweep([V((sx * 0.125, -0.24, 0.57)), V((sx * 0.10, -0.42, 0.70)), V((sx * 0.078, -0.60, 0.82))], 0.010, 10, 6, name="substay", tag="alu", node="Bike"))
        # footpeg + mount
        pc = V((sx * 0.215, -0.175, 0.385))
        objs.append(box(pc, (0.10, 0.05, 0.018), 0.3, 1, "peg", "steel_dark", "Bike"))
        for k in range(6):
            objs.append(box(pc + V((sx * (-0.04 + 0.016 * k), 0, 0.012)), (0.004, 0.05, 0.008), 0.2, 0, "pegtooth", "steel_dark", "Bike"))
        objs.append(box(V((sx * 0.155, -0.18, 0.39)), (0.04, 0.05, 0.05), 0.5, 1, "pegmount", "alu", "Bike"))
    # top spar bridge behind head
    objs.append(box(V((0, 0.24, 0.93)), (0.17, 0.05, 0.05), 0.6, 1, "bridge", "alu", "Bike"))
    return objs

def engine(stroke2=True):
    objs = []
    # crankcase
    objs.append(box(V((0, -0.025, 0.31)), (0.21, 0.33, 0.20), 0.55, 2, "case", "engine", "Bike"))
    objs.append(lathe([(0.0, 0.0), (0.085, 0.0), (0.09, 0.008), (0.088, 0.02), (0.07, 0.035), (0.0, 0.04)], V((0.10, -0.06, 0.33)), (1, 0, 0), 40, "clutchcover", "engine_cover", "Bike"))
    objs.append(lathe([(0.0, 0.0), (0.066, 0.0), (0.07, 0.008), (0.066, 0.018), (0.05, 0.028), (0.0, 0.03)], V((-0.10, 0.05, 0.30)), (-1, 0, 0), 36, "ignitioncover", "engine_cover", "Bike"))
    # cover bolts
    for k in range(8):
        a = 2 * math.pi * k / 8
        objs.append(cyl(V((0.128, -0.06 + math.cos(a) * 0.078, 0.33 + math.sin(a) * 0.078)), V((0.134, -0.06 + math.cos(a) * 0.078, 0.33 + math.sin(a) * 0.078)), 0.006, 6, name="bolt", tag="steel", node="Bike"))
    tilt = Matrix.Rotation(math.radians(-10), 3, 'X')
    if stroke2:
        objs.append(box(V((0, 0.075, 0.53)), (0.13, 0.13, 0.20), 0.35, 1, "cylinder", "engine", "Bike", rot=tilt))
        objs.append(box(V((0, 0.095, 0.66)), (0.145, 0.145, 0.06), 0.5, 1, "head", "engine_cover", "Bike", rot=tilt))
        for k in range(3):
            objs.append(box(V((0, 0.095, 0.685 + 0.012 * k)), (0.12 - 0.02 * k, 0.12 - 0.02 * k, 0.006), 0.3, 0, "headfin", "engine_cover", "Bike", rot=tilt))
        objs.append(cyl(V((0, 0.10, 0.70)), V((0, 0.105, 0.75)), 0.012, 10, name="plug", tag="rubber", node="Bike"))
        # power valve housing
        objs.append(box(V((0.0, 0.15, 0.53)), (0.09, 0.03, 0.06), 0.6, 1, "pv", "engine_cover", "Bike"))
        # intake: reed + carb + boot
        objs.append(cyl(V((0, -0.02, 0.50)), V((0, -0.12, 0.60)), 0.034, 20, name="reed", tag="engine", node="Bike"))
        objs.append(box(V((0, -0.135, 0.62)), (0.07, 0.06, 0.075), 0.6, 1, "carb", "alu_cast", "Bike"))
        objs.append(cyl(V((0, -0.16, 0.64)), V((0, -0.23, 0.70)), 0.036, 20, name="boot", tag="rubber", node="Bike"))
    else:
        objs.append(box(V((0, 0.06, 0.53)), (0.13, 0.12, 0.22), 0.35, 1, "cylinder", "engine", "Bike", rot=tilt))
        objs.append(box(V((0, 0.08, 0.68)), (0.15, 0.15, 0.10), 0.5, 1, "head", "engine_cover", "Bike", rot=tilt))
        objs.append(box(V((0, 0.085, 0.745)), (0.14, 0.13, 0.04), 0.7, 1, "camcover", "engine", "Bike", rot=tilt))
        objs.append(cyl(V((0, -0.02, 0.62)), V((0, -0.13, 0.68)), 0.03, 20, name="intake", tag="engine", node="Bike"))
        objs.append(box(V((0, -0.15, 0.69)), (0.08, 0.07, 0.07), 0.6, 1, "throttlebody", "alu_cast", "Bike"))
        objs.append(cyl(V((0, -0.18, 0.70)), V((0, -0.25, 0.73)), 0.036, 20, name="boot", tag="rubber", node="Bike"))
    # water pump / hoses
    objs.append(cyl(V((-0.105, 0.11, 0.30)), V((-0.135, 0.11, 0.30)), 0.04, 24, name="pump", tag="engine_cover", node="Bike"))
    objs.append(sweep([V((-0.12, 0.13, 0.33)), V((-0.10, 0.22, 0.45)), V((-0.085, 0.24, 0.55))], 0.013, 10, 6, name="hose", tag="hose", node="Bike"))
    objs.append(sweep([V((0.04, 0.12, 0.66)), V((0.07, 0.20, 0.72)), V((0.085, 0.24, 0.80))], 0.013, 10, 6, name="hose", tag="hose", node="Bike"))
    # front sprocket + cover
    objs += sprocket(V((-0.078, -0.155, 0.40)) - V((-0.078, 0, 0)) + V((0, 0, 0)), 0.033, 13, -0.078, "Bike")
    objs.append(box(V((-0.09, -0.12, 0.40)), (0.02, 0.09, 0.08), 0.7, 1, "sprocketcover", "engine_cover", "Bike"))
    # skid plate
    rings = []
    for k in range(8):
        y = 0.28 - 0.50 * k / 7; z = 0.175 if k > 0 else 0.25
        z = 0.15 + (0.08 if k == 0 else 0.0) + (0.04 if k == 7 else 0.0)
        ring = [V((-0.09, y, z + 0.05)), V((-0.085, y, z + 0.01)), V((-0.06, y, z - 0.004)), V((0.06, y, z - 0.004)), V((0.085, y, z + 0.01)), V((0.09, y, z + 0.05))]
        rings.append(ring)
    o = loft(rings, closed=False, name="skid", tag="blackplastic", node="Bike"); s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.005; subsurf(o, 1); objs.append(o)
    # pedals
    objs.append(sweep([V((0.17, -0.19, 0.40)), V((0.18, -0.08, 0.37)), V((0.185, 0.02, 0.36)), V((0.20, 0.06, 0.37))], 0.009, 8, 6, name="brakepedal", tag="steel_dark", node="Bike", shape=rrect(0.012, 0.022, 0.5)))
    objs.append(sweep([V((-0.13, -0.10, 0.40)), V((-0.155, -0.02, 0.43)), V((-0.17, 0.06, 0.445)), V((-0.205, 0.075, 0.445))], 0.008, 8, 6, name="shifter", tag="steel_dark", node="Bike", shape=rrect(0.012, 0.02, 0.5)))
    return objs

def exhaust(stroke2=True):
    objs = []
    if stroke2:
        pts = [V((0.0, 0.16, 0.55)), V((0.02, 0.245, 0.50)), V((0.06, 0.29, 0.39)), V((0.11, 0.24, 0.27)), V((0.15, 0.10, 0.25)), V((0.17, -0.08, 0.32)), V((0.165, -0.24, 0.47)), V((0.15, -0.36, 0.60))]
        rad = [0.022, 0.030, 0.048, 0.058, 0.058, 0.046, 0.032, 0.020]
        objs.append(sweep(pts, rad, 24, 8, name="pipe", tag="pipe", node="Bike"))
        objs.append(lathe([(0.0, -0.01), (0.018, -0.01), (0.028, 0.04), (0.045, 0.06), (0.046, 0.30), (0.04, 0.33), (0.016, 0.34), (0.016, 0.36), (0.0, 0.36)],
                          V((0.15, -0.36, 0.60)), V((0, -0.92, 0.39)), 28, "silencer", "silencer", "Bike"))
    else:
        pts = [V((0.0, 0.14, 0.68)), V((0.04, 0.24, 0.62)), V((0.10, 0.28, 0.50)), V((0.15, 0.20, 0.38)), V((0.17, 0.02, 0.40)), V((0.165, -0.16, 0.52)), V((0.155, -0.30, 0.64))]
        objs.append(sweep(pts, [0.019, 0.02, 0.021, 0.022, 0.024, 0.026, 0.028], 20, 8, name="header", tag="pipe4", node="Bike"))
        objs.append(lathe([(0.0, -0.01), (0.03, -0.01), (0.05, 0.05), (0.058, 0.08), (0.058, 0.36), (0.05, 0.39), (0.018, 0.40), (0.018, 0.42), (0.0, 0.42)],
                          V((0.155, -0.30, 0.64)), V((0, -0.92, 0.36)), 28, "silencer", "silencer", "Bike"))
    # silencer straps
    return objs

def radiators():
    objs = []
    for sx in (-1, 1):
        rot = Matrix.Rotation(math.radians(sx * -28), 3, 'Z') @ Matrix.Rotation(math.radians(-14), 3, 'X')
        c = V((sx * 0.078, 0.25, 0.71))
        objs.append(box(c, (0.105, 0.03, 0.26), 0.15, 1, "radcore", "rad", "Bike", rot=rot))
        objs.append(box(c + rot @ V((0, 0, 0.142)), (0.115, 0.036, 0.03), 0.4, 1, "radtank", "alu", "Bike", rot=rot))
        objs.append(box(c + rot @ V((0, 0, -0.142)), (0.115, 0.036, 0.03), 0.4, 1, "radtank", "alu", "Bike", rot=rot))
    return objs

def coons(c0, c1, d0, d1, nu, nv):
    """c0(u),c1(u): bottom/top curves; d0(v),d1(v): left/right curves; returns grid of points nu x nv"""
    g = []
    for i in range(nu):
        u = i / (nu - 1); row = []
        for j in range(nv):
            v = j / (nv - 1)
            p = (c0(u) * (1 - v) + c1(u) * v) + (d0(v) * (1 - u) + d1(v) * u) - (c0(0) * (1 - u) * (1 - v) + c0(1) * u * (1 - v) + c1(0) * (1 - u) * v + c1(1) * u * v)
            row.append(p)
        g.append(row)
    return g

def curve(pts):
    P = catmull(pts, 16)
    L = [0.0]
    for a, b in zip(P, P[1:]): L.append(L[-1] + (a - b).length)
    def f(t):
        d = t * L[-1]
        for k in range(1, len(L)):
            if L[k] >= d:
                w = (d - L[k - 1]) / max(1e-9, L[k] - L[k - 1]); return P[k - 1].lerp(P[k], w)
        return P[-1].copy()
    return f

def patch(sx, yz_bottom, yz_top, yz_rear, yz_front, xfun, nu=16, nv=12, name="panel", tag="paint", thick=0.004, node="Bike"):
    """side panel patch from 2D (y,z) boundary curves; xfun(u,v) gives |x|."""
    c0 = curve([V((0, y, z)) for y, z in yz_bottom]); c1 = curve([V((0, y, z)) for y, z in yz_top])
    d0 = curve([V((0, y, z)) for y, z in yz_rear]); d1 = curve([V((0, y, z)) for y, z in yz_front])
    g = coons(c0, c1, d0, d1, nu, nv)
    for i in range(nu):
        for j in range(nv):
            g[i][j].x = sx * xfun(i / (nu - 1), j / (nv - 1))
    o = loft(g, closed=False, name=name, tag=tag, node=node)
    if sx < 0: o.data.flip_normals()
    s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = thick; s.offset = -1
    subsurf(o, 1)
    return o

def bodywork():
    objs = []
    for sx in (-1, 1):
        # radiator shroud
        objs.append(patch(sx,
            [(0.00, 0.60), (0.12, 0.52), (0.26, 0.54), (0.34, 0.62)],          # bottom rear->front
            [(-0.02, 0.965), (0.10, 0.985), (0.22, 0.99), (0.33, 0.975)],      # top
            [(0.00, 0.60), (-0.03, 0.78), (-0.02, 0.965)],                    # rear
            [(0.34, 0.62), (0.36, 0.80), (0.33, 0.975)],                      # front
            lambda u, v: 0.118 + 0.05 * math.sin(math.pi * min(1, u * 1.1)) * (1 - v) ** 0.6 + 0.02 * (1 - v) - 0.055 * v ** 3,
            name="shroud"))
        # side number panel
        objs.append(patch(sx,
            [(-0.70, 0.80), (-0.52, 0.66), (-0.36, 0.60), (-0.24, 0.60)],
            [(-0.74, 0.885), (-0.52, 0.895), (-0.34, 0.905), (-0.22, 0.91)],
            [(-0.70, 0.80), (-0.73, 0.85), (-0.74, 0.885)],
            [(-0.24, 0.60), (-0.21, 0.76), (-0.22, 0.91)],
            lambda u, v: 0.105 + 0.03 * u + 0.01 * math.sin(math.pi * v) - 0.012 * v ** 4,
            name="sidepanel"))
    # rear fender: loft along y
    rings = []
    for k in range(16):
        t = k / 15; y = -0.16 - 0.85 * t
        z = 0.89 + 0.02 * t + 0.08 * max(0, t - 0.7) ** 2 / 0.09 * 0.5
        hw = 0.12 - 0.05 * t ** 1.5
        drop = 0.05 * (1 - t) + 0.015
        ring = []
        for j in range(13):
            u = -1 + 2 * j / 12
            ring.append(V((u * hw, y, z - drop * abs(u) ** 4 + 0.008 * (1 - u * u))))
        rings.append(ring)
    o = loft(rings, closed=False, name="rfender", tag="paint", node="Bike"); s = o.modifiers.new("t", "SOLIDIFY"); s.thickness = 0.0035; subsurf(o, 1); objs.append(o)
    # fuel tank (mostly hidden)
    rings = []
    for k in range(10):
        t = k / 9; y = 0.30 - 0.36 * t; ztop = 0.985 - 0.01 * t - 0.04 * max(0, (t - 0.6) / 0.4) ** 2; zb = 0.76 + 0.06 * t
        hw = 0.10 + 0.03 * math.sin(math.pi * t)
        ring = []
        for j in range(16):
            u = 2 * math.pi * j / 16; cx = math.cos(u); cz = math.sin(u)
            ring.append(V((cx * hw, y, (ztop + zb) / 2 + cz * (ztop - zb) / 2)))
        rings.append(ring)
    t = loft(rings, closed=True, cap=True, name="tank", tag="tank", node="Bike"); subsurf(t, 1); objs.append(t)
    objs.append(cyl(V((0, 0.17, 0.975)), V((0, 0.17, 0.995)), 0.024, 20, name="fuelcap", tag="blackplastic", node="Bike"))
    # airbox (under side panels)
    objs.append(box(V((0, -0.38, 0.72)), (0.20, 0.26, 0.16), 0.6, 1, "airbox", "blackplastic", "Bike"))
    # seat: loft along y
    rings = []
    for k in range(18):
        t = k / 17; y = 0.14 - 0.94 * t
        ztop = 0.955 + 0.06 * max(0, (0.18 - t) / 0.18) ** 1.2 - 0.004 * t
        hw = 0.07 + 0.065 * min(1, t / 0.32) ** 0.7 - 0.03 * max(0, (t - 0.85) / 0.15) ** 2
        zb = ztop - 0.075
        ring = []
        for j in range(20):
            u = 2 * math.pi * j / 20; cx = math.cos(u); cz = math.sin(u)
            px = math.copysign(abs(cx) ** 0.45, cx) * hw; pz = (ztop + zb) / 2 + math.copysign(abs(cz) ** 0.6, cz) * (ztop - zb) / 2
            ring.append(V((px, y, pz)))
        rings.append(ring)
    s = loft(rings, closed=True, cap=True, name="seat", tag="seat", node="Bike"); subsurf(s, 1); objs.append(s)
    return objs

def rear_suspension():
    objs = []
    top = V((0, -0.19, 0.84)); bot = V((0, -0.255, 0.37))
    d = (bot - top)
    objs.append(cyl(top, top + d * 0.55, 0.024, 20, name="shockbody", tag="alu_anod", node="Bike"))
    objs.append(cyl(top + d * 0.5, bot, 0.009, 12, name="shockrod", tag="chrome", node="Bike"))
    objs.append(cyl(bot - d.normalized() * 0.03, bot + d.normalized() * 0.01, 0.018, 16, name="clevis", tag="alu", node="Bike"))
    objs.append(cyl(top + V((0.0, 0.0, 0.0)) + d.normalized() * 0.02 + V((0.045, 0, 0)), top + d * 0.35 + V((0.045, 0, 0)), 0.021, 16, name="reservoir", tag="alu_anod", node="Bike"))
    # spring helix
    n = 9; pts = []; a0 = top + d * 0.22; a1 = bot - d.normalized() * 0.04
    ax = (a1 - a0); e1 = V((1, 0, 0)); e2 = ax.normalized().cross(e1)
    for k in range(n * 12 + 1):
        t = k / (n * 12); u = 2 * math.pi * n * t
        pts.append(a0 + ax * t + (e1 * math.cos(u) + e2 * math.sin(u)) * 0.036)
    objs.append(sweep(pts, 0.0062, 8, 1, name="spring", tag="spring", node="Bike", smooth_path=False))
    objs.append(cyl(top + d * 0.2, top + d * 0.225, 0.045, 24, name="collar", tag="alu_anod", node="Bike"))
    return objs

def swingarm():
    objs = []
    for sx in (-1, 1):
        arm = [V((sx * 0.112, SP.y + 0.02, SP.z)), V((sx * 0.112, SP.y - 0.12, SP.z - 0.02)), V((sx * 0.105, -0.55, 0.385)), V((sx * 0.098, RA.y + 0.02, RA.z + 0.002)), V((sx * 0.098, RA.y - 0.04, RA.z))]
        objs.append(sweep(arm, [1.0, 1.0, 0.85, 0.72, 0.70], 12, 6, name="arm", tag="alu", node="SwingArm", shape=rrect(0.034, 0.085, 0.45), twist_ref=(1, 0, 0)))
        objs.append(box(V((sx * 0.098, RA.y - 0.005, RA.z)), (0.042, 0.07, 0.045), 0.4, 1, "axleblock", "alu_cast", "SwingArm"))
    objs.append(cyl(SP + V((-0.135, 0, 0)), SP + V((0.135, 0, 0)), 0.028, 24, name="pivot", tag="alu", node="SwingArm"))
    objs.append(sweep([V((-0.108, -0.32, 0.47)), V((0, -0.33, 0.52)), V((0.108, -0.32, 0.47))], 0.022, 12, 6, name="brace", tag="alu", node="SwingArm"))
    # chain slider + guide
    objs.append(sweep([V((-0.112, -0.20, 0.52)), V((-0.11, -0.36, 0.49)), V((-0.106, -0.48, 0.45))], 0.016, 10, 6, name="slider", tag="blackplastic", node="SwingArm", shape=rrect(0.03, 0.03, 0.4)))
    objs.append(box(V((-0.09, -0.62, 0.30)), (0.03, 0.12, 0.06), 0.5, 1, "chainguide", "blackplastic", "SwingArm"))
    # rear caliper + bracket (right side below arm)
    th = math.radians(210); cp = RA + V((0.07, math.cos(th) * -0.095, math.sin(th) * 0.095))
    objs.append(box(cp, (0.045, 0.075, 0.045), 0.6, 1, "rcaliper", "caliper", "SwingArm"))
    # linkage
    objs.append(box(V((0, -0.27, 0.30)), (0.06, 0.16, 0.035), 0.6, 1, "linkage", "alu_cast", "SwingArm"))
    # chain (simple links along the loop)
    fs = V((-0.078, -0.155, 0.40)); rs = V((-0.078, RA.y, RA.z))
    objs.append(chain(fs, 0.034, rs, 0.104))
    return objs

def chain(c1, r1, c2, r2):
    # loop: tangent lines between two circles in the YZ plane
    import math
    d = c2 - c1; L = math.hypot(d.y, d.z); ang = math.atan2(d.z, d.y)
    beta = math.asin((r1 - r2) / L)
    pts = []
    def circ(c, r, a0, a1, n):
        return [V((c.x, c.y + r * math.cos(a0 + (a1 - a0) * k / n), c.z + r * math.sin(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]
    up = ang + math.pi / 2 + beta; dn = ang - math.pi / 2 - beta
    pts += circ(c1, r1, dn - 2 * math.pi, up - 2 * math.pi + 0 if False else dn, 1)[:0]
    loop = circ(c1, r1, up, dn + 2 * math.pi, 12) + circ(c2, r2, dn + 2 * math.pi, up + 2 * math.pi + 0.0, 1)[:0]
    # build as polyline: around front sprocket (up -> dn going the long way via front), bottom run, around rear (dn -> up via back), top run
    a = circ(c1, r1, up, dn + 2 * math.pi if dn < up else dn, 10)
    b = circ(c2, r2, dn, up, 24) if False else circ(c2, r2, dn + 2 * math.pi if dn + 2 * math.pi < up + 2 * math.pi else dn, up + 2 * math.pi if up < dn else up, 24)
    poly = a + b
    # resample at link pitch
    pitch = 0.0159; P = []
    tot = 0; seg = list(zip(poly, poly[1:] + poly[:1]))
    acc = 0.0; P.append(poly[0])
    for p, q in seg:
        l = (q - p).length; t = pitch - acc
        while t <= l:
            P.append(p.lerp(q, t / l)); t += pitch
        acc = l - (t - pitch)
    verts = []; faces = []
    for i in range(len(P)):
        p = P[i]; q = P[(i + 1) % len(P)]; dd = (q - p)
        if dd.length < 1e-6: continue
        dn_ = dd.normalized(); nn = V((1, 0, 0)).cross(dn_)
        for side in (-1, 1):
            b0 = len(verts); x = side * 0.0055
            for (s, h) in ((0, -1), (1, -1), (1, 1), (0, 1)):
                verts.append(p + dn_ * (s * dd.length) + nn * (h * 0.0045) + V((x, 0, 0)))
            for (s, h) in ((0, -1), (1, -1), (1, 1), (0, 1)):
                verts.append(p + dn_ * (s * dd.length) + nn * (h * 0.0045) + V((x + side * 0.0015, 0, 0)))
            faces += [(b0, b0 + 1, b0 + 2, b0 + 3), (b0 + 4, b0 + 7, b0 + 6, b0 + 5), (b0, b0 + 4, b0 + 5, b0 + 1), (b0 + 1, b0 + 5, b0 + 6, b0 + 2), (b0 + 2, b0 + 6, b0 + 7, b0 + 3), (b0 + 3, b0 + 7, b0 + 4, b0)]
    me = mesh_from(verts, faces, "chain"); o = link("chain", me, "chain", "SwingArm", smooth=False)
    return o

def kickstart():
    p = KS
    o = sweep([p + V((0.012, 0, 0)), p + V((0.02, -0.06, 0.05)), p + V((0.022, -0.13, 0.10)), p + V((0.022, -0.15, 0.11))], 0.009, 8, 6, name="kick", tag="steel_dark", node="KickStart", shape=rrect(0.012, 0.02, 0.5))
    f = cyl(p + V((0.022, -0.15, 0.11)), p + V((0.075, -0.15, 0.11)), 0.009, 10, name="kickfoot", tag="steel_dark", node="KickStart")
    return [o, f]

def build(stroke2=True):
    objs = []
    objs += wheel(True) + wheel(False) + forks() + clamps() + bars() + front_fender() + front_plate()
    objs += frame() + engine(stroke2) + exhaust(stroke2) + radiators() + bodywork() + rear_suspension() + swingarm()
    if stroke2: objs += kickstart()
    return objs
