"""Mesh helpers for the HQ bike and rider (metres, X right, Y forward, Z up)."""
import bpy, bmesh, math
from mathutils import Vector, Matrix
V = Vector
OBJ = []

def link(name, me, tag, node, smooth=True):
    o = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(o)
    o["node"] = node; o["tag"] = tag
    if smooth:
        for p in me.polygons: p.use_smooth = True
    OBJ.append(o); return o

def mesh_from(verts, faces, name="m"):
    me = bpy.data.meshes.new(name); me.from_pydata([tuple(v) for v in verts], [], faces); me.update(); return me

def grid_faces(nu, nv, close_u=False, close_v=False):
    f = []
    cu = nu if close_u else nu - 1; cv = nv if close_v else nv - 1
    for i in range(cu):
        for j in range(cv):
            a = i * nv + j; b = ((i + 1) % nu) * nv + j; c = ((i + 1) % nu) * nv + (j + 1) % nv; d = i * nv + (j + 1) % nv
            f.append((a, b, c, d))
    return f

def loft(sections, closed=True, cap=False, name="loft", tag="paint", node="Bike", sub=0, smooth=True):
    """sections: list of rings (lists of points, equal length). closed: ring is closed."""
    nu = len(sections); nv = len(sections[0])
    verts = [p for s in sections for p in s]
    faces = grid_faces(nu, nv, False, closed)
    if cap and closed:
        faces.append(tuple(range(nv))[::-1]); faces.append(tuple((nu - 1) * nv + j for j in range(nv)))
    me = mesh_from(verts, faces, name); o = link(name, me, tag, node, smooth)
    fix_normals(o)
    if sub: subsurf(o, sub)
    return o

def fix_normals(o):
    bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.recalc_face_normals(bm, faces=bm.faces); bm.to_mesh(o.data); bm.free()

def subsurf(o, lv=1):
    m = o.modifiers.new("sub", "SUBSURF"); m.levels = m.render_levels = lv; return m

def lathe(profile, axis_pt, axis_dir, segs=48, name="lathe", tag="alu", node="Bike", closed_profile=False, sub=0):
    """profile: list of (r, a) radius and axial offset. Revolved around axis."""
    ax = V(axis_dir).normalized(); ref = V((0, 0, 1)) if abs(ax.z) < 0.9 else V((0, 1, 0))
    e1 = ax.cross(ref).normalized(); e2 = ax.cross(e1)
    verts = []
    for i in range(segs):
        t = 2 * math.pi * i / segs; c, s = math.cos(t), math.sin(t)
        for r, a in profile:
            verts.append(V(axis_pt) + ax * a + (e1 * c + e2 * s) * r)
    faces = grid_faces(segs, len(profile), True, closed_profile)
    me = mesh_from(verts, faces, name); o = link(name, me, tag, node); fix_normals(o)
    if sub: subsurf(o, sub)
    return o

def frame_along(pts):
    """parallel-transport frames along a polyline: list of (p, t, n, b)."""
    out = []; n_prev = None
    for i, p in enumerate(pts):
        if i == 0: t = (pts[1] - pts[0])
        elif i == len(pts) - 1: t = (pts[-1] - pts[-2])
        else: t = (pts[i + 1] - pts[i - 1])
        t = t.normalized()
        if n_prev is None:
            ref = V((1, 0, 0)) if abs(t.x) < 0.9 else V((0, 0, 1)); n = (ref - t * ref.dot(t)).normalized()
        else:
            n = (n_prev - t * n_prev.dot(t)).normalized()
        b = t.cross(n); out.append((p, t, n, b)); n_prev = n
    return out

def catmull(pts, per=8):
    pts = [V(p) for p in pts]
    if len(pts) < 3: 
        return [pts[0].lerp(pts[1], k / per) for k in range(per + 1)]
    out = []; P = [pts[0] * 2 - pts[1]] + pts + [pts[-1] * 2 - pts[-2]]
    for i in range(1, len(P) - 2):
        p0, p1, p2, p3 = P[i - 1], P[i], P[i + 1], P[i + 2]
        for k in range(per):
            t = k / per; t2 = t * t; t3 = t2 * t
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 + (-p0 + 3 * p1 - 3 * p2 + p3) * t3))
    out.append(P[-2]); return out

