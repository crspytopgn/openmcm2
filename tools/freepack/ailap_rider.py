# Cuts the Free Pack rider (FitPose) and MX2 bike, posed and baked in the
# browser (posed.json), to a few hundred flat-coloured triangles for the
# Tools page's AI lap: roles paint, kit, dark, metal.
import json, struct, sys
import numpy as np
import fast_simplification as fs

parts = json.load(open(sys.argv[1]))
DARK = {"2c2c2c", "353538", "303030", "4b4b4d", "505052", "55504b", "896130"}
PAINT = {"livery", "plate", "pasted__petrol_tank1", "pasted__blue1", "pasted__button1"}

def role(p):
    if p["who"] == "rider":
        if p["mat"] != "outfit":
            return "dark"
        if p["node"] == "Head":
            return "paint"
        if p["node"].startswith(("Hand", "Foot")):
            return "dark"
        return "kit"
    if p["mat"] in PAINT:
        return "paint"
    return "dark" if p["col"] in DARK else "metal"

# Triangle budget per role, shared between its pieces by their size.
BUDGET = {"paint": 220, "kit": 300, "dark": 340, "metal": 180}
groups = {}
for p in parts:
    P = np.array(p["P"], dtype=np.float64).reshape(-1, 3)
    I = np.array(p["I"], dtype=np.int64).reshape(-1, 3)
    groups.setdefault((role(p), p["who"], p["node"] if p["who"] == "rider" else ""), []).append((P, I))
total = {}
for (r, _, _), pieces in groups.items():
    total[r] = total.get(r, 0) + sum(len(I) for _, I in pieces)

import trimesh

# Each piece as its convex hull, cut down: limbs, plastics and wheels as
# simple blocks, the look Michael liked, with the real shapes and pose.
out = {r: ([], []) for r in BUDGET}
for (r, who, node), pieces in groups.items():
    P = np.concatenate([q for q, _ in pieces])
    off = np.cumsum([0] + [len(q) for q, _ in pieces[:-1]])
    I = np.concatenate([i + o for (_, i), o in zip(pieces, off)])
    m = trimesh.Trimesh(P, I, process=True)
    comps = m.split(only_watertight=False) if who == "bike" else [m]
    # Bits under this size (bolts, cables, levers) go: they'd be specks.
    comps = [c for c in comps if c.extents.max() > (0.7 if who == "bike" else 0.05)]
    for c in comps:
        try:
            h = c.convex_hull
        except Exception:
            continue
        want = 22 if who == "rider" else int(np.clip(c.extents.max() * 6, 10, 28))
        if r == "dark" and c.extents.max() > 1.8:
            want = 40  # the tyres: round enough to read as wheels
        V, F = np.asarray(h.vertices, dtype=np.float32), np.asarray(h.faces, dtype=np.int32)
        if len(F) > want:
            V, F = fs.simplify(V, F, target_reduction=1 - want / len(F))
        V = np.asarray(V) * [-1, 1, -1]  # the game faces +z, the pack -z
        vs, ts = out[r]
        base = sum(len(v) for v in vs)
        vs.append(V)
        # Mirroring flips the winding: put it back.
        ts.append(np.asarray(F)[:, ::-1] + base)

blob = bytearray(b"LPR1")
for r in ["paint", "kit", "dark", "metal"]:
    vs, ts = out[r]
    V = np.concatenate(vs)
    T = np.concatenate(ts)
    q = np.clip(np.round(V * 1000), -32767, 32767).astype("<i2")
    blob += struct.pack("<HH", len(V), len(T))
    blob += q.tobytes() + T.astype("<u2").tobytes()
    print(r, len(V), "verts", len(T), "tris")
open(sys.argv[2], "wb").write(blob)
print(len(blob), "bytes")
