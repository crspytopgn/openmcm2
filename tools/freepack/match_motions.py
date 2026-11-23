"""Matches the free rider's and bike's motions to the original game's:
every riding, trick, crash, menu and finish motion takes the original's
poses and timing, with the feet and hands refitted onto the free bike's
pegs and grips (seated_pose.py SEATED), so they sit where the free
bike's are.

Needs the original game's motions, dumped from an owned copy of the game
with the engine's motion_dump example (run from the engine folder):

    for m in Rider Bike UIRider UIHonda; do
      cargo run -q --release -p mm2-formats --example motion_dump -- \\
        GAME $m.mcf $(cargo run -q --release -p mm2-formats --example mcf_list -- GAME $m.mcf | cut -f1) \\
        > DUMPS/$m.json
    done
    python tools/freepack/free_motions.py crates/mm2-viewer/src/free_motions.bin --match DUMPS

free_motions.py calls match() with its own tables and packs the result;
the podium's motions stay the pack's own (its podium is laid out apart).
"""
import json, math, os
import numpy as np
from seated_pose import REST, ORDER, SEATED

# The original's seated rider (Turn2D's centred frame): where the feet sit
# on the pegs and the wrists over the grips, model space, ft.
ORIG_ANKLE = np.array([0.62, 1.85, -0.80])

def _R(look, up):
    l = np.array(look, float); u = np.array(up, float)
    l /= np.linalg.norm(l) or 1; u = u - l * (u @ l); u /= np.linalg.norm(u) or 1
    return np.stack([np.cross(u, l), u, l], 1)

def _rest(tree, i):
    m = np.array(tree[i]['m']).reshape(4, 4)
    return m[:3, :3].T, m[3, :3]

def world(tree, keys):
    """Each frame's world rotation and place for one motion frame."""
    loc = {k[0]: (_R(k[1], k[2]), np.array(k[3], float)) for k in keys}
    out = []
    for i, f in enumerate(tree):
        r, t = loc.get(i) or _rest(tree, i)
        p = f['parent']
        out.append((r, t) if p < 0 else (out[p][0] @ r, out[p][1] + out[p][0] @ t))
    return out

def to_keys(tree, W):
    keys = []
    for i, f in enumerate(tree):
        R, t = W[i]; p = f['parent']
        if p >= 0:
            Rp, tp = W[p]; R = Rp.T @ R; t = Rp.T @ (t - tp)
        keys.append((i, R[:, 2].round(5).tolist(), R[:, 1].round(5).tolist(), np.round(t, 5).tolist()))
    return keys

def turn_onto(a, b):
    """The least rotation taking direction a onto b."""
    a = a / np.linalg.norm(a); b = b / np.linalg.norm(b); v = np.cross(a, b); c = float(a @ b)
    if np.linalg.norm(v) < 1e-9:
        return np.eye(3)
    K = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + K + K @ K * (1 / (1 + c))

def two_bone(A, T, L1, L2, knee):
    """Mid joint for a limb from A reaching T, bending toward `knee`."""
    d = T - A; D = min(np.linalg.norm(d), L1 + L2 - 1e-4); dn = d / np.linalg.norm(d)
    a = (L1 * L1 - L2 * L2 + D * D) / (2 * D); h = math.sqrt(max(0.0, L1 * L1 - a * a))
    hint = knee - A; hv = hint - dn * (hint @ dn)
    hv = hv / (np.linalg.norm(hv) or 1)
    return A + dn * a + hv * h, A + dn * D

def weight(d, full, none):
    return float(np.clip((none - d) / (none - full), 0.0, 1.0))

# The free bike's seat is lower than the original's: a rider on it sits
# this much lower (ft, game axes) and leans the upper body forward this
# many degrees, over the bars, with the head kept up by most of it; the
# elbows bend out and up. Fades out as the pelvis leaves the seat
# (tricks, crashes).
SEAT_DROP = np.array([0.0, -0.6, 0.05])
# ...but never with the hips below this height (ft) over the ground.
SEAT_HIP = 3.5
# On the seat with the pelvis behind SEAT_BACK (ft, model z), off it
# (standing forward over the tank) ahead of SEAT_FRONT.
SEAT_BACK, SEAT_FRONT = -1.0, -0.6
SEAT_LEAN = 0.0
# Standing up over the tank (the pelvis forward of this, ft) leans this
# much more: the attack position, chest over the bars.
ATTACK_Z, ATTACK_LEAN = -1.0, 6.0
HEAD_KEEP = 0.8
# The free bike's pegs sit this much lower (ft) than the original's: a
# rider standing on them comes down by as much, so the knees keep the
# original's bend instead of straightening.
PEG_DROP = ORIG_ANKLE[1] - SEATED['ankle'][1]
ELBOW_BIAS = np.array([0.0, 0.3, 0.0])

def _axis_angle(a, deg):
    a = a / np.linalg.norm(a); t = math.radians(deg); c, s = math.cos(t), math.sin(t)
    K = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + s * K + (1 - c) * K @ K

