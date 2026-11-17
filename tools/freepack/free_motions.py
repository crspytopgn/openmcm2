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
(seated_pose.py), and joint angles for the crashes. Writes the packed
motions to the path given.

    python tools/freepack/free_motions.py crates/mm2-viewer/src/free_motions.bin
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
    for side in 'LR':
        if P.get('ankle_' + side.lower()) is not None:
            ankles[side] = np.array(P['ankle_' + side.lower()], float)
    if foot_out:
        inside = 'R' if steer > 0 else 'L'; sx = 1 if inside == 'R' else -1; k = abs(steer) * foot_out
        ankles[inside] = ankles[inside] + np.array([sx * 0.75, 0.55, 2.0]) * k
    B = body if body is not None else np.eye(3)
    move = lambda v: B @ (v - BODY_PIVOT) + BODY_PIVOT
    for side in 'LR':
        if P.get('grip_' + side.lower()) is not None:
            grips[side] = np.array(P['grip_' + side.lower()], float)
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
# Each motion's frames a second and whether it loops; the riding mix plays
# the rest by phase.
TIMING = {}
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

# Crashes (wreck.rs): the bails play on the bike (12 frames at 15 a second,
# Kahuna 25; the rider is let go half way, with the pelvis's speed and
# spin), the falls loop while the body tumbles (the motion does the
# turning), and a landing settles into lying on the ground.
def fk(pelvis=(0, 3.62, -1.02), R=None, spine=(0, 0, 0), head=(0, 0), legs=((0, 0, 0), (0, 0, 0)),
       arms=((0, 0, 0), (0, 0, 0))):
    """A pose by joint angles, degrees. spine (pitch, roll, yaw) split over
    the two torso parts; legs/arms per side (L, R): (swing back, out, bend)
    with bend as the knee's (shin back) or the elbow's (forearm forward)."""
    W = {}
    Rp = np.eye(3) if R is None else R
    W['Pelvis'] = (Rp, np.array(pelvis, float))
    sp_ = rx(spine[0] / 2) @ rz(spine[1] / 2) @ ry(spine[2] / 2)
    Rl = Rp @ sp_; W['LwrTorso'] = (Rl, W['Pelvis'][1] + Rp @ np.array(REST['LwrTorso'][1]))
    Ru = Rl @ sp_; W['UprTorso'] = (Ru, W['LwrTorso'][1] + Rl @ np.array(REST['UprTorso'][1]))
    W['Head'] = (Ru @ ry(head[1]) @ rx(head[0]), W['UprTorso'][1] + Ru @ np.array(REST['Head'][1]))
    for k, (s, sx) in enumerate((('L', -1), ('R', 1))):
        swing, out, bend = legs[k]
        Rt = Rp @ rz(sx * out) @ rx(swing); hip = W['Pelvis'][1] + Rp @ np.array(REST['UprLeg' + s][1])
        W['UprLeg' + s] = (Rt, hip)
        Rs = Rt @ rx(bend); W['LwrLeg' + s] = (Rs, hip + Rt @ np.array(REST['LwrLeg' + s][1]))
        W['Foot' + s] = (Rs, W['LwrLeg' + s][1] + Rs @ np.array(REST['Foot' + s][1]))
        swing, out, bend = arms[k]
        Ra = Ru @ rz(sx * out) @ rx(swing); sh = W['UprTorso'][1] + Ru @ np.array(REST['UprArm' + s][1])
        W['UprArm' + s] = (Ra, sh)
        Rf = Ra @ rx(-bend); W['LwrArm' + s] = (Rf, sh + Ra @ np.array(REST['LwrArm' + s][1]))
        W['Hand' + s] = (Rf, W['LwrArm' + s][1] + Rf @ np.array(REST['Hand' + s][1]))
    return W

def mix(a, b, t):
    """Keys between two key lists (the engine squares the axes up)."""
    return [(i, (np.array(la) * (1 - t) + np.array(lb) * t).tolist(), (np.array(ua) * (1 - t) + np.array(ub) * t).tolist(),
             (np.array(pa) * (1 - t) + np.array(pb) * t).tolist()) for (i, la, ua, pa), (_, lb, ub, pb) in zip(a, b)]

def ease(t): return t * t * (3 - 2 * t)