def sweep(pts, radius, sides=12, per=6, cap=True, name="tube", tag="steel", node="Bike", shape=None, smooth_path=True, twist_ref=None):
    """Sweep a circle (or shape(u)->(x,y) in the normal plane) along a path. radius: float or list per control point."""
    P = catmull(pts, per) if smooth_path else [V(p) for p in pts]
    if isinstance(radius, (list, tuple)):
        R = catmull([V((r, 0, 0)) for r in radius], per) if smooth_path else [V((r, 0, 0)) for r in radius]
        R = [r.x for r in R]
    else: R = [radius] * len(P)
    F = frame_along(P)
    if twist_ref is not None:
        F2 = []
        for p, t, n, b in F:
            ref = V(twist_ref); n = (ref - t * ref.dot(t)).normalized(); b = t.cross(n); F2.append((p, t, n, b))
        F = F2
    rings = []
    for (p, t, n, b), r in zip(F, R):
        ring = []
        for j in range(sides):
            u = 2 * math.pi * j / sides
            if shape: x, y = shape(u, r)
            else: x, y = math.cos(u) * r, math.sin(u) * r
            ring.append(p + n * x + b * y)
        rings.append(ring)
    o = loft(rings, closed=True, cap=cap, name=name, tag=tag, node=node)
    return o

def rrect(w, h, rad, n=3):
    """rounded rectangle shape function for sweep: returns shape(u, r)."""
    def f(u, r):
        # superellipse approx
        c, s = math.cos(u), math.sin(u); e = 2.0 / max(0.15, rad)
        x = math.copysign(abs(c) ** (2 / e), c) * w / 2; y = math.copysign(abs(s) ** (2 / e), s) * h / 2
        k = r if r else 1
        return x * k, y * k
    return f

def box(c, size, bevel=0.2, sub=2, name="box", tag="alu", node="Bike", rot=None):
    bm = bmesh.new(); bmesh.ops.create_cube(bm, size=1)
    bmesh.ops.scale(bm, vec=V(size), verts=bm.verts)
    if rot: bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=rot)
    bmesh.ops.translate(bm, vec=V(c), verts=bm.verts)
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free(); o = link(name, me, tag, node)
    if bevel:
        b = o.modifiers.new("bev", "BEVEL"); b.width = bevel * min(size) * 0.5; b.segments = 2; b.limit_method = "NONE"
    if sub: subsurf(o, sub)
    return o

def cyl(a, b, r, segs=16, r2=None, cap=True, name="cyl", tag="steel", node="Bike", bevel=0.0):
    a, b = V(a), V(b)
    prof = [(0.0, 0.0)] if cap else []
    rr2 = r if r2 is None else r2; L = (b - a).length
    if bevel:
        prof += [(r - bevel, 0), (r, bevel), (rr2, L - bevel), (rr2 - bevel, L)]
    else:
        prof += [(r, 0), (rr2, L)]
    if cap: prof += [(0.0, L)]
    o = lathe(prof, a, b - a, segs, name, tag, node)
    if cap: weld(o)
    return o

def weld(o, d=1e-6):
    bm = bmesh.new(); bm.from_mesh(o.data); bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=d); bm.to_mesh(o.data); bm.free()

def mirror_x(o, name=None):
    me = o.data.copy(); o2 = bpy.data.objects.new(name or o.name + "_m", me); bpy.context.scene.collection.objects.link(o2)
    for k in o.keys(): o2[k] = o[k]
    for v in me.vertices: v.co.x = -v.co.x
    me.flip_normals()
    for m in o.modifiers:
        n = o2.modifiers.new(m.name, m.type)
        for attr in ("levels", "render_levels", "width", "segments", "limit_method", "thickness", "offset"):
            if hasattr(m, attr):
                try: setattr(n, attr, getattr(m, attr))
                except Exception: pass
    OBJ.append(o2); return o2

def array_rot(o, n, axis_pt, axis_dir, phase=0.0):
    """Repeat the object's mesh n times around an axis, in place."""
    me = o.data; ax = V(axis_dir).normalized(); P = V(axis_pt)
    vs = [v.co.copy() for v in me.vertices]; fs = [tuple(p.vertices) for p in me.polygons]
    V2 = []; F2 = []
    for i in range(n):
        m = Matrix.Translation(P) @ Matrix.Rotation(phase + 2 * math.pi * i / n, 4, ax) @ Matrix.Translation(-P)
        base = len(V2); V2 += [m @ v for v in vs]; F2 += [tuple(base + k for k in f) for f in fs]
    me.clear_geometry(); me.from_pydata([tuple(v) for v in V2], [], F2); me.update()
    for p in me.polygons: p.use_smooth = True
    return o

def join(objs, name):
    """join meshes (modifiers applied) into one object; keeps first's tags"""
    import bpy
    dg = bpy.context.evaluated_depsgraph_get()
    V2 = []; F2 = []
    for o in objs:
        ev = o.evaluated_get(dg); me = ev.to_mesh(); me.transform(o.matrix_world)
        base = len(V2); V2 += [v.co.copy() for v in me.vertices]; F2 += [tuple(base + k for k in p.vertices) for p in me.polygons]
        ev.to_mesh_clear()
    me = mesh_from(V2, F2, name); t = objs[0]
    o = link(name, me, t["tag"], t["node"])
    for x in objs:
        if x in OBJ: OBJ.remove(x)
        bpy.data.objects.remove(x)
    return o
