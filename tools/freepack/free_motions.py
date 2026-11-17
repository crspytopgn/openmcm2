"""Makes the Free Pack rider's and bike's own riding motions (the engine's
free_motions.rs), for when the original game's Rider.mcf / Bike.mcf motions
aren't there. Same names and phase meanings as the game's, so the engine's
riding mix (riding.rs) drives them unchanged:

- steer motions (LeanR01-03, Turn1D-3U and the bike's B*): phase 0 centre,
  0.25 full right, 0.5 centre, 0.75 full left, 1 centre;
- rows 1-3: weight forward, middle, back;
- CrosOv1-3 / CrosBik1-3: a cross-up through the air, phase 0 to 1;
- LookBack: phase 0.5 looks ahead, 0 and 1 over each shoulder.

Poses come from two-bone IK onto the free bikes' seat, pegs and grips
(seated_pose.py). Writes Rust to the path given.

    python tools/freepack/free_motions.py crates/mm2-viewer/src/free_motions.rs
"""
import math, sys, os
import numpy as np
sys.path.insert(0, os.path.dirname(__file__))
from seated_pose import REST, ORDER, SEATED, solve, locals_, rx, ry, rz

STEPS = 9
BAR_PIVOT = np.array([0.0, 3.701, 0.95])
STEER_AXIS = np.array([0.0, 2.55, -1.358]) / np.linalg.norm([0.0, 2.55, -1.358])
BODY_PIVOT = np.array([0.0, 2.3, 0.0])
BIKE = [  # frame, parent, rest local translation (game axes), as the free bikes have it
    ("Bike", None, (0, 0, 0)), ("HandleBars", 0, (0, 3.701, 0.95)), ("KickStart", 0, (0.381, 1.592, -0.816)),
    ("NullFrame", 0, (0, 2.5, -0.333)), ("SwingArm", 0, (0, 1.619, -0.731)), ("Shocks", 1, (0, 0, 0)),
    ("TireFront", 5, (0, -2.55, 1.358)), ("TireRear", 4, (0, -0.504, -1.926))]

def axis_angle(a, deg):
    a = a / np.linalg.norm(a); t = math.radians(deg); c, s = math.cos(t), math.sin(t)
    K = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + s * K + (1 - c) * K @ K

def steer_of(p):
    """Right turn amount, -1..1, at phase p of a steer motion."""
    return math.sin(2 * math.pi * p)

# Rows: weight forward (an attacking, half-standing rider), middle, back.
# The wrist just behind and above the grip, so the glove lies over the bar.
SEATED = dict(SEATED, wrist_from_grip=(0.0, 0.03, -0.26))
ROWS = [
    dict(pelvis=(0, 4.15, -0.62), pelvis_pitch=34, lower_pitch=10, upper_pitch=8, head_pitch=-18, elbow_out=1.4,
         ankle=(0.66, 1.64, -0.72), knee_out=0.45),
    dict(),
    dict(pelvis=(0, 3.58, -1.32), pelvis_pitch=6, lower_pitch=8, upper_pitch=12, head_pitch=0,
         wrist_from_grip=(0.0, 0.0, -0.22)),
]
STAND = dict(pelvis=(0, 4.55, -0.55), pelvis_pitch=30, lower_pitch=8, upper_pitch=6, head_pitch=-16, knee_out=0.35,
             elbow_out=1.5)

def pose(base=None, steer=0.0, bars=0.0, roll=0.0, head_yaw=0.0, torso_yaw=0.0, foot_out=0.0, body=None, **over):
    """World pose: `steer` right -1..1 turns the bars by `bars` degrees and
    leans the torso `roll` degrees into the turn; `foot_out` puts the inside
    foot out forward (slow corners); `body` (R) turns the whole rider and
    bike about BODY_PIVOT (cross-ups)."""
    P = dict(SEATED); P.update(base or {}); P.update(over)
    P['roll'] = -roll * steer; P['head_yaw'] = head_yaw; P['torso_yaw'] = torso_yaw
    turn = axis_angle(STEER_AXIS, bars * steer)
    grips = {}
    for s, sx in (('L', -1), ('R', 1)):
        g = np.array(P['grip']) * np.array([sx, 1, 1])
        grips[s] = turn @ (g - BAR_PIVOT) + BAR_PIVOT
    ankles = {s: np.array(P['ankle']) * np.array([sx, 1, 1]) for s, sx in (('L', -1), ('R', 1))}
    if foot_out:
        inside = 'R' if steer > 0 else 'L'; sx = 1 if inside == 'R' else -1; k = abs(steer) * foot_out
        ankles[inside] = ankles[inside] + np.array([sx * 0.75, 0.55, 2.0]) * k
    B = body if body is not None else np.eye(3)
    move = lambda v: B @ (v - BODY_PIVOT) + BODY_PIVOT
    if 'grip_r' in P:
        grips['R'] = np.array(P['grip_r'], float)
    P['grips'] = {s: move(v) for s, v in grips.items()}
    P['ankles'] = {s: move(v) for s, v in ankles.items()}
    P['pelvis'] = move(np.array(P['pelvis'], float))
    P['body'] = B
    return solve_ex(P)