def settle(tree, W, home):
    """Sits the rider down onto the free bike's seat (see SEAT_DROP)."""
    idx = {f['name']: i for i, f in enumerate(tree)}
    w = weight(np.linalg.norm(W[idx['Pelvis']][1] - home), 0.6, 1.6)
    if w <= 0:
        return list(W)
    # Only a rider on the seat sits down onto it: one forward over the
    # tank is standing on the pegs, as the original rides most turns
    # (Turn1D), and keeps the original's height.
    sit = w * weight(-W[idx['Pelvis']][1][2], -SEAT_BACK, -SEAT_FRONT)
    y = W[idx['Pelvis']][1][1]
    d = SEAT_DROP * sit
    d[1] = -max(min(-SEAT_DROP[1], max(0.0, y - SEAT_HIP)) * sit, PEG_DROP * w)
    W = [(R, t + d) for R, t in W]
    kids = {i: [j for j, f in enumerate(tree) if f['parent'] == i] for i in range(len(tree))}
    def sub(i):
        yield i
        for j in kids[i]:
            yield from sub(j)
    lt = idx['LwrTorso']
    z = W[idx['Pelvis']][1][2]
    lean = SEAT_LEAN * w + ATTACK_LEAN * float(np.clip((z - ATTACK_Z) / 0.6, 0.0, 1.0)) * (w > 0)
    Q = _axis_angle(W[lt][0][:, 0], lean); j = W[idx['UprTorso']][1]
    for i in sub(idx['UprTorso']):
        R, t = W[i]; W[i] = (Q @ R, j + Q @ (t - j))
    h = idx['Head']
    W[h] = (_axis_angle(W[lt][0][:, 0], -lean * HEAD_KEEP) @ W[h][0], W[h][1])
    return W

def refit(tree, W, grips, pre=None):
    """Moves the feet near the original's pegs and the hands near the
    original's grips onto the free bike's, re-bending the limbs. `pre`:
    the pose before settle(), whose hands and feet say where they go."""
    idx = {f['name']: i for i, f in enumerate(tree)}
    free_ankle = np.array(SEATED['ankle'], float)
    W = list(W); pre = W if pre is None else pre
    for s, sx in (('L', -1), ('R', 1)):
        for upper, lower, end, orig, new, full, none in (
            ('UprLeg', 'LwrLeg', 'Foot', ORIG_ANKLE * [sx, 1, 1], free_ankle * [sx, 1, 1], 0.45, 1.0),
            ('UprArm', 'LwrArm', 'Hand', grips[0] * [sx, 1, 1], grips[1] * [sx, 1, 1], 0.9, 1.6),
        ):
            iu, il, ie = idx[upper + s], idx[lower + s], idx[end + s]
            A, K, T = W[iu][1], W[il][1], W[ie][1]
            Tp = pre[ie][1]
            w = weight(np.linalg.norm(Tp - orig), full, none)
            if w <= 0:
                continue
            T2 = T + (Tp + (new - orig) - T) * w
            L1, L2 = np.linalg.norm(K - A), np.linalg.norm(T - K)
            bias = ELBOW_BIAS * [sx, 1, 1] * w if upper == 'UprArm' else 0
            K2, T2 = two_bone(A, T2, L1, L2, K + bias)
            clear = SEATED.get('knee_clear') if upper == 'UprLeg' else None
            hint = K + bias
            for _ in range(12):
                # Knees kept outside the bike's shrouds: the bend turned
                # outward until the knee clears.
                if clear is None or sx * K2[0] >= clear * w + sx * K[0] * (1 - w) - 1e-3 or sx * K2[0] >= clear:
                    break
                hint = hint + np.array([sx * 0.25, 0.0, 0.0])
                K2, T2 = two_bone(A, T2, L1, L2, hint)
            Ru = turn_onto(K - A, K2 - A) @ W[iu][0]
            Rl = turn_onto(T - K, T2 - K2) @ W[il][0]
            W[iu] = (Ru, A); W[il] = (Rl, K2); W[ie] = (W[ie][0], T2)
    return W

def match(rider, bike, timing, dumps):
    """Replaces the tables' motions with ones matched to the original's
    (dumps: the folder of motion_dump JSON files)."""
    load = lambda n: json.load(open(os.path.join(dumps, n + '.json')))
    R, B, UR, UB = load('Rider'), load('Bike'), load('UIRider'), load('UIHonda')
    tree = R['frames']
    assert [f['name'] for f in tree] == ORDER, 'the rider frame tree differs from the free rider'
    # Where the original's and the free rider's hands sit on the grips.
    centre = world(tree, R['motions']['Turn2D'][0])
    orig_grip = centre[ORDER.index('HandR')][1]
    from free_motions import pose
    free_grip = pose(None)['HandR'][1]
    grips = (np.array(orig_grip), np.array(free_grip))
    home = np.array(centre[ORDER.index('Pelvis')][1])
    info = {}
    for d, kind in ((R, 'Rider'), (UR, 'UIRider')):
        for name, frames in d['motions'].items():
            rider[name] = [to_keys(tree, refit(tree, settle(tree, world(tree, k), home), grips, world(tree, k)))
                           for k in frames]
            info[name] = d.get('info', {}).get(name)
    for d in (B, UB):
        btree = d['frames']
        for name, frames in d['motions'].items():
            bike[name] = [to_keys(btree, world(btree, k)) for k in frames]
            info[name] = d.get('info', {}).get(name)
    for name, i in info.items():
        if i:
            timing[name] = (float(i['fps']), bool(i['looping']))
    return grips
