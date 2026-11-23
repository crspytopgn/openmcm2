//! Smooth joints for the free pack's riders.
//!
//! A rider is 16 stiff parts, one per frame of the original's rider, so
//! the game's motions can drive it. The free riders were fitted to the
//! frames from one smooth model, sitting on the bike in the free riding
//! pose (`Turn2D`'s first frame, [`BIND_MOTION`]): there every part meets
//! its neighbours. Any other pose bends the joints, and the stiff parts
//! open gaps or sink into each other at the elbows, knees, shoulders and
//! hips.
//!
//! Here each vertex near a joint is shared between the two parts either
//! side of it, by how far past the joint it lies, and is moved by a blend
//! of both (linear blend skinning, measured in the fitting pose). The
//! blended place is put back into its own part's space, so the vertex
//! shaders, shadows, ray tracing and the motion buffer all draw it as they
//! draw any moving part: nothing on the graphics card changes, and it
//! works the same on every backend, the browser and Low settings.
//!
//! The weights come from the shape alone, so any rider the pack fits this
//! way bends; one that doesn't meet at its joints in the fitting pose (a
//! model made another way) is left stiff, as before.

use glam::{Mat4, Vec3};
use std::collections::HashMap;

/// The pack's own motion whose first frame is the pose the riders were
/// fitted in (`engine/freepack/blender/rider4/fit.py`): the seated pose as
/// it was then (Turn2D's first frame), kept apart since the seated pose
/// moved (`tools/freepack/fit_pose.json`).
pub const BIND_MOTION: &str = "FitPose";

/// Each joint's half-width of blend, ft, by the frame below it: the skin
/// within this of the joint's plane is shared. Wide for the hips and the
/// back, narrow for the stiff boots, the gloves' cuffs and the helmet.
const BLEND: [(&str, f32); 15] = [
    ("LwrTorso", 0.35),
    ("UprTorso", 0.3),
    ("Head", 0.1),
    ("UprArmL", 0.28),
    ("UprArmR", 0.28),
    ("LwrArmL", 0.22),
    ("LwrArmR", 0.22),
    ("HandL", 0.1),
    ("HandR", 0.1),
    ("UprLegL", 0.32),
    ("UprLegR", 0.32),
    ("LwrLegL", 0.26),
    ("LwrLegR", 0.26),
    ("FootL", 0.1),
    ("FootR", 0.1),
];

/// Each frame's main child, the way its bone runs: a joint's plane halves
/// the angle between the bone above and the bone below.
const MAIN_CHILD: [(&str, &str); 7] = [
    ("Pelvis", "LwrTorso"),
    ("LwrTorso", "UprTorso"),
    ("UprTorso", "Head"),
    ("UprArmL", "LwrArmL"),
    ("UprArmR", "LwrArmR"),
    ("LwrArmL", "HandL"),
    ("LwrArmR", "HandR"),
];
const MAIN_CHILD_LEGS: [(&str, &str); 4] = [
    ("UprLegL", "LwrLegL"),
    ("UprLegR", "LwrLegR"),
    ("LwrLegL", "FootL"),
    ("LwrLegR", "FootR"),
];

/// Two vertices this close in the fitting pose, ft, are the same point of
/// the skin (a seam between two parts).
const SAME: f32 = 0.004;

/// One vertex that bends: which of the model's vertices, its own frame,
/// up to three frames with their weights, and where it is (and faces) in
/// the fitting pose.
#[derive(Clone, Debug)]
struct Bend {
    vertex: u32,
    owner: u8,
    frames: [u8; 3],
    weights: [f32; 3],
    at: Vec3,
    normal: Vec3,
}

/// A rider's smooth joints.
#[derive(Debug)]
pub struct Skin {
    /// Each frame's world transform in the fitting pose, inverted.
    inverse_bind: Vec<Mat4>,
    /// Each frame's world transform at rest (where the stored vertices
    /// sit).
    rest: Vec<Mat4>,
    bends: Vec<Bend>,
    /// The runs of vertices that bend, to upload (vertex indices from the
    /// model's first, end exclusive).
    pub spans: Vec<std::ops::Range<u32>>,
}