def solve_ex(P):
    """solve() with roll, yaws, per-side targets and a body turn."""
    import seated_pose as sp
    W = {}
    B = P['body']
    Rp = B @ ry(P['torso_yaw'] * 0.3) @ rx(P['pelvis_pitch']); W['Pelvis'] = (Rp, P['pelvis'])
    Rl = Rp @ rz(P['roll'] * 0.5) @ ry(P['torso_yaw'] * 0.35) @ rx(P['lower_pitch'])
    W['LwrTorso'] = (Rl, W['Pelvis'][1] + Rp @ np.array(REST['LwrTorso'][1]))
    Ru = Rl @ rz(P['roll'] * 0.5) @ ry(P['torso_yaw'] * 0.35) @ rx(P['upper_pitch'])
    W['UprTorso'] = (Ru, W['LwrTorso'][1] + Rl @ np.array(REST['UprTorso'][1]))
    Rh = B @ ry(P['head_yaw']) @ rx(P['head_pitch']); W['Head'] = (Rh, W['UprTorso'][1] + Ru @ np.array(REST['Head'][1]))
    for s, sx in (('L', -1), ('R', 1)):
        hip = W['Pelvis'][1] + Rp @ np.array(REST['UprLeg' + s][1])
        L1 = np.linalg.norm(REST['LwrLeg' + s][1]); L2 = np.linalg.norm(REST['Foot' + s][1])
        knee, ank = sp.two_bone(hip, P['ankles'][s], L1, L2, B @ np.array([sx * P['knee_out'], 0.2, 1.0]))
        bend = sp.nrm(np.cross(knee - hip, ank - knee))
        if (B.T @ bend)[0] < 0: bend = -bend
        W['UprLeg' + s] = (sp.align(np.array(REST['LwrLeg' + s][1]), np.array([1.0, 0, 0]), knee - hip, bend), hip)
        W['LwrLeg' + s] = (sp.align(np.array(REST['Foot' + s][1]), np.array([1.0, 0, 0]), ank - knee, bend), knee)
        W['Foot' + s] = (B @ ry(-sx * P['foot_yaw']) @ rx(P['foot_pitch']), ank)
        sh = W['UprTorso'][1] + Ru @ np.array(REST['UprArm' + s][1])
        A1 = np.linalg.norm(REST['LwrArm' + s][1]); A2 = np.linalg.norm(REST['Hand' + s][1])
        grip = P['grips'][s]
        wrist_t = grip + B @ (np.array(P['wrist_from_grip']) * np.array([sx, 1, 1]))
        el, wr = sp.two_bone(sh, wrist_t, A1, A2, B @ np.array([sx * P['elbow_out'], -0.3, -0.6]))
        bend = sp.nrm(np.cross(el - sh, wr - el))
        if (B.T @ bend)[0] * sx > 0: bend = -bend
        W['UprArm' + s] = (sp.align(np.array(REST['LwrArm' + s][1]), B @ np.array([-sx * 1.0, 0, 0]), el - sh, bend), sh)
        W['LwrArm' + s] = (sp.align(np.array(REST['Hand' + s][1]), B @ np.array([-sx * 1.0, 0, 0]), wr - el, bend), el)
        # Wrapped round the grip: the fingers (the hand's -y) from the
        # wrist over the bar, the palm (its inside, -x on the right) down.
        hd = grip + B @ np.array([0.0, -0.3, 0.1]) - wr
        W['Hand' + s] = (sp.align(np.array([0, -1.0, 0]), np.array([-sx * 1.0, 0, 0]), hd, B @ np.array([0, -1.0, 0])) @ ry(sx * P['hand_roll']), wr)
    return W

def keys(W, only=None):
    L = locals_(W)
    return [(i, L[n]['look'], L[n]['up'], L[n]['position']) for i, n in enumerate(ORDER) if only is None or n in only]

def bike_keys(bars=0.0, body=None, pivot_lift=0.0):
    B = body if body is not None else np.eye(3)
    out = []
    for i, (name, parent, t) in enumerate(BIKE):
        R = np.eye(3); t = np.array(t, float)
        if name == "Bike":
            R = B; t = BODY_PIVOT - B @ BODY_PIVOT
        if name == "HandleBars":
            R = axis_angle(STEER_AXIS, bars)
        out.append((i, R[:, 2].round(5).tolist(), R[:, 1].round(5).tolist(), t.round(5).tolist()))
    return out

