"""Works out the free rider's own seated riding pose (freepack.rs SEATED) by two-bone IK from the free bikes' seat, pegs and grips.
Outputs per-node local (look, up, position) keys, relative to the parent, like the engine's motion keys."""
import numpy as np, json, math, sys
REST = {  # node: (parent, local translation) from the rider frame tree
 'Pelvis': (None, (0, 3.177, -0.290)), 'LwrTorso': ('Pelvis', (0, 0.178, -0.147)), 'UprLegL': ('Pelvis', (-0.276, 0, 0)), 'UprLegR': ('Pelvis', (0.276, 0, 0)),
 'UprTorso': ('LwrTorso', (0, 0.621, 0)), 'Head': ('UprTorso', (0, 1.217, 0)), 'UprArmL': ('UprTorso', (-0.650, 0.925, 0.062)), 'UprArmR': ('UprTorso', (0.650, 0.925, 0.062)),
 'LwrArmL': ('UprArmL', (-0.191, -1.089, -0.010)), 'HandL': ('LwrArmL', (-0.151, -0.862, 0.067)), 'LwrArmR': ('UprArmR', (0.191, -1.089, -0.010)), 'HandR': ('LwrArmR', (0.151, -0.862, 0.067)),
 'LwrLegL': ('UprLegL', (0.001, -1.470, 0.133)), 'FootL': ('LwrLegL', (0.001, -1.377, -0.143)), 'LwrLegR': ('UprLegR', (-0.001, -1.470, 0.133)), 'FootR': ('LwrLegR', (-0.001, -1.377, -0.143))}
ORDER = ['Pelvis', 'LwrTorso', 'UprLegL', 'UprLegR', 'UprTorso', 'Head', 'UprArmL', 'UprArmR', 'LwrArmL', 'HandL', 'LwrArmR', 'HandR', 'LwrLegL', 'FootL', 'LwrLegR', 'FootR']
def rx(d): a = math.radians(d); c, s = math.cos(a), math.sin(a); return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
def ry(d): a = math.radians(d); c, s = math.cos(a), math.sin(a); return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
def rz(d): a = math.radians(d); c, s = math.cos(a), math.sin(a); return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])
def nrm(v): return v / np.linalg.norm(v)
def frame_from(d, ref):
    """orthonormal basis with first axis d and second as close to ref as possible"""
    a = nrm(d); b = nrm(ref - a * (ref @ a)); c = np.cross(a, b); return np.stack([a, b, c], 1)
def align(rest_d, rest_ref, new_d, new_ref):
    return frame_from(new_d, new_ref) @ frame_from(rest_d, rest_ref).T
def two_bone(A, T, L1, L2, hint):
    d = T - A; D = min(np.linalg.norm(d), L1 + L2 - 1e-4); dn = nrm(d)
    a = (L1 * L1 - L2 * L2 + D * D) / (2 * D); h = math.sqrt(max(0, L1 * L1 - a * a))
    hv = nrm(hint - dn * (hint @ dn))
    return A + dn * a + hv * h, A + dn * D