/// Each frame's world transform from local ones and parents.
fn worlds(local: &[Mat4], parents: &[Option<usize>]) -> Vec<Mat4> {
    let mut out: Vec<Option<Mat4>> = vec![None; local.len()];
    fn resolve(
        i: usize,
        local: &[Mat4],
        parents: &[Option<usize>],
        out: &mut [Option<Mat4>],
        depth: usize,
    ) -> Mat4 {
        if let Some(m) = out[i] {
            return m;
        }
        let m = match parents[i] {
            Some(p) if p < local.len() && depth < local.len() => {
                resolve(p, local, parents, out, depth + 1) * local[i]
            }
            _ => local[i],
        };
        out[i] = Some(m);
        m
    }
    for i in 0..local.len() {
        resolve(i, local, parents, &mut out, 0);
    }
    out.into_iter()
        .map(|m| m.unwrap_or(Mat4::IDENTITY))
        .collect()
}

/// Points by a grid of [`SAME`] cells, to find the ones at the same place.
struct Grid(HashMap<(i32, i32, i32), Vec<u32>>);

impl Grid {
    fn cell(p: Vec3) -> (i32, i32, i32) {
        let q = |x: f32| (x / SAME).floor() as i32;
        (q(p.x), q(p.y), q(p.z))
    }
    fn new(points: &[Vec3]) -> Grid {
        let mut map: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            map.entry(Self::cell(*p)).or_default().push(i as u32);
        }
        Grid(map)
    }
    /// Every point within [`SAME`] of `p`.
    fn near<'a>(&'a self, points: &'a [Vec3], p: Vec3) -> impl Iterator<Item = u32> + 'a {
        let (x, y, z) = Self::cell(p);
        (-1..=1)
            .flat_map(move |dx| {
                (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| (x + dx, y + dy, z + dz)))
            })
            .filter_map(|c| self.0.get(&c))
            .flatten()
            .copied()
            .filter(move |&j| points[j as usize].distance_squared(p) <= SAME * SAME)
    }
}