seated = keys(pose(ROWS[1]))
SPREAD = dict(legs=((-30, 25, 40), (-20, 30, 55)), arms=((-120, 40, 30), (-100, 50, 20)), spine=(10, 0, 0), head=(-10, 0))
BAIL_ENDS = {
    "Endo": (12, fk(pelvis=(0, 5.6, 2.2), R=rx(70), spine=(10, 0, 0), legs=((40, 10, 30), (30, 15, 20)), arms=((-150, 20, 10), (-140, 25, 10)), head=(-30, 0))),
    "LeftOver": (10, fk(pelvis=(-2.4, 4.2, 0.2), R=rz(55), spine=(10, 15, 0), legs=((-40, 20, 60), (-20, 35, 30)), arms=((-60, 70, 20), (-90, 60, 30)))),
    "RightOver": (10, fk(pelvis=(2.4, 4.2, 0.2), R=rz(-55), spine=(10, -15, 0), legs=((-20, 35, 30), (-40, 20, 60)), arms=((-90, 60, 30), (-60, 70, 20)))),
    "BackOver": (13, fk(pelvis=(0, 4.8, -2.8), R=rx(-55), spine=(-15, 0, 0), legs=((-70, 15, 40), (-60, 20, 30)), arms=((-160, 30, 10), (-150, 35, 10)), head=(25, 0))),
    "Kahuna": (25, fk(pelvis=(0, 7.5, 2.8), R=rx(80), spine=(-10, 0, 0), legs=((30, 15, 10), (25, 20, 15)), arms=((-170, 15, 0), (-165, 20, 0)), head=(-40, 0))),
}
for name, (n, end) in BAIL_ENDS.items():
    endk = keys(end)
    rider[name] = [mix(seated, endk, ease(min(1.0, k / (n - 1) * 1.25))) for k in range(n)]
    TIMING[name] = (15.0, False)
# As long as the game's: falls 21 frames (looping), landings 43 (on the
# feet 53), all at 15 a second.
FALL_FRAMES = 21
FALL_AXES = {"Fall01": (1, 0, 0), "Fall02": (-1, 0, 0), "Fall03": (0, 0, 1), "Fall04": (0.6, 0.5, 0.6)}
for f, (name, axis) in enumerate(FALL_AXES.items()):
    frames = []
    for k in range(FALL_FRAMES):
        a = 2 * math.pi * k / (FALL_FRAMES - 1); w = math.sin(a + f)
        frames.append(keys(fk(pelvis=(0, 3.2, -0.3), R=axis_angle(np.array(axis, float), 360 * k / (FALL_FRAMES - 1)),
            spine=(10 + 15 * w, 10 * math.cos(a), 0), head=(-10 + 20 * w, 15 * math.cos(a)),
            legs=((-30 + 35 * w, 25 + 10 * math.cos(a), 40 + 30 * w), (-20 - 35 * w, 30 - 10 * math.cos(a), 55 - 30 * w)),
            arms=((-120 + 50 * w, 45 + 20 * math.cos(a), 30 + 20 * w), (-100 - 50 * w, 50 - 20 * math.cos(a), 20 - 20 * w)))))
    rider[name] = frames
    TIMING[name] = (15.0, True)
LYING = {  # where each landing ends: on the left side, the right, sat down, on the back
    "LeftHit": fk(pelvis=(0, 0.6, 0), R=rz(80), spine=(15, 10, 0), legs=((-30, 10, 50), (-50, 5, 70)), arms=((-40, 70, 60), (-70, 30, 80)), head=(-10, 0)),
    "RightHit": fk(pelvis=(0, 0.6, 0), R=rz(-80), spine=(15, -10, 0), legs=((-50, 5, 70), (-30, 10, 50)), arms=((-70, 30, 80), (-40, 70, 60)), head=(-10, 0)),
    "FeetHitL": fk(pelvis=(0, 0.7, 0), R=rx(-15), spine=(30, 0, 0), legs=((-75, 15, 30), (-80, 20, 60)), arms=((-20, 25, 40), (-30, 20, 30)), head=(-15, -10)),
    "FeetHitR": fk(pelvis=(0, 0.7, 0), R=rx(-15), spine=(30, 0, 0), legs=((-80, 20, 60), (-75, 15, 30)), arms=((-30, 20, 30), (-20, 25, 40)), head=(-15, 10)),
    "HeadHit": fk(pelvis=(0, 0.5, 0), R=rx(-85), spine=(-5, 0, 0), legs=((-15, 15, 20), (-25, 20, 35)), arms=((-110, 60, 20), (-100, 55, 30)), head=(10, 25)),
}
impact = keys(fk(pelvis=(0, 3.2, -0.3), **SPREAD))
for name, end in LYING.items():
    endk = keys(end)
    n = 53 if name.startswith("FeetHit") else 43
    rider[name] = [mix(impact, endk, ease(min(1.0, k / 20))) for k in range(n)]
    TIMING[name] = (15.0, False)