def solve(P):
    """P: dict of pose parameters. Returns world rotations/positions per node."""
    W = {}  # node -> (R, t) world
    rest_world = {}
    for n in ORDER:
        p, t = REST[n]; t = np.array(t, float)
        rest_world[n] = t if p is None else rest_world[p] + t
    Rp = rx(P['pelvis_pitch']); W['Pelvis'] = (Rp, np.array(P['pelvis']))
    Rl = Rp @ rx(P['lower_pitch']); W['LwrTorso'] = (Rl, W['Pelvis'][1] + Rp @ np.array(REST['LwrTorso'][1]))
    Ru = Rl @ rx(P['upper_pitch']); W['UprTorso'] = (Ru, W['LwrTorso'][1] + Rl @ np.array(REST['UprTorso'][1]))
    Rh = rx(P['head_pitch']); W['Head'] = (Rh, W['UprTorso'][1] + Ru @ np.array(REST['Head'][1]))
    for s, sx in (('L', -1), ('R', 1)):
        # legs
        hip = W['Pelvis'][1] + Rp @ np.array(REST['UprLeg' + s][1])
        L1 = np.linalg.norm(REST['LwrLeg' + s][1]); L2 = np.linalg.norm(REST['Foot' + s][1])
        ank = np.array(P['ankle']) * np.array([sx, 1, 1])
        knee, ank = two_bone(hip, ank, L1, L2, np.array([sx * P['knee_out'], 0.2, 1.0]))
        rd_th = np.array(REST['LwrLeg' + s][1]); rd_sh = np.array(REST['Foot' + s][1])
        bend = nrm(np.cross(knee - hip, ank - knee)); 
        if bend[0] * 1 < 0: bend = -bend       # hinge axis roughly +x
        Rth = align(rd_th, np.array([1.0, 0, 0]), knee - hip, bend)
        Rsh = align(rd_sh, np.array([1.0, 0, 0]), ank - knee, bend)
        W['UprLeg' + s] = (Rth, hip); W['LwrLeg' + s] = (Rsh, knee)
        W['Foot' + s] = (ry(-sx * P['foot_yaw']) @ rx(P['foot_pitch']), ank)
        # arms
        sh = W['UprTorso'][1] + Ru @ np.array(REST['UprArm' + s][1])
        A1 = np.linalg.norm(REST['LwrArm' + s][1]); A2 = np.linalg.norm(REST['Hand' + s][1])
        grip = np.array(P['grip']) * np.array([sx, 1, 1])
        wrist_t = grip + np.array(P['wrist_from_grip']) * np.array([sx, 1, 1])
        el, wr = two_bone(sh, wrist_t, A1, A2, np.array([sx * P['elbow_out'], -0.3, -0.6]))
        rd_u = np.array(REST['LwrArm' + s][1]); rd_f = np.array(REST['Hand' + s][1])
        bend = nrm(np.cross(el - sh, wr - el))
        if bend[0] * sx > 0: bend = -bend
        Rua = align(rd_u, np.array([-sx * 1.0, 0, 0.0]) , el - sh, bend)
        Rfa = align(rd_f, np.array([-sx * 1.0, 0, 0.0]), wr - el, bend)
        W['UprArm' + s] = (Rua, sh); W['LwrArm' + s] = (Rfa, el)
        # hand: rest hangs along -y with palm facing in (-sx x). Point it at the grip, palm toward the bar.
        hd = grip - wr
        Rhd = align(np.array([0, -1.0, 0]), np.array([0, 0, 1.0]), hd, np.array([0, 0.2, 1.0]) - nrm(hd) * 0)
        W['Hand' + s] = (Rhd @ ry(sx * P['hand_roll']), wr)
    return W

def locals_(W):
    out = {}
    for n in ORDER:
        R, t = W[n]; p = REST[n][0]
        if p is None: Rl, tl = R, t
        else:
            Rp, tp = W[p]; Rl = Rp.T @ R; tl = Rp.T @ (t - tp)
        out[n] = dict(look=Rl[:, 2].round(5).tolist(), up=Rl[:, 1].round(5).tolist(), position=tl.round(5).tolist())
    return out

# Fitted to the free bikes as they are now (Ridgeback and Kestrel), which
# are the original bikes' size: their peg tips sit at x 0.64-0.67, y 1.35
# (the original's 0.68, 1.45) and their shrouds are no wider than the
# original's (body half-width 0.69 against 0.81). So the boots sit where
# the original's do over the pegs and the knees stay tucked in against the
# bike like the original's (x about 0.6). The older OpenMX bikes were wider
# and had the boots out at 0.78 and the knees pushed out to 0.95, which made
# the rider ride like a frog. `knee_clear` is the least knee x (game units)
# any pose keeps. `grip` is the Ridgeback's grip (the Kestrel's bars are
# raised to the same place).
SEATED = dict(pelvis=(0, 3.62, -1.02), pelvis_pitch=14, lower_pitch=12, upper_pitch=16, head_pitch=-6,
              ankle=(0.60, 1.75, -0.75), knee_out=0.8, knee_clear=0.60, foot_yaw=6, foot_pitch=-8,
              grip=(0.99, 4.10, 0.58), wrist_from_grip=(0.0, 0.10, -0.30), elbow_out=1.0, hand_roll=0)
if __name__ == '__main__':
    W = solve(SEATED); L = locals_(W)
    json.dump(L, open(sys.argv[1] if len(sys.argv) > 1 else '/home/user/work/pose/seated.json', 'w'), indent=1)
    for n in ('HandR', 'FootR', 'LwrLegR', 'LwrArmR'): print(n, W[n][1].round(2))