impl Skin {
    /// The smooth joints of a rider with these frames (rest transforms
    /// relative to their parents, names) and vertices (rest positions and
    /// normals in model space, each one's frame), fitted in `bind` (each
    /// frame's local transform in the fitting pose; `None` keeps its rest
    /// one). `None` if it doesn't meet at its joints in that pose.
    pub fn new(
        frames: &[(Mat4, Option<usize>)],
        names: &[String],
        positions: &[Vec3],
        normals: &[Vec3],
        owners: &[usize],
        bind: &[Option<Mat4>],
    ) -> Option<Skin> {
        let count = frames.len();
        if count == 0 || count > 255 || positions.is_empty() {
            return None;
        }
        let parents: Vec<Option<usize>> = frames.iter().map(|f| f.1).collect();
        let rest = worlds(&frames.iter().map(|f| f.0).collect::<Vec<_>>(), &parents);
        let local: Vec<Mat4> = (0..count)
            .map(|i| bind.get(i).copied().flatten().unwrap_or(frames[i].0))
            .collect();
        let posed = worlds(&local, &parents);
        // Every vertex in the fitting pose.
        let to_bind: Vec<Mat4> = (0..count).map(|i| posed[i] * rest[i].inverse()).collect();
        let at: Vec<Vec3> = positions
            .iter()
            .zip(owners)
            .map(|(p, &o)| to_bind[o].transform_point3(*p))
            .collect();
        let mut facing: Vec<Vec3> = normals
            .iter()
            .zip(owners)
            .map(|(n, &o)| to_bind[o].transform_vector3(*n).normalize_or_zero())
            .collect();
        let grid = Grid::new(&at);
        let named = |name: &str| names.iter().position(|n| n.eq_ignore_ascii_case(name));
        let pivot = |i: usize| posed[i].w_axis.truncate();

        // Each joint (the frame below it): its parent, pivot, the plane's
        // normal (pointing into the part below), half-width, and how far
        // out from the pivot its seam reaches.
        struct Joint {
            child: usize,
            parent: usize,
            pivot: Vec3,
            axis: Vec3,
            half: f32,
            reach: f32,
        }
        let main_child = |f: usize| -> Option<usize> {
            let name = &names[f];
            MAIN_CHILD
                .iter()
                .chain(&MAIN_CHILD_LEGS)
                .find(|(p, _)| p.eq_ignore_ascii_case(name))
                .and_then(|(_, c)| named(c))
        };
        let centroid = |f: usize| -> Option<Vec3> {
            let (sum, n) = at
                .iter()
                .zip(owners)
                .filter(|(_, &o)| o == f)
                .fold((Vec3::ZERO, 0usize), |(s, n), (p, _)| (s + *p, n + 1));
            (n > 0).then(|| sum / n as f32)
        };
        let mut joints: Vec<Joint> = Vec::new();
        let mut met = 0usize;
        for &(name, half) in &BLEND {
            let Some(child) = named(name) else { continue };
            let Some(parent) = parents[child] else {
                continue;
            };
            let j = pivot(child);
            // Down the bone below: to its own main child, or its middle.
            let below = main_child(child)
                .map(pivot)
                .or_else(|| centroid(child))
                .map(|p| p - j)
                .unwrap_or(Vec3::ZERO);
            let below = below.normalize_or_zero();
            // Along the bone above, if this is the way it runs.
            let above = if main_child(parent) == Some(child) {
                (j - pivot(parent)).normalize_or_zero()
            } else {
                Vec3::ZERO
            };
            let axis = (below + above).normalize_or(below);
            if axis == Vec3::ZERO {
                continue;
            }
            // The seam: the parts' vertices that meet the other's.
            let mut reach: f32 = 0.0;
            for (v, &o) in owners.iter().enumerate() {
                if o != child {
                    continue;
                }
                if grid.near(&at, at[v]).any(|k| owners[k as usize] == parent) {
                    met += 1;
                    reach = reach.max(at[v].distance(j));
                }
            }
            if reach > 0.0 {
                joints.push(Joint {
                    child,
                    parent,
                    pivot: j,
                    axis,
                    half,
                    reach,
                });
            }
        }
        // Fitted another way: the parts don't meet, so stay stiff.
        if joints.len() < 8 || met * 4 < joints.len() {
            return None;
        }

        // Each vertex's shares: its own frame's, passed on to the parts
        // either side of the joints at its ends.
        let smooth = |x: f32| {
            let t = (x * 0.5 + 0.5).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let mut shares: Vec<Vec<(usize, f32)>> = Vec::with_capacity(at.len());
        for (v, &o) in owners.iter().enumerate() {
            let p = at[v];
            let mut w: Vec<(usize, f32)> = vec![(o, 1.0)];
            let give = |to: usize, amount: f32, w: &mut Vec<(usize, f32)>| {
                if amount <= 1e-4 {
                    return;
                }
                w[0].1 -= amount;
                match w.iter_mut().find(|(f, _)| *f == to) {
                    Some(e) => e.1 += amount,
                    None => w.push((to, amount)),
                }
            };
            for jt in &joints {
                if jt.child != o && jt.parent != o {
                    continue;
                }
                // Fades out past the seam's reach, so a joint only touches
                // the skin around it.
                let d = p.distance(jt.pivot);
                let near =
                    1.0 - smooth(((d - jt.reach * 1.15) / (jt.reach * 0.35).max(1e-3)) * 2.0 - 1.0);
                if near <= 0.0 {
                    continue;
                }
                let t = smooth((p - jt.pivot).dot(jt.axis) / jt.half);
                if jt.child == o {
                    give(jt.parent, (1.0 - t) * near, &mut w);
                } else {
                    give(jt.child, t * near, &mut w);
                }
            }
            if w[0].1 < 0.0 {
                let total: f32 = w.iter().skip(1).map(|e| e.1).sum();
                let keep = 1.0 / total.max(1e-6);
                w[0].1 = 0.0;
                w.iter_mut().skip(1).for_each(|e| e.1 *= keep);
            }
            shares.push(w);
        }
        // The copies of a seam point share one blend, so the skin stays
        // closed.
        let mut done = vec![false; at.len()];
        for v in 0..at.len() {
            if done[v] {
                continue;
            }
            let same: Vec<u32> = grid.near(&at, at[v]).collect();
            if same.len() < 2 {
                done[v] = true;
                continue;
            }
            let mut sum: Vec<(usize, f32)> = Vec::new();
            let mut n = Vec3::ZERO;
            for &k in &same {
                for &(f, x) in &shares[k as usize] {
                    match sum.iter_mut().find(|e| e.0 == f) {
                        Some(e) => e.1 += x,
                        None => sum.push((f, x)),
                    }
                }
                n += facing[k as usize];
            }
            let scale = 1.0 / same.len() as f32;
            sum.iter_mut().for_each(|e| e.1 *= scale);
            let n = n.normalize_or(facing[v]);
            for &k in &same {
                let k = k as usize;
                shares[k] = sum.clone();
                done[k] = true;
                // Only copies that face much the same way share a normal
                // (a hard edge stays hard).
                if facing[k].dot(n) > 0.8 {
                    facing[k] = n;
                }
            }
        }

        let mut bends = Vec::new();
        for (v, w) in shares.iter_mut().enumerate() {
            let o = owners[v];
            w.retain(|e| e.1 > 0.01);
            if w.len() <= 1 && w.first().is_none_or(|e| e.0 == o) {
                continue;
            }
            w.sort_by(|a, b| b.1.total_cmp(&a.1));
            w.truncate(3);
            let total: f32 = w.iter().map(|e| e.1).sum();
            let mut frames = [o as u8; 3];
            let mut weights = [0.0; 3];
            for (k, e) in w.iter().enumerate() {
                frames[k] = e.0 as u8;
                weights[k] = e.1 / total;
            }
            bends.push(Bend {
                vertex: v as u32,
                owner: o as u8,
                frames,
                weights,
                at: at[v],
                normal: facing[v],
            });
        }
        if bends.is_empty() {
            return None;
        }
        // Runs to upload: close bends share one.
        let mut spans: Vec<std::ops::Range<u32>> = Vec::new();
        for b in &bends {
            match spans.last_mut() {
                Some(s) if b.vertex < s.end + 64 => s.end = b.vertex + 1,
                _ => spans.push(b.vertex..b.vertex + 1),
            }
        }
        Some(Skin {
            inverse_bind: posed.iter().map(Mat4::inverse).collect(),
            rest,
            bends,
            spans,
        })
    }

    /// How many vertices bend.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.bends.len()
    }