# Tricks (stunt.rs): each as long as the game's, at its frames a second,
# rising from the air stance to its peak by the hold frame (where a held
# button keeps it), holding a little and coming back by the end.
AIR = dict(STAND)
NUMS = ("pelvis", "pelvis_pitch", "lower_pitch", "upper_pitch", "head_pitch", "knee_out", "elbow_out",
        "torso_yaw", "head_yaw", "ankle_l", "ankle_r", "grip_l", "grip_r")
def full(P):
    """Every number a trick pose moves, filled in from the air stance."""
    Q = dict(SEATED); Q.update(AIR); Q.update(P)
    Q.setdefault("torso_yaw", 0.0); Q.setdefault("head_yaw", 0.0)
    for s_, sx in (("l", -1), ("r", 1)):
        Q.setdefault("ankle_" + s_, tuple(np.array(Q["ankle"]) * [sx, 1, 1]))
        Q.setdefault("grip_" + s_, tuple(np.array(Q["grip"]) * [sx, 1, 1]))
    return Q
def lerp_pose(a, b, t):
    out = dict(a)
    for k in NUMS:
        out[k] = tuple(np.array(a[k], float) * (1 - t) + np.array(b[k], float) * t) if isinstance(a[k], (tuple, list)) else a[k] * (1 - t) + b[k] * t
    return out
def trick(peak, frames, hold):
    a, b = full({}), full(peak)
    out = []
    for k in range(frames):
        if k <= hold: t = ease(k / max(hold, 1))
        elif k <= hold + 3: t = 1.0
        else: t = ease(max(0.0, 1 - (k - hold - 3) / max(frames - 1 - hold - 3, 1)))
        P = lerp_pose(a, b, t)
        out.append(keys(pose(None, torso_yaw=P["torso_yaw"], head_yaw=P["head_yaw"],
                             **{k: v for k, v in P.items() if k not in ("torso_yaw", "head_yaw", "roll")})))
    return out