def phases(n=STEPS): return [k / (n - 1) for k in range(n)]

rider, bike = {}, {}
# Steer motions: slow (LeanR, bars far round, inside foot out) and at speed
# (Turn, bars a little, leaning in).
for r in range(3):
    rider[f"LeanR0{r+1}"] = [keys(pose(ROWS[r], steer_of(p), bars=24, roll=8, head_yaw=-14 * steer_of(p), foot_out=0.85)) for p in phases()]
    for kind, base in (("D", ROWS[r]), ("U", dict(ROWS[r], **STAND))):
        rider[f"Turn{r+1}{kind}"] = [keys(pose(base, steer_of(p), bars=9, roll=10, head_yaw=-10 * steer_of(p))) for p in phases()]
bike["BLeanR02"] = [bike_keys(bars=24 * steer_of(p)) for p in phases()]
bike["BTurn2D"] = [bike_keys(bars=9 * steer_of(p)) for p in phases()]
bike["BTurn2U"] = bike["BTurn2D"]
# Wheelie: sat back, arms long.
rider["Wheelie"] = [keys(pose(ROWS[2], pelvis=(0, 3.55, -1.5), pelvis_pitch=-4, upper_pitch=4, head_pitch=6,
                             wrist_from_grip=(0.0, 0.04, -0.18))) for _ in range(2)]
bike["BWheelie"] = [bike_keys() for _ in range(2)]
# Cross-ups: the bike laid over and turned through the air and back, the
# rider's legs and hips going with it, the shoulders staying square.
for r, (yaw, lay) in enumerate(((22, 28), (30, 38), (36, 48))):
    frames_r, frames_b = [], []
    for p in phases():
        k = math.sin(math.pi * p)
        B = ry(-yaw * k) @ rz(-lay * k)
        frames_r.append(keys(pose(STAND, body=B, torso_yaw=yaw * k * 0.8, head_yaw=yaw * k * 0.9)))
        frames_b.append(bike_keys(body=B))
    rider[f"CrosOv{r+1}"] = frames_r
    bike[f"CrosBik{r+1}"] = frames_b
# Looking back over a shoulder.
rider["LookBack"] = [keys(pose(STAND, head_yaw=(p - 0.5) * 2 * 110, torso_yaw=(p - 0.5) * 2 * 40)) for p in phases()]
bike["BLookBack"] = [bike_keys() for _ in range(2)]
# A fist in the air at the finish.
rider["Victory"] = [keys(pose(STAND, grip_r=(1.3, 7.6, -0.2), head_pitch=-30)) for _ in range(2)]
# Backing up: sat down, feet on the ground (paddling, layered over the legs).
rider["BackUpTop"] = [keys(pose(ROWS[1], steer_of(p), bars=24, ankle=(1.05, 0.3, -0.2), knee_out=0.9)) for p in phases()]
bike["bBackUpTop"] = bike["BLeanR02"]
legs = {"UprLegL", "LwrLegL", "FootL", "UprLegR", "LwrLegR", "FootR"}
rider["BackUpBottom"] = [keys(pose(ROWS[1], ankle=(1.05, 0.3 + 0.25 * math.sin(2 * math.pi * p), -0.2 + 0.9 * math.cos(2 * math.pi * p)), knee_out=0.9), legs) for p in phases()]

def emit(out, table):
    for name, frames in table.items():
        out.append(f"    (\"{name}\", &[\n")
        for f in frames:
            out.append("        &[" + ", ".join(
                f"({i}, [{', '.join(f'{v:.4}' for v in l)}], [{', '.join(f'{v:.4}' for v in u)}], [{', '.join(f'{v:.4}' for v in t)}])"
                for i, l, u, t in f) + "],\n")
        out.append("    ]),\n")

if __name__ == '__main__':
    out = ["// Made by tools/freepack/free_motions.py: do not edit by hand.\n",
           "// The Free Pack rider's and bike's own riding motions (see freepack.rs).\n\n",
           "/// (name, frames: (frame, look, up, position) keys)\n",
           "pub type Table = &'static [(&'static str, &'static [&'static [(u8, [f32; 3], [f32; 3], [f32; 3])]])];\n\n",
           "#[rustfmt::skip]\npub const RIDER: Table = &[\n"]
    emit(out, rider); out.append("];\n\n#[rustfmt::skip]\npub const BIKE: Table = &[\n"); emit(out, bike); out.append("];\n")
    open(sys.argv[1], 'w').write("".join(out))
    print("rider", len(rider), "bike", len(bike))