    /// The vertices that bend (indices from the model's first).
    pub fn vertices(&self) -> impl Iterator<Item = u32> + '_ {
        self.bends.iter().map(|b| b.vertex)
    }

    /// Moves the bending vertices for frames posed at `world` (each frame's
    /// world transform, as drawn), calling `set` with each one's index,
    /// position and normal in its own frame's rest space (where the node
    /// transform the shader applies takes it to the blended place).
    pub fn apply(&self, world: &[Mat4], mut set: impl FnMut(u32, Vec3, Vec3)) {
        let count = self.rest.len().min(world.len());
        // From the fitting pose to now, per frame, and from now back into
        // each frame's rest space.
        let to_now: Vec<Mat4> = (0..count)
            .map(|i| world[i] * self.inverse_bind[i])
            .collect();
        let back: Vec<Mat4> = (0..count)
            .map(|i| self.rest[i] * world[i].inverse())
            .collect();
        // Each (own frame, other frame) pair's transform, made when first
        // needed.
        let mut pair: Vec<Option<Mat4>> = vec![None; count * count];
        for b in &self.bends {
            let o = b.owner as usize;
            if o >= count {
                continue;
            }
            let mut m = Mat4::ZERO;
            for k in 0..3 {
                let (f, w) = (b.frames[k], b.weights[k]);
                if w <= 0.0 || f as usize >= count {
                    continue;
                }
                let c = *pair[o * count + f as usize]
                    .get_or_insert_with(|| back[o] * to_now[f as usize]);
                m += c * w;
            }
            set(
                b.vertex,
                m.transform_point3(b.at),
                m.transform_vector3(b.normal).normalize_or(b.normal),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arm of two parts meeting at an elbow, bent 90° in the fitting
    /// pose: straightened, the skin by the elbow follows both parts and the
    /// two copies of the seam stay together.
    #[test]
    fn an_elbow_bends_smoothly_and_stays_closed() {
        let names: Vec<String> = [
            "Pelvis", "LwrTorso", "UprTorso", "UprArmL", "LwrArmL", "HandL", "UprArmR", "LwrArmR",
            "HandR", "Head",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let t = |x: f32, y: f32, z: f32| Mat4::from_translation(Vec3::new(x, y, z));
        // Straight down the y axis at rest, each part a foot long.
        let frames: Vec<(Mat4, Option<usize>)> = vec![
            (t(0.0, 0.0, 0.0), None),
            (t(0.0, 1.0, 0.0), Some(0)),
            (t(0.0, 1.0, 0.0), Some(1)),
            (t(1.0, 0.0, 0.0), Some(2)),
            (t(0.0, -1.0, 0.0), Some(3)),
            (t(0.0, -1.0, 0.0), Some(4)),
            (t(-1.0, 0.0, 0.0), Some(2)),
            (t(0.0, -1.0, 0.0), Some(6)),
            (t(0.0, -1.0, 0.0), Some(7)),
            (t(0.0, 1.0, 0.0), Some(2)),
        ];
        // Fitted with each elbow bent 90° (the forearm turned about z).
        let mut bind: Vec<Option<Mat4>> = vec![None; frames.len()];
        bind[4] = Some(t(0.0, -1.0, 0.0) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2));
        bind[7] = Some(t(0.0, -1.0, 0.0) * Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2));
        let parents: Vec<Option<usize>> = frames.iter().map(|f| f.1).collect();
        let rest = worlds(&frames.iter().map(|f| f.0).collect::<Vec<_>>(), &parents);
        let posed = worlds(
            &(0..frames.len())
                .map(|i| bind[i].unwrap_or(frames[i].0))
                .collect::<Vec<_>>(),
            &parents,
        );
        // A tube of points along every part (in the fitting pose), split
        // at each joint, with the seam copied onto both sides.
        let (mut positions, mut owners, mut pairs) = (Vec::new(), Vec::new(), Vec::new());
        for f in 1..frames.len() {
            let p = parents[f].unwrap();
            let (a, b) = (posed[p].w_axis.truncate(), posed[f].w_axis.truncate());
            for k in 0..=10 {
                let x = a + (b - a) * (k as f32 / 10.0);
                for ring in 0..6 {
                    let ang = ring as f32 * std::f32::consts::TAU / 6.0;
                    let off = Vec3::new(ang.cos(), 0.0, ang.sin()) * 0.15;
                    let at = x + off;
                    // The point belongs to the parent part's bone: stored
                    // in the parent's rest space.
                    let owner = p;
                    positions.push((rest[owner] * posed[owner].inverse()).transform_point3(at));
                    owners.push(owner);
                    if k == 10 {
                        // The seam copy on the child's side.
                        positions.push((rest[f] * posed[f].inverse()).transform_point3(at));
                        owners.push(f);
                        pairs.push((positions.len() - 2, positions.len() - 1));
                    }
                }
            }
        }
        let normals = vec![Vec3::Y; positions.len()];
        let skin = Skin::new(&frames, &names, &positions, &normals, &owners, &bind).expect("fits");
        assert!(skin.len() > 0);
        // In the fitting pose nothing moves.
        let mut moved = 0.0f32;
        skin.apply(&posed, |v, p, _| {
            moved = moved.max(p.distance(positions[v as usize]))
        });
        assert!(moved < 1e-3, "moved {moved}");
        // Straightened at rest: the seam copies stay together.
        let mut placed = positions.clone();
        skin.apply(&rest, |v, p, _| placed[v as usize] = p);
        assert!(!pairs.is_empty());
        for &(a, b) in &pairs {
            assert!(
                placed[a].distance(placed[b]) < 1e-3,
                "{} {}",
                placed[a],
                placed[b]
            );
        }
        // And the elbow's skin moved: the seam points sit half way round.
        let elbow: Vec<&(usize, usize)> = pairs.iter().filter(|p| owners[p.1] == 4).collect();
        assert!(elbow
            .iter()
            .any(|p| placed[p.0].distance(positions[p.0]) > 0.01));
    }
}