OFF = (0.0, 0.0, 0.0)
SEAT_BACK = (0.0, 4.0, -2.6)  # the rear of the seat, for a grab
TRICKS = {  # name: (frames a second, frames, hold frame, peak)
    "BarKneel": (18, 30, 14, dict(pelvis=(0, 5.0, -0.2), pelvis_pitch=10, ankle_l=(-0.4, 4.1, -2.1), ankle_r=(0.4, 4.1, -2.1), knee_out=0.2)),
    "SplitX": (15, 32, 16, dict(pelvis=(0, 4.9, -0.6), pelvis_pitch=20, ankle_l=(-2.9, 3.4, -1.0), ankle_r=(2.9, 3.4, -1.0), knee_out=1.5)),
    "BKahuna": (15, 35, 17, dict(pelvis=(0.4, 5.2, -2.6), pelvis_pitch=70, upper_pitch=-10, head_pitch=20, ankle_l=(-0.6, 5.6, -5.6), ankle_r=(1.4, 5.0, -5.4))),
    "Cordova": (15, 36, 18, dict(pelvis=(0, 5.0, -1.4), pelvis_pitch=-35, upper_pitch=-20, head_pitch=25, ankle_l=(-0.4, 4.4, 2.4), ankle_r=(0.4, 4.4, 2.4), knee_out=0.3)),
    "HeelClicker": (15, 26, 11, dict(pelvis=(0, 4.5, -1.3), pelvis_pitch=-5, ankle_l=(-0.15, 4.5, 3.4), ankle_r=(0.15, 4.5, 3.4), knee_out=0.6)),
    "NacNac": (20, 31, 14, dict(pelvis=(0.6, 4.6, -0.9), pelvis_pitch=25, torso_yaw=-25, ankle_l=(2.2, 3.8, -2.6), ankle_r=(0.66, 1.9, -0.7))),
    "SuperMan": (15, 36, 17, dict(pelvis=(0, 5.0, -3.0), pelvis_pitch=78, upper_pitch=-12, head_pitch=25, ankle_l=(-0.5, 5.4, -6.2), ankle_r=(0.5, 5.4, -6.2), knee_out=0.2)),
    "AirWalk": (15, 37, 18, dict(pelvis=(0, 4.8, -1.8), pelvis_pitch=40, ankle_l=(-0.5, 2.6, -3.6), ankle_r=(0.5, 4.2, -4.0), knee_out=0.3)),
    "BarHop": (18, 32, 15, dict(pelvis=(0, 5.2, 0.4), pelvis_pitch=-10, ankle_l=(-0.5, 4.6, 2.9), ankle_r=(0.5, 4.6, 2.9), knee_out=0.8)),
    "SeatGrab": (15, 28, 17, dict(pelvis=(0, 4.9, -2.0), pelvis_pitch=55, grip_l=SEAT_BACK, ankle_l=(-0.8, 4.6, -5.0), ankle_r=(0.8, 4.4, -5.0))),
    "SaranWrap": (17, 31, 15, dict(pelvis=(0.3, 4.6, -0.8), pelvis_pitch=15, torso_yaw=15, ankle_l=(0.2, 4.7, 2.4), grip_l=(-1.6, 6.2, 0.4), knee_out=0.2)),
    "CliffHanger": (15, 36, 18, dict(pelvis=(0, 6.6, -0.6), pelvis_pitch=-10, upper_pitch=-8, head_pitch=-10, ankle_l=(-0.5, 3.6, 1.2), ankle_r=(0.5, 3.6, 1.2), grip_l=(-1.8, 9.0, -0.6), grip_r=(1.8, 9.0, -0.6))),
    "HeartAttack": (15, 33, 15, dict(pelvis=(0, 5.6, -1.0), pelvis_pitch=-45, upper_pitch=10, ankle_l=(-0.6, 8.4, -1.6), ankle_r=(0.6, 8.2, -1.2), grip_r=SEAT_BACK, knee_out=0.2)),
    "TailGrab": (15, 33, 15, dict(pelvis=(0, 4.6, -2.2), pelvis_pitch=50, torso_yaw=20, grip_r=(1.0, 3.4, -4.2), ankle_l=(-1.0, 3.8, -4.4), ankle_r=(0.9, 3.0, -4.6))),
    "LazyBoy": (16, 32, 17, dict(pelvis=(0, 4.0, -1.4), pelvis_pitch=-70, upper_pitch=-10, head_pitch=30, ankle_l=(-0.4, 4.6, 2.8), ankle_r=(0.4, 4.6, 2.8), grip_l=(-0.6, 5.4, -3.4), grip_r=(0.6, 5.4, -3.4))),
    "DCanCan": (18, 33, 16, dict(pelvis=(-0.3, 4.6, -0.9), pelvis_pitch=20, torso_yaw=10, ankle_l=(-2.6, 3.4, 0.6), ankle_r=(-2.0, 3.0, 0.9), knee_out=0.2)),
}
ALTS = {  # the Alt (held longer, a step further)
    "SuperManAlt": (15, 26, 13, "SuperMan", dict(grip_l=SEAT_BACK)),
    "DCanCanAlt": (15, 28, 13, "DCanCan", dict(grip_l=(-1.8, 6.4, 0.6))),
    "HeelClickerAlt": (15, 33, 15, "HeelClicker", dict(grip_l=(-1.8, 6.2, 0.2), grip_r=(1.8, 6.2, 0.2))),
    "NacNacAlt": (15, 24, 11, "NacNac", dict(grip_r=(1.9, 6.4, 0.0))),
    "SaranWrapAlt": (15, 19, 9, "SaranWrap", dict(grip_r=(1.8, 6.4, 0.2))),
    "BarKneelAlt": (15, 24, 1, "BarKneel", dict(grip_l=(-1.7, 6.6, 0.4), grip_r=(1.7, 6.6, 0.4))),
}
for name, (fps, frames, hold, peak) in TRICKS.items():
    rider[name] = trick(peak, frames, hold); TIMING[name] = (float(fps), False)
for name, (fps, frames, hold, base, more) in ALTS.items():
    rider[name] = trick(dict(TRICKS[base][3], **more), frames, max(hold, 4)); TIMING[name] = (float(fps), False)
TRICK_NAMES = list(TRICKS) + list(ALTS)

def pack(tables):
    """The motions as the engine reads them (freepack::own_motions):
    "FPMO", then per motion its name, frames a second, looping, rider or
    bike, frame and key counts, and per key its frame index, look and up
    (× 30000) and position (× 1000) as little-endian i16."""
    import struct
    out = bytearray(b"FPMO")
    items = [(n, f, True) for n, f in tables[0].items()] + [(n, f, False) for n, f in tables[1].items()]
    out += struct.pack("<H", len(items))
    for name, frames, is_rider in items:
        fps, looping = TIMING.get(name, (1.0, True))
        b = name.encode()
        out += struct.pack("<B", len(b)) + b + struct.pack("<fBBHB", fps, looping, is_rider, len(frames), len(frames[0]))
        for f in frames:
            assert len(f) == len(frames[0])
            for i, l, u, t in f:
                q = [round(v * 30000) for v in list(l) + list(u)] + [round(v * 1000) for v in t]
                out += struct.pack("<B9h", i, *q)
    return bytes(out)

if __name__ == '__main__':
    open(sys.argv[1], 'wb').write(pack((rider, bike)))
    print("rider", len(rider), "bike", len(bike), "bytes", os.path.getsize(sys.argv[1]))
