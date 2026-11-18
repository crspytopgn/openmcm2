//! `mm2-tools freetracks <game dir> [ground dir] [stockmap.toml]`: the free
//! pack's tracks, every event type, made from scratch. Nothing in them
//! comes from the original game: the layouts, terrain, jumps and scenery
//! are generated here from each track's seed and style.
//!
//! Each track is an `.ENV` written to `<game dir>/TERAFORM/<event>/`:
//! a terrain (`.trn`), a scene (`.scn`), and as the event needs them a
//! course (`.tdf`), Baja and enduro gates (`.wpt`) and an eco-system
//! (`.esb`). Models, plants and skies are named as the free pack's
//! `stockmap.toml` names them, so the pack draws them; with the original
//! game's files the original's models of those names are drawn.
//!
//! Ground textures come from `<ground dir>/<material>.png` (512 px,
//! seamless) when given, or are painted from flat colours and noise.

use mm2_formats::{rs2, trn};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    National,
    Supercross,
    Baja,
    Enduro,
    Quarry,
    Tag,
}

impl Kind {
    pub fn folder(self) -> &'static str {
        match self {
            Kind::National => "NATIONAL",
            Kind::Supercross => "SX",
            Kind::Baja => "BAJA",
            Kind::Enduro => "ENDURO",
            Kind::Quarry => "QUARRIES",
            Kind::Tag => "TAG",
        }
    }

    fn event(self) -> &'static str {
        match self {
            Kind::National => "National",
            Kind::Supercross => "Supercross",
            Kind::Baja => "Baja",
            Kind::Enduro => "Enduro",
            Kind::Quarry => "Quarry",
            Kind::Tag => "Tag",
        }
    }

    /// Samples per side and feet per unit, as the shipped tracks of the
    /// event have them.
    fn size(self) -> (usize, f32) {
        match self {
            Kind::National => (1281, 3.0),
            Kind::Supercross | Kind::Tag => (769, 3.0),
            Kind::Baja | Kind::Enduro | Kind::Quarry => (1537, 4.5),
        }
    }

    /// Tile texture size, px.
    fn texture(self) -> usize {
        256
    }
}

/// The look of a track: its ground, plants, sky and light.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Meadow,
    DryHills,
    Desert,
    Forest,
    Autumn,
    Alpine,
    Pit,
    Arena,
}

/// The ground materials, by index; each one's file name in the ground
/// folder, its flat colour, and how many feet one repeat of its picture
/// covers.
const MATERIALS: [(&str, [f32; 3], f32); 11] = [
    ("trackdirt", [0.42, 0.33, 0.24], 40.0),
    ("loosedirt", [0.50, 0.40, 0.29], 40.0),
    ("mud", [0.30, 0.24, 0.18], 40.0),
    ("grass", [0.31, 0.38, 0.20], 48.0),
    ("drygrass", [0.55, 0.50, 0.33], 48.0),
    ("desertsand", [0.70, 0.58, 0.42], 64.0),
    ("gravel", [0.52, 0.50, 0.46], 40.0),
    ("rock", [0.48, 0.45, 0.42], 64.0),
    ("forestfloor", [0.27, 0.24, 0.17], 40.0),
    ("sxclay", [0.36, 0.27, 0.20], 32.0),
    ("concrete", [0.55, 0.55, 0.53], 32.0),
];
const TRACKDIRT: usize = 0;
const LOOSE: usize = 1;
const MUD: usize = 2;
const GRASS: usize = 3;
const DRYGRASS: usize = 4;
const SAND: usize = 5;
const GRAVEL: usize = 6;
const ROCK: usize = 7;
const FOREST: usize = 8;
const CLAY: usize = 9;
const CONCRETE: usize = 10;

pub struct Def {
    pub file: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub biome: Biome,
    pub sky: Option<&'static str>,
    pub seed: u32,
}

const fn def(file: &'static str, name: &'static str, kind: Kind, biome: Biome, sky: Option<&'static str>, seed: u32) -> Def {
    Def { file, name, kind, biome, sky, seed }
}

/// The free tracks. File names start `FP` so they sort apart from any
/// other tracks in the same folder.
pub const SET: &[Def] = &[
    def("FPNAT01", "Copper Ridge", Kind::National, Biome::Meadow, Some("terra18.cub"), 11),
    def("FPNAT02", "Hollow Creek", Kind::National, Biome::Forest, Some("wind01.cub"), 12),
    def("FPNAT03", "Red Mesa Raceway", Kind::National, Biome::DryHills, Some("terra09.cub"), 13),
    def("FPNAT04", "Maple Bend", Kind::National, Biome::Autumn, Some("terra03.cub"), 14),
    def("FPNAT05", "Granite Flats", Kind::National, Biome::Alpine, Some("terra01.cub"), 15),
    def("FPNAT06", "Sundown Valley", Kind::National, Biome::DryHills, Some("terra16.cub"), 16),
    def("FPNAT07", "Willow Run", Kind::National, Biome::Meadow, Some("wind01.cub"), 17),
    def("FPSX01", "Harbor City Arena", Kind::Supercross, Biome::Arena, None, 21),
    def("FPSX02", "Ironworks Dome", Kind::Supercross, Biome::Arena, None, 22),
    def("FPSX03", "Lakeside Coliseum", Kind::Supercross, Biome::Arena, None, 23),
    def("FPSX04", "Northgate Stadium", Kind::Supercross, Biome::Arena, None, 24),
    def("FPSX05", "Riverside Bowl", Kind::Supercross, Biome::Arena, None, 25),
    def("FPSX06", "Summit Center", Kind::Supercross, Biome::Arena, None, 26),
    def("FPBAJA01", "Saltbush Basin", Kind::Baja, Biome::Desert, Some("terra12.cub"), 31),
    def("FPBAJA02", "Dry Lake Crossing", Kind::Baja, Biome::Desert, Some("terra06.cub"), 32),
    def("FPBAJA03", "Coyote Wash", Kind::Baja, Biome::DryHills, Some("terra11.cub"), 33),
    def("FPBAJA04", "Sandstone Canyon", Kind::Baja, Biome::Desert, Some("terra09.cub"), 34),
    def("FPEND01", "Timberline Trails", Kind::Enduro, Biome::Forest, Some("terra03.cub"), 41),
    def("FPEND02", "Foxglove Woods", Kind::Enduro, Biome::Autumn, Some("terra01.cub"), 42),
    def("FPEND03", "Cedar Hollow", Kind::Enduro, Biome::Forest, Some("terra01.cub"), 43),
    def("FPEND04", "High Pass", Kind::Enduro, Biome::Alpine, Some("terra18.cub"), 44),
    def("FPQUAR01", "Blackrock Pit", Kind::Quarry, Biome::Pit, Some("terra06.cub"), 51),
    def("FPQUAR02", "Limestone Bowl", Kind::Quarry, Biome::Pit, Some("terra12.cub"), 52),
    def("FPQUAR03", "Gravel Works", Kind::Quarry, Biome::Pit, Some("terra17.cub"), 53),
    def("FPQUAR04", "Slate Hollow", Kind::Quarry, Biome::Pit, Some("burn03.cub"), 54),
    def("FPTAG01", "Tag Arena One", Kind::Tag, Biome::Arena, None, 61),
    def("FPTAG02", "Tag Arena Two", Kind::Tag, Biome::Arena, None, 62),
    def("FPTAG03", "Tag Arena Three", Kind::Tag, Biome::Arena, None, 63),
];

// ---------------------------------------------------------------------
// Small helpers.

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A random number from 0 to 1, from a seed and a counter.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mut h = self.0;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb_352d);
        h ^= h >> 15;
        (h & 0xff_ffff) as f32 / 16_777_216.0
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }

    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[((self.next() * of.len() as f32) as usize).min(of.len() - 1)]
    }
}

/// Ridged noise: sharp crests, for rock and washes.
fn ridged(x: f32, z: f32, seed: u32) -> f32 {
    (0..4)
        .map(|o| {
            let f = (1 << o) as f32;
            (1.0 - (noise(x * f, z * f, seed + o) * 2.0 - 1.0).abs()) / f
        })
        .sum::<f32>()
        / 1.875
}

// ---------------------------------------------------------------------
// The course line.

/// A closed line through the course, sampled every few feet.
struct Course {
    points: Vec<[f32; 2]>,
    /// Distance along the course at each point, ft.
    along: Vec<f32>,
    length: f32,
    /// Signed curvature at each point, 1/ft (positive turning left as
    /// seen from above with +z up the page... only its size and sign
    /// against `side` matter).
    curve: Vec<f32>,
    /// Segments by grid cell, for finding the nearest quickly.
    grid: HashMap<(i32, i32), Vec<usize>>,
}

const CELL: f32 = 48.0;

/// Where a point is against the course.
#[derive(Clone, Copy)]
struct Near {
    /// Distance to the centre line, ft.
    distance: f32,
    /// Distance along the course, ft.
    along: f32,
    /// Which side: positive to the left of the direction of travel.
    side: f32,
    /// The point index.
    index: usize,
}

impl Course {
    /// Resamples a closed polyline every `step` feet.
    fn new(raw: &[[f32; 2]], step: f32) -> Course {
        let n = raw.len();
        let mut total = 0.0;
        let mut lens = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = (raw[i], raw[(i + 1) % n]);
            let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            lens.push(l);
            total += l;
        }
        let count = (total / step).ceil().max(8.0) as usize;
        let step = total / count as f32;
        let mut points = Vec::with_capacity(count);
        let (mut seg, mut into) = (0usize, 0.0f32);
        for k in 0..count {
            let target = k as f32 * step;
            while into + lens[seg] < target && seg + 1 < n {
                into += lens[seg];
                seg += 1;
            }
            let t = if lens[seg] > 0.0 { (target - into) / lens[seg] } else { 0.0 };
            let (a, b) = (raw[seg], raw[(seg + 1) % n]);
            points.push([lerp(a[0], b[0], t), lerp(a[1], b[1], t)]);
        }
        let along: Vec<f32> = (0..count).map(|k| k as f32 * step).collect();
        let heading = |i: usize| {
            let (a, b) = (points[i % count], points[(i + 1) % count]);
            (b[1] - a[1]).atan2(b[0] - a[0])
        };
        let curve = (0..count)
            .map(|i| {
                let mut d = heading(i + 1) - heading(i + count - 1);
                while d > PI {
                    d -= TAU;
                }
                while d < -PI {
                    d += TAU;
                }
                d / (2.0 * step)
            })
            .collect();
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for i in 0..count {
            let (a, b) = (points[i], points[(i + 1) % count]);
            let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
            let (z0, z1) = (a[1].min(b[1]), a[1].max(b[1]));
            for cx in (x0 / CELL).floor() as i32..=(x1 / CELL).floor() as i32 {
                for cz in (z0 / CELL).floor() as i32..=(z1 / CELL).floor() as i32 {
                    grid.entry((cx, cz)).or_default().push(i);
                }
            }
        }
        Course { points, along, length: total, curve, grid }
    }

    /// The nearest point of the course within `reach` feet, if any.
    fn near(&self, p: [f32; 2], reach: f32) -> Option<Near> {
        let n = self.points.len();
        let r = (reach / CELL).ceil() as i32;
        let (cx, cz) = ((p[0] / CELL).floor() as i32, (p[1] / CELL).floor() as i32);
        let mut best: Option<Near> = None;
        for gx in cx - r..=cx + r {
            for gz in cz - r..=cz + r {
                let Some(list) = self.grid.get(&(gx, gz)) else { continue };
                for &i in list {
                    let (a, b) = (self.points[i], self.points[(i + 1) % n]);
                    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
                    let l2 = dx * dx + dz * dz;
                    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    let (qx, qz) = (a[0] + dx * t, a[1] + dz * t);
                    let d = ((p[0] - qx).powi(2) + (p[1] - qz).powi(2)).sqrt();
                    if d <= reach && best.is_none_or(|b| d < b.distance) {
                        let side = (dx * (p[1] - a[1]) - dz * (p[0] - a[0])).signum();
                        let step = self.length / n as f32;
                        best = Some(Near { distance: d, along: self.along[i] + t * step, side, index: i });
                    }
                }
            }
        }
        best
    }

    /// The point and direction at distance `s` along.
    fn at(&self, s: f32) -> ([f32; 2], [f32; 2]) {
        let n = self.points.len();
        let s = s.rem_euclid(self.length);
        let i = ((s / self.length * n as f32) as usize).min(n - 1);
        let (a, b) = (self.points[i], self.points[(i + 1) % n]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dz * dz).sqrt().max(1e-4);
        (a, [dx / l, dz / l])
    }

    /// The tightest turn's radius, ft.
    fn tightest(&self) -> f32 {
        // Over a few points, so noise in the sampling doesn't count.
        let n = self.curve.len();
        (0..n)
            .map(|i| {
                let c: f32 = (0..5).map(|k| self.curve[(i + k) % n]).sum::<f32>() / 5.0;
                1.0 / c.abs().max(1e-6)
            })
            .fold(f32::MAX, f32::min)
    }

    /// Whether the course comes within `gap` feet of itself, away from
    /// its neighbouring stretches.
    fn crosses(&self, gap: f32) -> bool {
        let n = self.points.len();
        let step = self.length / n as f32;
        let skip = (gap * 3.0 / step) as usize + 2;
        (0..n).step_by(2).any(|i| {
            let p = self.points[i];
            (0..n).step_by(2).any(|j| {
                let d = (i as isize - j as isize).unsigned_abs();
                let d = d.min(n - d);
                d > skip && ((p[0] - self.points[j][0]).powi(2) + (p[1] - self.points[j][1]).powi(2)) < gap * gap
            })
        })
    }
}

/// A closed loop of `radius` feet round `centre`, star-shaped so it never
/// crosses itself, its turns at least `min_turn` feet in radius.
fn star_loop(rng: &mut Rng, centre: [f32; 2], radius: [f32; 2], min_turn: f32, gap: f32) -> Course {
    for attempt in 0..200 {
        let shrink = 1.0 - attempt as f32 * 0.004;
        let harmonics: Vec<(f32, f32, f32)> = (2..=6)
            .map(|k| (k as f32, rng.range(0.0, 0.16) * shrink / (k as f32 * 0.5), rng.range(0.0, TAU)))
            .collect();
        let turn = rng.range(0.0, TAU);
        let raw: Vec<[f32; 2]> = (0..900)
            .map(|i| {
                let a = i as f32 / 900.0 * TAU;
                let r = 1.0 + harmonics.iter().map(|&(k, amp, ph)| amp * (k * a + ph).cos()).sum::<f32>();
                let (s, c) = (a + turn).sin_cos();
                [centre[0] + radius[0] * r * c, centre[1] + radius[1] * r * s]
            })
            .collect();
        let course = Course::new(&raw, 4.0);
        if course.tightest() >= min_turn && !course.crosses(gap) {
            return course;
        }
    }
    let raw: Vec<[f32; 2]> = (0..400)
        .map(|i| {
            let a = i as f32 / 400.0 * TAU;
            [centre[0] + radius[0] * a.cos(), centre[1] + radius[1] * a.sin()]
        })
        .collect();
    Course::new(&raw, 4.0)
}

/// A polygon with its corners rounded: each corner `(point, radius)`.
fn rounded(corners: &[([f32; 2], f32)]) -> Vec<[f32; 2]> {
    let n = corners.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (p, r) = corners[i];
        let prev = corners[(i + n - 1) % n].0;
        let next = corners[(i + 1) % n].0;
        let unit = |a: [f32; 2], b: [f32; 2]| {
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let l = (dx * dx + dz * dz).sqrt().max(1e-4);
            ([dx / l, dz / l], l)
        };
        let (u, lu) = unit(p, prev);
        let (v, lv) = unit(p, next);
        let cos = (u[0] * v[0] + u[1] * v[1]).clamp(-0.999, 0.999);
        let half = cos.acos() / 2.0;
        // How far back from the corner the arc starts.
        let back = (r / half.tan()).min(lu * 0.5).min(lv * 0.5);
        let r = back * half.tan();
        let a = [p[0] + u[0] * back, p[1] + u[1] * back];
        let b = [p[0] + v[0] * back, p[1] + v[1] * back];
        // The arc's centre, along the bisector.
        let bis = [u[0] + v[0], u[1] + v[1]];
        let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt().max(1e-4);
        let dist = r / half.sin();
        let c = [p[0] + bis[0] / bl * dist, p[1] + bis[1] / bl * dist];
        let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
        let mut a1 = (b[1] - c[1]).atan2(b[0] - c[0]);
        let cross = u[0] * v[1] - u[1] * v[0];
        // Go round the short way.
        if cross < 0.0 {
            while a1 < a0 {
                a1 += TAU;
            }
        } else {
            while a1 > a0 {
                a1 -= TAU;
            }
        }
        let steps = ((a1 - a0).abs() * r / 3.0).ceil().max(2.0) as usize;
        for k in 0..=steps {
            let t = a0 + (a1 - a0) * k as f32 / steps as f32;
            out.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
        }
    }
    out
}

/// A closed course laid out the way motocross tracks fold up on
/// themselves: a random tree is grown over a grid of `nx` × `nz` points,
/// `nodes` points big, and the course runs round its outline. Every
/// stretch is `cell` feet from the next, the turns are hairpins and
/// corners of `cell / 2` radius, and long straight branches make long
/// straights. Returns the outline's corners, in cells.
fn tree_outline(rng: &mut Rng, nx: i32, nz: i32, nodes: usize, straight: f32) -> Vec<[i32; 2]> {
    use std::collections::HashSet;
    let mut tree: Vec<([i32; 2], Option<usize>)> = vec![([rng.range(0.0, nx as f32) as i32, rng.range(0.0, nz as f32) as i32], None)];
    let mut taken: HashSet<[i32; 2]> = tree.iter().map(|t| t.0).collect();
    let mut cells: HashSet<[i32; 2]> = HashSet::new();
    cells.insert([tree[0].0[0] * 2, tree[0].0[1] * 2]);
    let dirs = [[1, 0], [0, 1], [-1, 0], [0, -1]];
    let mut tries = 0;
    while tree.len() < nodes && tries < 20000 {
        tries += 1;
        // Grow from a recent point more often: longer branches.
        let pick = if rng.next() < 0.35 { tree.len() - 1 - ((rng.next() * 3.0) as usize).min(tree.len() - 1) } else { (rng.next() * tree.len() as f32) as usize };
        let (at, parent) = tree[pick.min(tree.len() - 1)];
        let came = parent.map(|p| [at[0] - tree[p].0[0], at[1] - tree[p].0[1]]);
        let weights: Vec<f32> = dirs
            .iter()
            .map(|d| {
                let to = [at[0] + d[0], at[1] + d[1]];
                if to[0] < 0 || to[1] < 0 || to[0] >= nx || to[1] >= nz || taken.contains(&to) {
                    0.0
                } else if came == Some(*d) {
                    straight
                } else {
                    1.0
                }
            })
            .collect();
        let total: f32 = weights.iter().sum();
        if total == 0.0 {
            continue;
        }
        let mut r = rng.next() * total;
        let mut k = 0;
        for (i, w) in weights.iter().enumerate() {
            r -= w;
            if r <= 0.0 && *w > 0.0 {
                k = i;
                break;
            }
        }
        let d = dirs[k];
        let to = [at[0] + d[0], at[1] + d[1]];
        taken.insert(to);
        tree.push((to, Some(pick)));
        cells.insert([to[0] * 2, to[1] * 2]);
        cells.insert([at[0] * 2 + d[0], at[1] * 2 + d[1]]);
    }
    // The outline, anticlockwise: each cell side with nothing beyond it.
    let mut next: HashMap<[i32; 2], [i32; 2]> = HashMap::new();
    for &[x, z] in &cells {
        let has = |dx: i32, dz: i32| cells.contains(&[x + dx, z + dz]);
        if !has(0, -1) {
            next.insert([x, z], [x + 1, z]);
        }
        if !has(1, 0) {
            next.insert([x + 1, z], [x + 1, z + 1]);
        }
        if !has(0, 1) {
            next.insert([x + 1, z + 1], [x, z + 1]);
        }
        if !has(-1, 0) {
            next.insert([x, z + 1], [x, z]);
        }
    }
    let first = *next.keys().min().unwrap();
    let mut ring = vec![first];
    let mut at = next[&first];
    while at != first && ring.len() <= next.len() {
        ring.push(at);
        at = next[&at];
    }
    // Only the corners.
    let n = ring.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) != 0
        })
        .map(|i| ring[i])
        .collect()
}

/// The outline as a smooth course: scaled to `cell` ft, its corners
/// rounded, turned by `turn`, bent by a gentle warp of `warp` ft, and
/// centred on `centre`.
fn outline_course(corners: &[[i32; 2]], cell: [f32; 2], centre: [f32; 2], turn: f32, warp: f32, seed: u32) -> Course {
    let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
    for c in corners {
        for k in 0..2 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
    }
    let mid = [(lo[0] + hi[0]) as f32 / 2.0, (lo[1] + hi[1]) as f32 / 2.0];
    let r = cell[0].min(cell[1]) * 0.48;
    let pts: Vec<([f32; 2], f32)> = corners.iter().map(|c| ([(c[0] as f32 - mid[0]) * cell[0], (c[1] as f32 - mid[1]) * cell[1]], r)).collect();
    let smooth_line = rounded(&pts);
    let (s, c) = turn.sin_cos();
    let wave = cell[0].min(cell[1]) * 5.0;
    let bent: Vec<[f32; 2]> = smooth_line
        .iter()
        .map(|p| {
            let wx = (fbm(p[0] / wave, p[1] / wave, seed + 201) - 0.5) * 2.0 * warp;
            let wz = (fbm(p[0] / wave, p[1] / wave, seed + 202) - 0.5) * 2.0 * warp;
            let (x, z) = (p[0] + wx, p[1] + wz);
            [centre[0] + x * c - z * s, centre[1] + x * s + z * c]
        })
        .collect();
    Course::new(&bent, 3.0)
}

/// The straight runs of a course: (from, to) distances along, where it
/// turns no tighter than `radius` ft.
fn straights(course: &Course, radius: f32) -> Vec<(f32, f32)> {
    let n = course.points.len();
    let step = course.length / n as f32;
    let straight: Vec<bool> = (0..n).map(|i| (0..7).all(|k| course.curve[(i + n + k - 3) % n].abs() < 1.0 / radius)).collect();
    // Start at a bend so no run wraps round.
    let Some(bend) = straight.iter().position(|s| !s) else { return vec![(0.0, course.length)] };
    let mut runs = Vec::new();
    let mut from = None;
    for k in 0..=n {
        let i = (bend + k) % n;
        match (straight[i] && k < n, from) {
            (true, None) => from = Some(k),
            (false, Some(f)) => {
                runs.push(((bend + f) as f32 * step % course.length, (k - f) as f32 * step));
                from = None;
            }
            _ => {}
        }
    }
    // As (start, length).
    runs
}

/// Fills each straight with jumps one after another, leaving `exit` feet
/// after the turn and `entry` before the next; none within the
/// `clear` stretch (start, length).
fn fill_straights(course: &Course, runs: &[(f32, f32)], rng: &mut Rng, make: &mut dyn FnMut(&mut Rng, f32) -> Option<Shape>, exit: f32, entry: f32, gap: (f32, f32), clear: Option<(f32, f32)>) -> Features {
    let mut out = Vec::new();
    for &(start, len) in runs {
        let mut s = exit;
        // Where the last feature on this straight ended, if any.
        let mut last_end: Option<f32> = None;
        while s < len - entry {
            let room = len - entry - s;
            let Some(shape) = make(rng, room) else { break };
            if shape.length() > room {
                s += 6.0;
                continue;
            }
            // Flow: a jump needs a run-up after the feature before it,
            // the longer the jump the more, so the rider meets its face
            // at the speed it is built for and lands set up for the next.
            if let Some(end) = last_end {
                let need = shape.run_up();
                if s - end < need {
                    s = end + need;
                    continue;
                }
            }
            let at = (start + s) % course.length;
            let blocked = clear.is_some_and(|(c0, cl)| {
                let a = (at - c0).rem_euclid(course.length);
                let b = (at + shape.length() - c0).rem_euclid(course.length);
                a < cl || b < cl
            });
            if !blocked {
                if std::env::var_os("FT_FEATURES").is_some() {
                    let (p0, _) = course.at(at);
                    let (p1, _) = course.at((at + shape.length()) % course.length);
                    eprintln!("feature {:.0} {:.0} {:.0} {:.0} {:.0} {:?}", at, p0[0], p0[1], p1[0], p1[1], shape);
                }
                out.push((at, shape));
                last_end = Some(s + shape.length());
                s += shape.length() + rng.range(gap.0, gap.1);
            } else {
                s += 10.0;
            }
        }
    }
    Features(out)
}

/// The longest straight's (start, length).
fn longest(runs: &[(f32, f32)]) -> (f32, f32) {
    runs.iter().copied().fold((0.0, 0.0), |a, b| if b.1 > a.1 { b } else { a })
}

// ---------------------------------------------------------------------
// Jumps and other shapes along a course.

#[derive(Clone, Copy, Debug)]
enum Shape {
    /// Up, flat top, down: a tabletop (ft: up face, top, down face).
    Table { up: f32, top: f32, down: f32, height: f32 },
    /// A takeoff, a gap and a landing: a double (or triple with a
    /// longer gap).
    Gap { up: f32, gap: f32, down: f32, height: f32 },
    /// Up to a higher plateau and gently down later.
    Step { up: f32, top: f32, down: f32, height: f32 },
    /// A row of small bumps.
    Whoops { count: u32, spacing: f32, height: f32 },
    /// A row of rounded rollers.
    Rollers { count: u32, spacing: f32, height: f32 },
}

impl Shape {
    /// The flat ground it wants before it after another feature, ft: a
    /// gap about two thirds of its length, a table or step half its top,
    /// rollers and whoops little.
    fn run_up(&self) -> f32 {
        match *self {
            Shape::Gap { gap, .. } => gap * 0.7,
            Shape::Table { top, height, .. } => top * 0.5 + height * 2.0,
            Shape::Step { height, .. } => height * 4.0,
            Shape::Whoops { .. } => 20.0,
            Shape::Rollers { .. } => 15.0,
        }
    }

    /// The same jump with faces long enough to ride like the original
    /// game's: the takeoff's lip no steeper than an original jump of its
    /// height (about 20 degrees at 3 ft up to 33 at 13 ft and over), and
    /// landings and table downs at most 24 degrees. Measured along the
    /// originals' course lines (`jump_angles` example).
    fn tamed(self) -> Shape {
        // The face rises as (s/up)^1.4, so its lip slope is 1.4 h / up;
        // a smoothstep down falls at most 1.5 h / down.
        let lip = |h: f32| ((17.0 + 1.2 * h).min(33.0)).to_radians().tan();
        let land = 24f32.to_radians().tan();
        let up_for = |up: f32, h: f32| up.max(1.4 * h / lip(h));
        let down_for = |down: f32, h: f32| down.max(1.5 * h / land);
        match self {
            Shape::Table { up, top, down, height } => Shape::Table { up: up_for(up, height), top, down: down_for(down, height), height },
            Shape::Step { up, top, down, height } => Shape::Step { up: up_for(up, height), top, down: down_for(down, height), height },
            Shape::Gap { up, gap, down, height } => Shape::Gap { up: up_for(up, height), gap, down: down_for(down, height), height },
            other => other,
        }
    }

    /// The share of a gap that the takeoff's back and the landing's front
    /// each take: enough that neither is a wall (about 30 degrees at most,
    /// like the originals' doubles), leaving the pit at least 0.16 of the
    /// gap.
    fn gap_sides(gap: f32, height: f32) -> f32 {
        (2.6 * height / gap.max(1.0)).clamp(0.2, 0.42)
    }

    fn length(&self) -> f32 {
        match *self {
            Shape::Table { up, top, down, .. } | Shape::Step { up, top, down, .. } => up + top + down,
            Shape::Gap { up, gap, down, .. } => up + gap + down,
            Shape::Whoops { count, spacing, .. } | Shape::Rollers { count, spacing, .. } => count as f32 * spacing,
        }
    }

    /// The height `s` feet into the shape.
    fn height(&self, s: f32) -> f32 {
        let ramp = |s: f32, len: f32| smooth(s / len);
        match *self {
            Shape::Table { up, top, down, height } | Shape::Step { up, top, down, height } => {
                if s < 0.0 {
                    0.0
                } else if s < up {
                    // A takeoff face: steeper towards its lip.
                    height * (s / up).powf(1.4)
                } else if s < up + top {
                    height
                } else if s < up + top + down {
                    height * (1.0 - ramp(s - up - top, down))
                } else {
                    0.0
                }
            }
            Shape::Gap { up, gap, down, height } => {
                if s < 0.0 {
                    0.0
                } else if s < up {
                    height * (s / up).powf(1.4)
                } else if s < up + gap * Shape::gap_sides(gap, height) {
                    let f = Shape::gap_sides(gap, height);
                    height * (1.0 - smooth((s - up) / (gap * f)))
                } else if s < up + gap * (1.0 - Shape::gap_sides(gap, height)) {
                    0.0
                } else if s < up + gap {
                    let f = Shape::gap_sides(gap, height);
                    height * smooth((s - up - gap * (1.0 - f)) / (gap * f))
                } else if s < up + gap + down {
                    height * (1.0 - smooth((s - up - gap) / down))
                } else {
                    0.0
                }
            }
            Shape::Whoops { count, spacing, height } => {
                if s < 0.0 || s > count as f32 * spacing {
                    0.0
                } else {
                    height * (0.5 - 0.5 * (s / spacing * TAU).cos())
                }
            }
            Shape::Rollers { count, spacing, height } => {
                if s < 0.0 || s > count as f32 * spacing {
                    0.0
                } else {
                    height * (s / spacing * PI).sin().powi(2)
                }
            }
        }
    }
}

/// Shapes placed along a course: where each starts.
struct Features(Vec<(f32, Shape)>);

impl Features {
    fn height(&self, along: f32, length: f32) -> f32 {
        self.0
            .iter()
            .map(|(at, shape)| {
                let mut s = along - at;
                if s < -length / 2.0 {
                    s += length;
                }
                shape.height(s)
            })
            .fold(0.0, f32::max)
    }
}

/// Picks shapes for the course's straight stretches: wherever a stretch
/// is at least as straight as `straight` (ft radius) for the shape's
/// length plus run-up, leaving `clear` feet after the start.
fn place_features(course: &Course, rng: &mut Rng, make: &mut dyn FnMut(&mut Rng) -> Shape, straight: f32, clear: f32, spacing: f32) -> Features {
    let n = course.points.len();
    let step = course.length / n as f32;
    let mut out = Vec::new();
    let mut s = clear;
    while s < course.length - 60.0 {
        let shape = make(rng).tamed();
        let len = shape.length() + 30.0;
        let first = (s / step) as usize;
        let last = ((s + len) / step) as usize;
        let ok = last < n && (first..=last).all(|i| course.curve[i % n].abs() < 1.0 / straight);
        if ok {
            out.push((s + 15.0, shape));
            s += len + spacing * rng.range(0.6, 1.4);
        } else {
            s += 12.0;
        }
    }
    Features(out)
}

// ---------------------------------------------------------------------
// The land.

struct Land {
    size: usize,
    scale: f32,
    heights: Vec<f32>,
    /// How much of each sample is track surface, 0 to 1.
    track: Vec<f32>,
    /// How worn the racing line is there, 0 to 1.
    wear: Vec<f32>,
    /// Where the ground is too steep for plants, 0 to 1.
    steep: Vec<f32>,
    /// A material that covers the off-track ground there, and how much
    /// (paths, pits, parking).
    patch: Vec<(u8, f32)>,
    /// Where on the track each sample is: ft left of the centre line
    /// (negative right), the distance along, and the track's half width.
    lane: Vec<[f32; 3]>,
}

impl Land {
    fn new(kind: Kind) -> Land {
        let (size, scale) = kind.size();
        Land {
            size,
            scale,
            heights: vec![0.0; size * size],
            track: vec![0.0; size * size],
            wear: vec![0.0; size * size],
            steep: vec![0.0; size * size],
            patch: vec![(0, 0.0); size * size],
            lane: vec![[f32::MAX, 0.0, 0.0]; size * size],
        }
    }

    fn span(&self) -> f32 {
        (self.size - 1) as f32 * self.scale
    }

    fn at(&self, x: f32, z: f32) -> f32 {
        let fx = (x / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let fz = (z / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let h = |x: usize, z: usize| self.heights[z * self.size + x];
        lerp(lerp(h(ix, iz), h(ix + 1, iz), tx), lerp(h(ix, iz + 1), h(ix + 1, iz + 1), tx), tz)
    }

    fn sample<T: Copy + Into<f32>>(&self, field: &[T], x: f32, z: f32) -> f32 {
        let fx = (x / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let fz = (z / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let h = |x: usize, z: usize| field[z * self.size + x].into();
        lerp(lerp(h(ix, iz), h(ix + 1, iz), tx), lerp(h(ix, iz + 1), h(ix + 1, iz + 1), tx), tz)
    }

    /// [`Land::lane`] at (x, z) ft, smoothly between samples where all
    /// four round it are on the same stretch of track.
    fn lane_at(&self, x: f32, z: f32) -> [f32; 3] {
        let fx = (x / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let fz = (z / self.scale).clamp(0.0, (self.size - 1) as f32 - 0.001);
        let (ix, iz) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - ix as f32, fz - iz as f32);
        let n = self.size;
        let q = [self.lane[iz * n + ix], self.lane[iz * n + ix + 1], self.lane[(iz + 1) * n + ix], self.lane[(iz + 1) * n + ix + 1]];
        let nearest = q[(tx >= 0.5) as usize + 2 * (tz >= 0.5) as usize];
        if q.iter().any(|l| l[0] == f32::MAX || (l[1] - q[0][1]).abs() > 30.0) {
            return nearest;
        }
        let mix = |k: usize| lerp(lerp(q[0][k], q[1][k], tx), lerp(q[2][k], q[3][k], tx), tz);
        [mix(0), mix(1), nearest[2]]
    }

    fn compute_steep(&mut self) {
        let n = self.size;
        for z in 1..n - 1 {
            for x in 1..n - 1 {
                let dx = self.heights[z * n + x + 1] - self.heights[z * n + x - 1];
                let dz = self.heights[(z + 1) * n + x] - self.heights[(z - 1) * n + x];
                let slope = (dx * dx + dz * dz).sqrt() / (2.0 * self.scale);
                self.steep[z * n + x] = smooth((slope - 0.45) / 0.5);
            }
        }
    }

    /// How far each sample is from the nearest of `courses`' centre
    /// lines, ft (exact close to the line, a chamfer estimate further out).
    fn course_distance(&self, courses: &[&Course]) -> Vec<f32> {
        let n = self.size;
        let mut d = vec![f32::MAX; n * n];
        for course in courses {
            for p in &course.points {
                let (cx, cz) = ((p[0] / self.scale).round() as isize, (p[1] / self.scale).round() as isize);
                for z in cz - 3..=cz + 3 {
                    for x in cx - 3..=cx + 3 {
                        if x < 0 || z < 0 || x >= n as isize || z >= n as isize {
                            continue;
                        }
                        let e = ((x as f32 * self.scale - p[0]).powi(2) + (z as f32 * self.scale - p[1]).powi(2)).sqrt();
                        let i = z as usize * n + x as usize;
                        d[i] = d[i].min(e);
                    }
                }
            }
        }
        let (a, b) = (self.scale, self.scale * std::f32::consts::SQRT_2);
        for z in 0..n {
            for x in 0..n {
                let i = z * n + x;
                let mut v = d[i];
                if x > 0 { v = v.min(d[i - 1] + a); }
                if z > 0 {
                    v = v.min(d[i - n] + a);
                    if x > 0 { v = v.min(d[i - n - 1] + b); }
                    if x + 1 < n { v = v.min(d[i - n + 1] + b); }
                }
                d[i] = v;
            }
        }
        for z in (0..n).rev() {
            for x in (0..n).rev() {
                let i = z * n + x;
                let mut v = d[i];
                if x + 1 < n { v = v.min(d[i + 1] + a); }
                if z + 1 < n {
                    v = v.min(d[i + n] + a);
                    if x + 1 < n { v = v.min(d[i + n + 1] + b); }
                    if x > 0 { v = v.min(d[i + n - 1] + b); }
                }
                d[i] = v;
            }
        }
        d
    }

    /// Sets the track down in the land the way the original tracks sit:
    /// in a valley or along a hillside, the ground climbing away from it
    /// on one side or both from `near` ft out to `rise` ft higher by
    /// `far` ft, rolling under the track itself by up to `roll` ft, and a
    /// ring of hills at the land's edge so the world doesn't end in a
    /// flat line under the sky.
    fn valley(&mut self, courses: &[&Course], near: f32, far: f32, rise: f32, roll: f32, ring: f32, seed: u32) {
        let dist = self.course_distance(courses);
        let n = self.size;
        let span = self.span();
        for z in 0..n {
            for x in 0..n {
                let (fx, fz) = (x as f32 * self.scale, z as f32 * self.scale);
                let i = z * n + x;
                // Higher on some sides than others: a valley here, a
                // hillside there, a saddle between.
                let side = 0.35 + 1.1 * fbm(fx / 650.0, fz / 650.0, seed + 21);
                let climb = smooth((dist[i] - near) / (far - near));
                // Knolls and spurs on the slopes, so they aren't smooth walls.
                let spurs = (fbm(fx / 210.0, fz / 210.0, seed + 22) - 0.5) * 0.5 * climb;
                let rolling = (fbm(fx / 330.0, fz / 330.0, seed + 23) - 0.5) * 2.0 * roll;
                let edge = fx.min(span - fx).min(fz).min(span - fz);
                let hills = (1.0 - smooth(edge / (span * 0.22))) * ring * (0.6 + 0.8 * fbm(fx / 500.0, fz / 500.0, seed + 24));
                self.heights[i] += rise * (side * climb + spurs) + rolling + hills;
            }
        }
    }

    /// Lays a course into the land: its corridor `half` feet each side
    /// at the course's own height (`profile`, by distance along), with
    /// `features` on it and its turns banked, blended into the ground
    /// over `blend` feet.
    fn carve(&mut self, course: &Course, half: f32, blend: f32, profile: &dyn Fn(f32) -> f32, features: &Features, bank: f32, sculpt: f32, calm: Option<(f32, f32)>) {
        let n = self.size;
        let reach = half + blend;
        let len = course.length;
        // Shaped like a raced track: graded dirt pushed up in a lip along
        // each edge (taller round the outside of turns), the edges ragged
        // where dirt spills into the grass, a worn groove down the middle
        // and sets of low rollers on the quieter straights.
        let lip_w = (self.scale * 1.7).max(5.0);
        let seed = (len * 7.0) as u32;
        let rolled: Vec<f32> = if sculpt > 0.0 {
            let k = (len / 4.0).ceil() as usize;
            (0..k)
                .map(|j| {
                    let s = j as f32 * 4.0;
                    let near_jump = [-45.0f32, -20.0, 0.0, 20.0, 45.0].iter().any(|o| features.height((s + o).rem_euclid(len), len) > 0.05);
                    let in_calm = calm.is_some_and(|(at, l)| (s - at).rem_euclid(len) < l + 60.0 || (at - s).rem_euclid(len) < 90.0);
                    if near_jump || in_calm { return 0.0; }
                    let set = smooth((noise(s / 260.0, 3.5, seed) - 0.5) * 5.0);
                    set * (s / 46.0 * TAU).sin() * 1.1 * sculpt
                })
                .collect()
        } else {
            Vec::new()
        };
        // Rollers fade in and out smoothly between jumps.
        let rolled: Vec<f32> = (0..rolled.len())
            .map(|j| (-3i32..=3).map(|o| rolled[(j as i32 + o).rem_euclid(rolled.len() as i32) as usize]).sum::<f32>() / 7.0)
            .collect();
        for z in 0..n {
            for x in 0..n {
                let p = [x as f32 * self.scale, z as f32 * self.scale];
                let Some(near) = course.near(p, reach) else { continue };
                let i = z * n + x;
                let curve = course.curve[near.index];
                // The outside of a turn rises: a berm.
                let outside = -near.side * curve.signum();
                let turn = (curve.abs() * 120.0).min(1.0);
                let berm = if outside > 0.0 { bank * turn * smooth(near.distance / half) } else { 0.0 };
                let jump = if near.distance < half + 2.0 { features.height(near.along, course.length) } else { 0.0 };
                let edge = smooth((near.distance - half) / blend);
                let mut on = profile(near.along) + jump * (1.0 - smooth((near.distance - half + 3.0) / 5.0)) + berm;
                let mut spill = 0.0;
                if sculpt > 0.0 {
                    let t = (near.distance / half).min(1.0);
                    let groove = -0.7 * (1.0 - t * t);
                    let roll = if rolled.is_empty() { 0.0 } else { rolled[((near.along / 4.0) as usize).min(rolled.len() - 1)] } * (1.0 - smooth((near.distance - half * 0.75) / (half * 0.3)));
                    on += (groove + roll) * sculpt;
                    // Ragged edges: the dirt wanders out into the grass.
                    spill = (fbm(p[0] / 40.0, p[1] / 40.0, seed + 11) - 0.45) * 12.0 + smooth((fbm(p[0] / 18.0, p[1] / 18.0, seed + 12) - 0.58) * 6.0) * 9.0;
                }
                let mut h = lerp(on, self.heights[i], edge);
                let mut lip_dirt = 0.0;
                if sculpt > 0.0 {
                    // The lip: broken here and there, higher on the outside of turns.
                    // Low and often broken on the straights, so the ground
                    // past the track stays easy to ride out onto; it builds
                    // into a berm only round the outside of turns.
                    let gap = smooth((noise(near.along / 45.0, near.side, seed + 13) - 0.45) * 5.0);
                    let lip_h = (0.7 + 0.8 * noise(near.along / 30.0, near.side * 2.0, seed + 14)) * gap * (1.0 + if outside > 0.0 { 3.0 * turn } else { 0.0 });
                    let u = (near.distance - half - lip_w * 0.6) / lip_w;
                    let lip = lip_h * (1.0 - u * u).max(0.0).powf(1.5) * sculpt;
                    h += lip;
                    // The pushed-up dirt is bare, grass only on its back.
                    lip_dirt = smooth(lip / 0.8) * (1.0 - smooth(u * 1.4));
                }
                self.heights[i] = h;
                let surface = (1.0 - smooth((near.distance - half - spill.max(-3.0)) / 6.0)).max(lip_dirt * 0.95);
                self.track[i] = self.track[i].max(surface);
                if near.distance < self.lane[i][0].abs() {
                    self.lane[i] = [near.distance * near.side, near.along, half];
                }
                // The racing line: worn darker, wandering a little.
                let line = (near.distance - half * 0.3 * (near.along / 90.0).sin()).abs();
                self.wear[i] = self.wear[i].max(1.0 - smooth(line / (half * 0.7)));
            }
            if z % 256 == 0 {
                eprint!(".");
            }
        }
    }

    /// Widens the track to `wide` ft each side round the start line at
    /// `line`, so a full gate of riders (eight abreast, 7.75 ft apart, as
    /// the game lines them up) starts on the track: from 70 ft behind the
    /// line to 15 past it, narrowing back over 60 ft either way.
    fn widen_start(&mut self, course: &Course, line: f32, half: f32, wide: f32, blend: f32, profile: &dyn Fn(f32) -> f32) {
        let n = self.size;
        let len = course.length;
        for z in 0..n {
            for x in 0..n {
                let p = [x as f32 * self.scale, z as f32 * self.scale];
                let Some(near) = course.near(p, wide + blend) else { continue };
                let diff = (near.along - line + len / 2.0).rem_euclid(len) - len / 2.0;
                let out = (-70.0 - diff).max(diff - 15.0).max(0.0);
                let w = 1.0 - smooth(out / 60.0);
                if w <= 0.0 {
                    continue;
                }
                let reach = half + (wide - half) * w;
                // The track itself, jumps and all, stays as carved.
                if near.distance > reach + blend || near.distance <= half {
                    continue;
                }
                let i = z * n + x;
                let edge = smooth((near.distance - reach) / blend);
                self.heights[i] = lerp(profile(near.along), self.heights[i], edge);
                self.track[i] = self.track[i].max(1.0 - smooth((near.distance - reach) / 6.0));
            }
        }
    }

    /// The course's own height at each point: the land under it,
    /// smoothed along over `window` feet.
    fn profile(&self, course: &Course, window: f32) -> Vec<f32> {
        let n = course.points.len();
        let raw: Vec<f32> = course.points.iter().map(|p| self.at(p[0], p[1])).collect();
        let step = course.length / n as f32;
        let w = ((window / step) as isize).max(1);
        (0..n as isize)
            .map(|i| (-w..=w).map(|k| raw[((i + k).rem_euclid(n as isize)) as usize]).sum::<f32>() / (2 * w + 1) as f32)
            .collect()
    }
}

fn profile_fn(course: &Course, heights: Vec<f32>) -> impl Fn(f32) -> f32 + '_ {
    move |s: f32| {
        let n = heights.len();
        let f = s.rem_euclid(course.length) / course.length * n as f32;
        let i = f as usize % n;
        lerp(heights[i], heights[(i + 1) % n], f.fract())
    }
}

// ---------------------------------------------------------------------
// Scenery.

/// A model placed in the scene.
struct Model {
    name: String,
    at: [f32; 3],
    look: [f32; 2],
    scale: f32,
}

/// An eco-system plant species.
struct Species {
    name: &'static str,
    /// Height and width, ft: middle, smallest, largest.
    height: [f32; 3],
    width: [f32; 3],
    /// A tree's trunk radius (solid), or none for a bush.
    trunk: Option<f32>,
    /// How common, relative.
    weight: f32,
}

const fn tree(name: &'static str, h: f32, w: f32, trunk: f32, weight: f32) -> Species {
    Species { name, height: [h, h * 0.75, h * 1.3], width: [w, w * 0.75, w * 1.3], trunk: Some(trunk), weight }
}

const fn bush(name: &'static str, h: f32, w: f32, weight: f32) -> Species {
    Species { name, height: [h, h * 0.7, h * 1.4], width: [w, w * 0.7, w * 1.4], trunk: None, weight }
}

fn flora(biome: Biome) -> Vec<Species> {
    match biome {
        Biome::Meadow => vec![tree("aspen01", 30.0, 12.0, 0.7, 2.0), tree("aspen02", 34.0, 13.0, 0.7, 2.0), tree("tree01", 26.0, 20.0, 0.9, 2.0), tree("tree02", 28.0, 20.0, 0.9, 1.5), bush("shrub01", 4.0, 6.0, 3.0), bush("grass02", 2.0, 5.0, 6.0), bush("wideleaf01", 4.0, 4.0, 2.0)],
        Biome::DryHills => vec![tree("mesquite01", 16.0, 14.0, 0.6, 2.0), tree("mesquite02", 14.0, 13.0, 0.6, 2.0), bush("drytumbleweed01", 3.0, 3.0, 3.0), bush("shrub02", 3.5, 6.0, 4.0), bush("grass03", 2.0, 5.0, 5.0), tree("cypress01", 30.0, 8.0, 0.6, 0.6)],
        Biome::Desert => vec![tree("saguaro01", 28.0, 7.0, 1.1, 1.5), tree("saguaro03", 22.0, 6.0, 1.0, 1.5), tree("ocotillo01", 12.0, 6.0, 0.4, 1.5), bush("drytumbleweed02", 3.0, 3.0, 3.0), bush("tumbleweed01", 3.0, 3.0, 2.0), tree("drymesquite01", 12.0, 12.0, 0.5, 1.5), bush("shrub03", 3.0, 5.0, 3.0)],
        Biome::Forest => vec![tree("pine01", 48.0, 14.0, 0.9, 4.0), tree("pine02", 52.0, 15.0, 0.9, 4.0), tree("pine03", 56.0, 16.0, 1.0, 3.0), tree("cypress02", 40.0, 10.0, 0.8, 1.0), bush("shrub01", 4.0, 6.0, 3.0), bush("wideleaf02", 4.0, 4.0, 3.0), bush("grass01", 1.5, 4.0, 3.0)],
        Biome::Autumn => vec![tree("aspen01", 32.0, 13.0, 0.7, 3.0), tree("aspen03", 28.0, 12.0, 0.7, 3.0), tree("tree03", 26.0, 20.0, 0.9, 2.0), tree("pine01", 44.0, 14.0, 0.9, 1.5), bush("shrub02", 4.0, 6.0, 3.0), bush("wideleaf03", 4.0, 4.0, 2.0), bush("grass03", 2.0, 5.0, 3.0)],
        Biome::Alpine => vec![tree("pine02", 46.0, 13.0, 0.9, 4.0), tree("pine03", 52.0, 14.0, 1.0, 3.0), tree("snowpine01", 42.0, 13.0, 0.9, 0.5), tree("aspen02", 28.0, 11.0, 0.7, 1.5), bush("shrub03", 3.0, 5.0, 3.0), bush("grass02", 1.5, 4.0, 4.0)],
        Biome::Pit => vec![bush("drytumbleweed03", 3.0, 3.0, 2.0), bush("shrub02", 3.5, 6.0, 2.0), tree("pine01", 40.0, 13.0, 0.9, 1.0), bush("grass03", 2.0, 5.0, 2.0)],
        Biome::Arena => Vec::new(),
    }
}

/// Plants per square foot of open ground.
fn plant_density(kind: Kind, biome: Biome) -> f32 {
    let base = match biome {
        Biome::Forest | Biome::Alpine => 1.0 / 380.0,
        Biome::Autumn => 1.0 / 450.0,
        Biome::Meadow => 1.0 / 850.0,
        Biome::DryHills => 1.0 / 950.0,
        // The original Baja deserts hold 40,000 plants each.
        Biome::Desert => 1.0 / 600.0,
        Biome::Pit => 1.0 / 2600.0,
        Biome::Arena => 0.0,
    };
    if kind == Kind::Enduro {
        base * 1.2
    } else {
        base
    }
}

/// The eco-system file (`.esb`, method 1) for `objects`: each a species
/// index, place (ft) and its size bytes.
fn write_esb(species: &[Species], objects: &[(u8, [f32; 3], u8, u8)], land: &Land) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&1u32.to_le_bytes());
    let string = |out: &mut Vec<u8>, s: &str| {
        out.push(s.len() as u8 + 1);
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    };
    for slot in 0..256usize {
        let Some(sp) = species.get(slot) else {
            out.push(0);
            continue;
        };
        out.push(1);
        string(&mut out, sp.name);
        string(&mut out, "");
        for v in sp.height.iter().chain(&sp.width).copied().chain([0.0f32; 7]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        match sp.trunk {
            Some(r) => {
                out.extend_from_slice(&1u32.to_le_bytes());
                out.extend_from_slice(&1u32.to_le_bytes());
                for v in [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, r, sp.height[0] * 0.4] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            None => out.extend_from_slice(&0u32.to_le_bytes()),
        }
    }
    out.push(0);
    out.extend_from_slice(&(objects.len() as u32).to_le_bytes());
    let unit = land.span() / 65536.0;
    for &(s, p, h, w) in objects {
        out.push(s);
        for v in p {
            out.extend_from_slice(&((v / unit).round().clamp(0.0, 65535.0) as u16).to_le_bytes());
        }
        out.push(h);
        out.push(w);
    }
    out
}

/// Scatters plants over the land where it is open: off the track and
/// clear of `keep` (places and their radii), thicker in noisy clumps.
fn scatter(land: &Land, species: &[Species], density: f32, seed: u32, keep: &[([f32; 2], f32)], track_clear: f32, infield: Option<([f32; 2], [f32; 2])>) -> Vec<(u8, [f32; 3], u8, u8)> {
    if species.is_empty() || density <= 0.0 {
        return Vec::new();
    }
    let mut rng = Rng(seed ^ 0x5eed);
    let total: f32 = species.iter().map(|s| s.weight).sum();
    let span = land.span();
    let cell = 1.0 / density.sqrt();
    let mut out = Vec::new();
    let cells = (span / cell) as usize;
    for cz in 0..cells {
        for cx in 0..cells {
            let x = (cx as f32 + rng.next()) * cell;
            let z = (cz as f32 + rng.next()) * cell;
            if x < 30.0 || z < 30.0 || x > span - 30.0 || z > span - 30.0 {
                continue;
            }
            let clump = fbm(x / 600.0, z / 600.0, seed + 77);
            if rng.next() > smooth((clump - 0.3) / 0.35) {
                continue;
            }
            let i = (z / land.scale) as usize * land.size + (x / land.scale) as usize;
            if land.track[i] > 0.01 || land.steep[i] > 0.6 || land.patch[i].1 > 0.3 {
                continue;
            }
            // Keep a clear band beside the track.
            let near_track = land.sample(&land.track, x + track_clear, z) + land.sample(&land.track, x - track_clear, z) + land.sample(&land.track, x, z + track_clear) + land.sample(&land.track, x, z - track_clear);
            if near_track > 0.01 {
                continue;
            }
            if keep.iter().any(|(p, r)| (p[0] - x).powi(2) + (p[1] - z).powi(2) < r * r) {
                continue;
            }
            let mut pick = rng.next() * total;
            let mut s = 0;
            for (k, sp) in species.iter().enumerate() {
                pick -= sp.weight;
                if pick <= 0.0 {
                    s = k;
                    break;
                }
            }
            let inside = infield.is_some_and(|(lo, hi)| x > lo[0] && x < hi[0] && z > lo[1] && z < hi[1]);
            if inside && species[s].trunk.is_some() {
                continue;
            }
            let y = land.at(x, z);
            out.push((s as u8, [x, y, z], (rng.next() * 255.0) as u8, (rng.next() * 255.0) as u8));
            if out.len() >= 80000 {
                return out;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
// Painting the ground.
//
// As the original tracks do, most of the land shares a few textures: the
// open ground is a picture that repeats every tile, with a handful of
// variants whose differences fade out towards the tile's edges so any
// two tiles meet without a seam. Only tiles with the track, pits, rock
// or the like on them get a texture of their own.

/// Value noise that repeats every `period` lattice cells.
fn pnoise(x: f32, z: f32, period: i32, seed: u32) -> f32 {
    let (ix, iz) = (x.floor() as i32, z.floor() as i32);
    let (tx, tz) = (x - ix as f32, z - iz as f32);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let w = |i: i32| i.rem_euclid(period.max(1));
    let h = |a: i32, b: i32| hash(w(a), w(b), seed);
    let top = lerp(h(ix, iz), h(ix + 1, iz), sx);
    let bottom = lerp(h(ix, iz + 1), h(ix + 1, iz + 1), sx);
    lerp(top, bottom, sz)
}

/// Fractal noise over a square of `size` ft that repeats, with its
/// largest features `cells` to a side.
fn pfbm(x: f32, z: f32, size: f32, cells: i32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut total = 0.0;
    for o in 0..4 {
        let c = cells << o;
        sum += pnoise(x / size * c as f32, z / size * c as f32, c, seed + o as u32) * amp;
        total += amp;
        amp *= 0.5;
    }
    sum / total
}

/// The ground pictures, each shrunk to the size one repeat covers at the
/// track's texture resolution. Every repeat divides a tile exactly.
struct Ground {
    /// Per material: side and RGB (0 to 1), or none to paint flat.
    images: Vec<Option<(usize, Vec<[f32; 3]>)>>,
    tile_feet: f32,
}

impl Ground {
    fn load(read: &dyn Fn(&str) -> Option<Vec<u8>>, tex: usize, tile_feet: f32) -> Ground {
        let images = MATERIALS
            .iter()
            .map(|&(name, _, period)| {
                let file = format!("{name}.png");
                let bytes = read(&file)?;
                let (w, h, rgb) = decode_png(&bytes).map_err(|e| eprintln!("ground {file}: {e}")).ok()?;
                // Whole repeats per tile, a power of two so they divide
                // the texture's side.
                let repeats = 2f32.powf((tile_feet / period).log2().round()).clamp(1.0, 16.0) as usize;
                let side = (tex / repeats).min(w.min(h));
                let mut out = vec![[0.0f32; 3]; side * side];
                let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
                for y in 0..side {
                    for x in 0..side {
                        let (x0, x1) = (x * w / side, ((x + 1) * w / side).max(x * w / side + 1));
                        let (y0, y1) = (y * h / side, ((y + 1) * h / side).max(y * h / side + 1));
                        let mut sum = [0.0f32; 3];
                        for sy in y0..y1 {
                            for sx in x0..x1 {
                                let p = &rgb[(sy * w + sx) * 3..][..3];
                                for k in 0..3 {
                                    sum[k] += lin(p[k]);
                                }
                            }
                        }
                        let count = ((x1 - x0) * (y1 - y0)) as f32;
                        out[y * side + x] = sum.map(|v| (v / count).powf(1.0 / 2.2));
                    }
                }
                Some((side, out))
            })
            .collect();
        Ground { images, tile_feet }
    }

    /// Material `m`'s colour at (x, z) ft; `fpp` feet per texture pixel.
    fn color(&self, m: usize, x: f32, z: f32, fpp: f32, seed: u32) -> [f32; 3] {
        match &self.images[m] {
            Some((side, px)) => {
                let i = ((x / fpp).floor() as isize).rem_euclid(*side as isize) as usize;
                let j = ((z / fpp).floor() as isize).rem_euclid(*side as isize) as usize;
                px[j * side + i]
            }
            None => {
                let t = self.tile_feet;
                let base = MATERIALS[m].1;
                let n = pfbm(x, z, t, (t / 12.0) as i32, seed + m as u32) - 0.5;
                let fine = hash(((x / fpp).floor() as i32).rem_euclid((t / fpp) as i32), ((z / fpp).floor() as i32).rem_euclid((t / fpp) as i32), seed + 3) - 0.5;
                base.map(|c| (c * (1.0 + n * 0.35 + fine * 0.12)).clamp(0.0, 1.0))
            }
        }
    }
}

pub fn decode_png(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), Box<dyn std::error::Error>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf)?;
    let px = &buf[..info.buffer_size()];
    let rgb: Vec<u8> = match info.color_type {
        png::ColorType::Rgb => px.to_vec(),
        png::ColorType::Rgba => px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0]]).collect(),
        png::ColorType::Indexed => return Err("indexed PNG".into()),
    };
    Ok((info.width as usize, info.height as usize, rgb))
}

/// What covers the open ground for a biome: the main material, the one
/// it gives way to in patches, and a third for the odd bare or dark spot.
fn biome_ground(biome: Biome) -> (usize, usize, usize) {
    match biome {
        Biome::Meadow => (GRASS, DRYGRASS, LOOSE),
        Biome::DryHills => (DRYGRASS, LOOSE, GRAVEL),
        Biome::Desert => (SAND, GRAVEL, LOOSE),
        Biome::Forest => (FOREST, GRASS, MUD),
        Biome::Autumn => (DRYGRASS, FOREST, GRASS),
        Biome::Alpine => (GRASS, GRAVEL, ROCK),
        Biome::Pit => (GRAVEL, LOOSE, ROCK),
        Biome::Arena => (CLAY, LOOSE, CLAY),
    }
}

/// The track's own surface for a biome: the dirt and its worn line.
fn track_ground(kind: Kind, biome: Biome) -> (usize, usize) {
    match (kind, biome) {
        (Kind::Supercross | Kind::Tag, _) => (LOOSE, CLAY),
        (_, Biome::Desert) => (LOOSE, TRACKDIRT),
        (_, Biome::Pit) => (GRAVEL, TRACKDIRT),
        (_, Biome::Forest | Biome::Autumn) => (LOOSE, MUD),
        _ => (LOOSE, TRACKDIRT),
    }
}

/// The most textures a track may have (the browser's texture array limit).
const MAX_TEXTURES: usize = 256;

/// How many open-ground variants a track shares between its plain tiles.
const VARIANTS: u32 = 10;

/// The open ground at (u, v) ft into a tile, for variant `k`.
fn open_ground(biome: Biome, ground: &Ground, u: f32, v: f32, k: u32, fpp: f32, seed: u32) -> [f32; 3] {
    let t = ground.tile_feet;
    let (a, b, c) = biome_ground(biome);
    // The same in every tile: soft patches that repeat with the tile.
    let base = pfbm(u, v, t, 3, seed + 11);
    // Each variant's own patches, faded to nothing at the tile's edges.
    let edge = smooth(u.min(t - u).min(v).min(t - v) / (t * 0.14));
    let own = (fbm(u / (t * 0.35) + k as f32 * 7.3, v / (t * 0.35), seed + 40 + k) - 0.5) * edge;
    let spot = smooth((fbm(u / (t * 0.22) + k as f32 * 3.1, v / (t * 0.22), seed + 70 + k) - 0.66) / 0.12) * edge;
    let mix = smooth((base * 0.3 + 0.35 - 0.42 + own * 1.8) / 0.3) * 0.7;
    let ca = ground.color(a, u, v, fpp, seed);
    let cb = ground.color(b, u + 16.0, v + 32.0, fpp, seed);
    let cc = ground.color(c, u + 8.0, v + 24.0, fpp, seed);
    std::array::from_fn(|i| lerp(lerp(ca[i], cb[i], mix), cc[i], spot * 0.6))
}

/// Paints the land's textures: the shared open-ground variants, and a
/// texture of its own for each tile with something on it.
fn paint(land: &Land, kind: Kind, biome: Biome, ground: &Ground, seed: u32) -> (Vec<(u32, Vec<u16>)>, Vec<Option<u16>>) {
    let tex = kind.texture();
    let n = land.size;
    let tiles_side = (n - 1) / 64;
    let tile_feet = 64.0 * land.scale;
    let fpp = tile_feet / tex as f32;
    let (track_a, track_b) = track_ground(kind, biome);
    let mut textures: Vec<(u32, Vec<u16>)> = Vec::new();
    // The shared variants first.
    for k in 0..VARIANTS {
        let mut px = Vec::with_capacity(tex * tex);
        for j in 0..tex {
            for i in 0..tex {
                let c = open_ground(biome, ground, (i as f32 + 0.5) * fpp, (j as f32 + 0.5) * fpp, k, fpp, seed);
                px.push(rgb555(c[0], c[1], c[2]));
            }
        }
        textures.push((tex as u32, px));
    }
    // What is on each tile, or just over its edge: the track (always
    // painted), else how many samples of patches or rock.
    let scores: Vec<u32> = (0..tiles_side * tiles_side)
        .map(|t| {
            let (tx, tz) = (t % tiles_side, t / tiles_side);
            let mut score = 0u32;
            for z in (tz * 64).saturating_sub(2)..=((tz + 1) * 64 + 2).min(n - 1) {
                for x in (tx * 64).saturating_sub(2)..=((tx + 1) * 64 + 2).min(n - 1) {
                    let i = z * n + x;
                    if land.track[i] > 0.0 || land.wear[i] > 0.0 {
                        return u32::MAX;
                    }
                    if land.patch[i].1 > 0.0 || (land.steep[i] > 0.02 && biome != Biome::Arena) {
                        score += 1;
                    }
                }
            }
            score
        })
        .collect();
    // The browser's WebGPU holds at most 256 layers in a texture array:
    // past that, the tiles with least on them share the plain ground.
    let room = MAX_TEXTURES - VARIANTS as usize;
    let mut ranked: Vec<u32> = scores.iter().copied().filter(|&s| s > 0).collect();
    ranked.sort_unstable_by(|a, b| b.cmp(a));
    let least = if ranked.len() > room { ranked[room - 1].max(1) } else { 1 };
    let mut tiles = Vec::with_capacity(tiles_side * tiles_side);
    let mut own = 0;
    for tz in 0..tiles_side {
        for tx in 0..tiles_side {
            let k = (hash(tx as i32, tz as i32, seed + 99) * VARIANTS as f32) as u32 % VARIANTS;
            let score = scores[tz * tiles_side + tx];
            if score < least || own >= room {
                tiles.push(Some(k as u16));
                continue;
            }
            own += 1;
            let mut px = Vec::with_capacity(tex * tex);
            for j in 0..tex {
                for i in 0..tex {
                    let (u, v) = ((i as f32 + 0.5) * fpp, (j as f32 + 0.5) * fpp);
                    let (x, z) = (tx as f32 * tile_feet + u, tz as f32 * tile_feet + v);
                    let mut c = open_ground(biome, ground, u, v, k, fpp, seed);
                    let (pm, pw) = land.patch[((z / land.scale) as usize).min(n - 1) * n + ((x / land.scale) as usize).min(n - 1)];
                    if pw > 0.0 {
                        let rag = (fbm(x / 9.0, z / 9.0, seed + 6) - 0.5) * 0.5;
                        let t = smooth((pw + rag - 0.15) / 0.5);
                        let mut cp = ground.color(pm as usize, x, z, fpp, seed);
                        if biome == Biome::Arena && pm as usize == CONCRETE {
                            // A stadium's concrete under a race: darker, with
                            // dirt tracked over it in drifts and tyre smears.
                            let cd = ground.color(LOOSE, x, z, fpp, seed);
                            let drift = smooth((fbm(x / 16.0, z / 16.0, seed + 41) - 0.3) / 0.3) * 0.8 + fbm(x / 2.5, z / 2.5, seed + 42) * 0.25;
                            cp = std::array::from_fn(|q| lerp(cp[q] * 0.5, cd[q] * 0.8, drift.min(0.92)));
                        }
                        c = std::array::from_fn(|q| lerp(c[q], cp[q], t));
                    }
                    let steep = land.sample(&land.steep, x, z);
                    if steep > 0.0 && biome != Biome::Arena {
                        let cr = ground.color(ROCK, x, z, fpp, seed);
                        let t = smooth((steep + (fbm(x / 14.0, z / 14.0, seed + 8) - 0.5) * 0.6) / 0.8);
                        c = std::array::from_fn(|q| lerp(c[q], cr[q], t));
                    }
                    let track = land.sample(&land.track, x, z);
                    let lane = land.lane_at(x, z);
                    // Dirt roosted out onto the ground just past the edge:
                    // speckles and clods, thinning out over 14 ft.
                    if lane[0] != f32::MAX && track < 1.0 {
                        let past = (lane[0].abs() - lane[2]).max(0.0);
                        let reach = 1.0 - smooth(past / 14.0);
                        let clods = hash((x / 0.9).floor() as i32, (z / 0.9).floor() as i32, seed + 31);
                        let drift = fbm(x / 5.0, z / 5.0, seed + 32);
                        let roost = smooth((clods * 0.6 + drift * 0.7 + reach * 0.9 - 1.15) / 0.2) * reach;
                        if roost > 0.0 {
                            let cd = ground.color(track_a, x, z, fpp, seed);
                            c = std::array::from_fn(|q| lerp(c[q], cd[q] * 0.92, roost * 0.85));
                        }
                    }
                    if track > 0.0 {
                        // A ragged edge where the track meets the ground,
                        // and loose dirt thrown just past it.
                        let rag = (fbm(x / 6.0, z / 6.0, seed + 5) - 0.5) * 0.7;
                        let t = smooth((track + rag - 0.15) / 0.45);
                        let wear = land.sample(&land.wear, x, z);
                        let streak = fbm(x / 3.0, z / 3.0, seed + 9);
                        let cd = ground.color(track_a, x, z, fpp, seed);
                        let cw = ground.color(track_b, x + 5.0, z + 9.0, fpp, seed);
                        let mut ct: [f32; 3] = std::array::from_fn(|q| lerp(cd[q], cw[q], (wear * 0.8 * (0.7 + streak * 0.6)).min(1.0)));
                        // Big patches: dry, sun-bleached dirt and darker
                        // damp ground, as a real track has after a day's
                        // riding.
                        let patches = fbm(x / 55.0, z / 55.0, seed + 33) - 0.5;
                        let damp = smooth((fbm(x / 30.0, z / 30.0, seed + 34) - 0.62) / 0.15);
                        let tone = 1.0 + patches * 0.35 - damp * 0.22;
                        ct = ct.map(|v| (v * tone).clamp(0.0, 1.0));
                        if lane[0] != f32::MAX {
                            // Ruts: the lines the bikes take, packed dark
                            // into the dirt, wandering across the track as
                            // they go, most of them near the racing line.
                            let (side, along, half) = (lane[0], lane[1], lane[2].max(1.0));
                            let mut rut = 0.0f32;
                            for k in 0..5u32 {
                                let lane_at = half * (0.55 * (fbm(along / 70.0, k as f32 * 3.7, seed + 40 + k) - 0.5) * 2.0 + (k as f32 - 2.0) * 0.16);
                                let width = 1.0 + 0.8 * fbm(along / 25.0, k as f32, seed + 50 + k);
                                let depth = smooth((fbm(along / 40.0, k as f32 * 1.3, seed + 60 + k) - 0.3) / 0.3);
                                rut = rut.max((-((side - lane_at) / width).powi(2)).exp() * depth);
                            }
                            // Braking bumps across the track, ahead of the
                            // worn sections.
                            let bumps = (along / 3.2).sin().max(0.0).powi(3) * smooth((fbm(along / 60.0, 0.0, seed + 70) - 0.55) / 0.15) * wear;
                            let dark = 1.0 - rut * 0.4 - bumps * 0.14;
                            ct = ct.map(|v| v * dark);
                        }
                        c = std::array::from_fn(|q| lerp(c[q], ct[q], t));
                    }
                    px.push(rgb555(c[0], c[1], c[2]));
                }
            }
            tiles.push(Some(textures.len() as u16));
            textures.push((tex as u32, px));
        }
        eprint!("t");
    }
    eprint!(" {own} own tiles ");
    (textures, tiles)
}

/// The ground map (`<track>.gmt`): which two ground materials cover each
/// terrain sample, so the viewer can lay their full-size pictures over
/// the baked tiles as fine detail. The same mix [`paint`] bakes, taken
/// once per sample.
///
/// Layout: `GMT1`, the samples per side and the material count (u32),
/// then per material its name (16 bytes, zero padded) and the feet one
/// repeat covers (f32), the baked tiles' feet per pixel (f32), and per
/// sample the first material, the second, the second's share (0..255)
/// and a spare byte.
fn ground_map(land: &Land, kind: Kind, biome: Biome, seed: u32) -> Vec<u8> {
    let n = land.size;
    let tile_feet = 64.0 * land.scale;
    let fpp = tile_feet / kind.texture() as f32;
    let (track_a, track_b) = track_ground(kind, biome);
    let (ga, gb, gc) = biome_ground(biome);
    let mut out = Vec::with_capacity(16 + MATERIALS.len() * 20 + n * n * 4);
    out.extend_from_slice(b"GMT1");
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(MATERIALS.len() as u32).to_le_bytes());
    for &(name, _, period) in &MATERIALS {
        let mut field = [0u8; 16];
        field[..name.len()].copy_from_slice(name.as_bytes());
        out.extend_from_slice(&field);
        out.extend_from_slice(&period.to_le_bytes());
    }
    out.extend_from_slice(&fpp.to_le_bytes());
    for j in 0..n {
        for i in 0..n {
            let (x, z) = (i as f32 * land.scale, j as f32 * land.scale);
            let mut w = [0.0f32; MATERIALS.len()];
            // The open ground, as open_ground mixes it.
            let (tx, tz) = ((x / tile_feet) as i32, (z / tile_feet) as i32);
            let k = (hash(tx, tz, seed + 99) * VARIANTS as f32) as u32 % VARIANTS;
            let (u, v) = (x.rem_euclid(tile_feet), z.rem_euclid(tile_feet));
            let t = tile_feet;
            let base = pfbm(u, v, t, 3, seed + 11);
            let edge = smooth(u.min(t - u).min(v).min(t - v) / (t * 0.14));
            let own = (fbm(
                u / (t * 0.35) + k as f32 * 7.3,
                v / (t * 0.35),
                seed + 40 + k,
            ) - 0.5)
                * edge;
            let spot = smooth(
                (fbm(
                    u / (t * 0.22) + k as f32 * 3.1,
                    v / (t * 0.22),
                    seed + 70 + k,
                ) - 0.66)
                    / 0.12,
            ) * edge
                * 0.6;
            let mix = smooth((base * 0.3 + 0.35 - 0.42 + own * 1.8) / 0.3) * 0.7;
            w[ga] += (1.0 - mix) * (1.0 - spot);
            w[gb] += mix * (1.0 - spot);
            w[gc] += spot;
            let over = |w: &mut [f32; MATERIALS.len()], m: usize, t: f32| {
                for q in w.iter_mut() {
                    *q *= 1.0 - t;
                }
                w[m] += t;
            };
            let (pm, pw) = land.patch[j * n + i];
            if pw > 0.0 {
                let rag = (fbm(x / 9.0, z / 9.0, seed + 6) - 0.5) * 0.5;
                over(&mut w, pm as usize, smooth((pw + rag - 0.15) / 0.5));
            }
            let steep = land.steep[j * n + i];
            if steep > 0.0 && biome != Biome::Arena {
                over(
                    &mut w,
                    ROCK,
                    smooth((steep + (fbm(x / 14.0, z / 14.0, seed + 8) - 0.5) * 0.6) / 0.8),
                );
            }
            let track = land.track[j * n + i];
            if track > 0.0 {
                let rag = (fbm(x / 6.0, z / 6.0, seed + 5) - 0.5) * 0.7;
                let t = smooth((track + rag - 0.15) / 0.45);
                let wear =
                    (land.wear[j * n + i] * 0.8 * (0.7 + fbm(x / 3.0, z / 3.0, seed + 9) * 0.6))
                        .min(1.0);
                over(&mut w, track_a, t);
                w[track_a] -= t * wear;
                w[track_b] += t * wear;
            }
            // The two that cover most of it.
            let mut first = 0;
            for m in 1..w.len() {
                if w[m] > w[first] {
                    first = m;
                }
            }
            let mut second = if first == 0 { 1 } else { 0 };
            for m in 0..w.len() {
                if m != first && w[m] > w[second] {
                    second = m;
                }
            }
            let share = w[second] / (w[first] + w[second]).max(1e-6);
            out.extend_from_slice(&[first as u8, second as u8, (share * 255.0).round() as u8, 0]);
        }
    }
    out
}

// ---------------------------------------------------------------------
// Files.

/// The course as a `.tdf`: one closed node of cross-sections round the
/// line, and, for a lapped race, start and finish lines.
fn write_tdf(course: &Course, half: f32, lines: Option<(f32, f32, f32)>, land: &Land, flags: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let f = |out: &mut Vec<u8>, v: f32| out.extend_from_slice(&v.to_le_bytes());
    let u = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let n = course.points.len();
    let every = ((40.0 / (course.length / n as f32)) as usize).max(1);
    let mut idx: Vec<usize> = (0..n).step_by(every).collect();
    idx.push(0);
    out.push(1);
    u(&mut out, 1);
    for (t, v) in [(2u8, 0.0f32), (3, 0.0), (4, 1.0)] {
        out.push(t);
        f(&mut out, v);
    }
    out.push(5);
    u(&mut out, 0);
    out.push(6);
    u(&mut out, flags);
    out.push(7);
    f(&mut out, course.length);
    for &i in &idx {
        let p = course.points[i];
        let (_, d) = course.at(course.along[i]);
        let side = [-d[1], d[0]];
        let y = land.at(p[0], p[1]);
        out.push(8);
        for (t, x, z) in [(9u8, p[0], p[1]), (10, p[0] + side[0] * half, p[1] + side[1] * half), (11, p[0] - side[0] * half, p[1] - side[1] * half)] {
            out.push(t);
            for v in [x, y, z] {
                f(&mut out, v);
            }
        }
    }
    out.push(13);
    u(&mut out, 1);
    out.push(14);
    u(&mut out, 0);
    u(&mut out, 0);
    if let Some((finish, start, tape)) = lines {
        // The start line (as wide as the gate), then the finish.
        for (t, at, back, half) in [(15u8, start, 0.0f32, tape), (16, finish, 0.0, half)] {
            let (p, d) = course.at(at);
            let side = [-d[1], d[0]];
            out.push(t);
            for v in [p[0] - d[0] * back, 0.0, p[1] - d[1] * back, d[0], 0.0, d[1], side[0] * half, 0.0, side[1] * half, half * 2.0] {
                f(&mut out, v);
            }
        }
    }
    out
}

fn write_wpt(course: &Course, gates: &[f32]) -> String {
    let mut s = String::from("[Waypoints]\r\n");
    let _ = write!(s, "NumWaypoints={}\r\n\r\n", gates.len());
    for (k, &at) in gates.iter().enumerate() {
        let (p, d) = course.at(at);
        let _ = write!(s, "Point{n}X={:.2}\r\nPoint{n}Z={:.2}\r\nPoint{n}LookX={:.2}\r\nPoint{n}LookZ={:.2}\r\n\r\n", p[0], p[1], d[0], d[1], n = k + 1);
    }
    s
}

/// Light and air for a biome: ambient, sun colour, sun direction (from
/// where it shines, unit-ish), fog.
fn light(biome: Biome, sky: Option<&str>) -> ([u8; 3], [u8; 3], [f32; 3], [u8; 3]) {
    let sunset = matches!(sky, Some("terra11.cub" | "terra16.cub" | "terra17.cub"));
    let overcast = matches!(sky, Some("terra04.cub" | "terra05.cub" | "terra15.cub" | "burn03.cub"));
    if biome == Biome::Arena {
        // Floodlit from high overhead: the floor and riders bright, the
        // stands and roof in the dimmer light round them.
        return ([74, 74, 86], [208, 204, 194], [0.15, 1.0, 0.1], [30, 30, 40]);
    }
    if sunset {
        ([105, 98, 100], [205, 170, 135], [0.9, 0.35, 0.3], [178, 160, 150])
    } else if overcast {
        ([125, 125, 130], [165, 165, 165], [0.4, 1.0, 0.5], [170, 174, 180])
    } else if biome == Biome::Desert {
        ([118, 112, 108], [205, 198, 182], [0.5, 1.0, 0.3], [196, 190, 182])
    } else {
        ([112, 114, 120], [196, 192, 182], [0.6, 1.0, 0.4], [180, 186, 196])
    }
}

/// The Free Pack sky's colour at the horizon (its pictures' average just
/// above it), for the fog.
fn horizon(sky: &str) -> Option<[u8; 3]> {
    Some(match sky {
        "burn03.cub" => [137, 137, 139],
        "terra01.cub" => [135, 138, 140],
        "terra03.cub" => [122, 126, 132],
        "terra04.cub" => [125, 123, 120],
        "terra05.cub" => [134, 135, 140],
        "terra06.cub" => [174, 174, 175],
        "terra09.cub" => [188, 190, 193],
        "terra11.cub" => [161, 156, 152],
        "terra12.cub" => [191, 197, 202],
        "terra15.cub" => [123, 124, 126],
        "terra16.cub" => [157, 161, 165],
        "terra17.cub" => [153, 143, 153],
        "terra18.cub" => [160, 162, 169],
        "wind01.cub" => [174, 177, 173],
        _ => return None,
    })
}

struct Scene<'a> {
    def: &'a Def,
    land: &'a Land,
    start: ([f32; 3], [f32; 2]),
    models: Vec<Model>,
    races: Vec<String>,
    podium: Option<([f32; 3], [f32; 2])>,
}

fn write_scene(s: &Scene) -> String {
    let d = s.def;
    let span = s.land.span();
    let mut o = String::new();
    let _ = write!(o, "; Free Pack track, made from scratch by mm2-tools freetracks (seed {})\r\n", d.seed);
    let _ = write!(o, "[SceneInfo]\r\nSceneName=\"{}\"\r\nLightGround=1\r\n", d.name);
    if !s.races.is_empty() {
        let _ = write!(o, "NumberOfWaypointRaces={}\r\n", s.races.len());
        for (k, r) in s.races.iter().enumerate() {
            let _ = write!(o, "WaypointRaceName{}=\"{}\"\r\n", k + 1, r);
        }
    }
    let (p, dir) = s.start;
    let _ = write!(o, "DefaultStartPosition={:.1},{:.1},{:.1}\r\nEventType={}\r\nDefaultStartDirection={:.3},0.0,{:.3}\r\n", p[0], p[1], p[2], d.kind.event(), dir[0], dir[1]);
    if let Some((pp, pd)) = s.podium {
        let _ = write!(o, "PodiumPosition={:.1},{:.1},{:.1}\r\nPodiumDirection={:.3},0.0,{:.3}\r\n", pp[0], pp[1], pp[2], pd[0], pd[1]);
    }
    let (w, _) = d.kind.size();
    let blocks = (w - 1) / 256;
    let _ = write!(o, "\r\n[Environment]\r\nTerrainFile={}.trn\r\n", d.file);
    if let Some(sky) = d.sky {
        let _ = write!(o, "CubeFile={sky}\r\n");
    }
    let _ = write!(o, "TerrainWidth={blocks}\r\nTerrainBreadth={blocks}\r\nTerrainWidthScale={0:.1}\r\nTerrainBreadthScale={0:.1}\r\nParticleTextureMapName=Track15P.tga\r\n", s.land.scale);
    let (amb, sun, from, fog) = light(d.biome, d.sky);
    // Fog the colour of the sky where it meets the ground, so far hills
    // fade into the sky instead of a pale band under it.
    let fog = d.sky.and_then(horizon).unwrap_or(fog);
    let _ = write!(
        o,
        "\r\n[Lights]\r\nNumberOfLights=2\r\n[Light1]\r\nType=Ambient\r\nColorRGB={},{},{}\r\n[Light2]\r\nType=Directional\r\nColorRGB={},{},{}\r\nPosition={:.1},{:.1},{:.1}\r\nTargetPosition={:.1},60.0,{:.1}\r\nPlaceLightAtInfinity=T\r\nCastsShadows=T\r\nEmitsLight=T\r\n",
        amb[0], amb[1], amb[2], sun[0], sun[1], sun[2],
        span / 2.0 + from[0] * 3000.0, from[1] * 3000.0, span / 2.0 + from[2] * 3000.0,
        span / 2.0, span / 2.0
    );
    let _ = write!(o, "\r\n[Fog]\r\nColorRGB={},{},{}\r\n\r\n[Sounds]\r\nCrowdPresent={}\r\n", fog[0], fog[1], fog[2], if matches!(d.kind, Kind::Supercross | Kind::National) { "T" } else { "F" });
    // Zone 1: the track (dust and dirt); 2: the open ground; 3: rock and
    // concrete (no dust).
    let _ = write!(o, "\r\n[TerrainZone1]\r\nGenerateDust=T\r\nGenerateDirt=T\r\n[TerrainZone2]\r\nSurfaceFriction=1.0\r\nSurfaceDrag=1.1\r\nSurfaceTraction=0.9\r\nGenerateDust=F\r\nGenerateDirt=T\r\n[TerrainZone3]\r\nSurfaceFriction=1.0\r\nSurfaceDrag=1.0\r\nSurfaceTraction=0.95\r\nGenerateDust=F\r\nGenerateDirt=F\r\n");
    if !s.models.is_empty() {
        let _ = write!(o, "\r\n[StaticModels]\r\nNumberOfStaticModels={}\r\n", s.models.len());
        for (k, m) in s.models.iter().enumerate() {
            let _ = write!(
                o,
                "[Model{k}]\r\nSLT={name}.slt\r\nPosition={x:.2},{y:.2},{z:.2}\r\nUseLighting=T\r\nLookVector={lx:.4},0.0,{lz:.4}\r\nUpVector=0.0,1.0,0.0\r\nScale={s:.3},{s:.3},{s:.3}\r\n",
                k = k + 1, name = m.name, x = m.at[0], y = m.at[1], z = m.at[2], s = m.scale, lx = m.look[0], lz = m.look[1]
            );
        }
    }
    o
}

// ---------------------------------------------------------------------
// The events.

struct Made {
    land: Land,
    /// Each course with its gates (distances along) and name.
    courses: Vec<(Course, f32, Vec<f32>, String)>,
    /// Where the finish line and the start's tape are along the course,
    /// and the tape's half width.
    lap_lines: Option<(f32, f32, f32)>,
    start: ([f32; 3], [f32; 2]),
    models: Vec<Model>,
    keep: Vec<([f32; 2], f32)>,
    podium: Option<([f32; 3], [f32; 2])>,
    /// Where only bushes and grass grow, no trees: inside the track.
    infield: Option<([f32; 2], [f32; 2])>,
}


/// Picks a jump that fits in `room` feet: the biggest kinds are tried
/// first when there is space for them, then smaller ones.
fn pick_shape(r: &mut Rng, room: f32, menu: &[(f32, fn(&mut Rng) -> Shape)]) -> Option<Shape> {
    let fits: Vec<(f32, Shape)> = menu.iter().map(|(w, f)| (*w, f(r).tamed())).filter(|(_, s)| s.length() <= room).collect();
    let total: f32 = fits.iter().map(|f| f.0).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = r.next() * total;
    for (w, s) in &fits {
        x -= w;
        if x <= 0.0 {
            return Some(*s);
        }
    }
    fits.last().map(|f| f.1)
}

const NATIONAL_JUMPS: &[(f32, fn(&mut Rng) -> Shape)] = &[
    (3.0, |r| Shape::Table { up: r.range(20.0, 28.0), top: r.range(20.0, 45.0), down: r.range(24.0, 34.0), height: r.range(8.0, 12.0) }),
    // The gap grows with the face's height, so a tall face is never
    // followed by a short gap the rider sails past.
    (3.0, |r| {
        let height = r.range(9.0, 13.0);
        Shape::Gap { up: r.range(20.0, 26.0), gap: height * r.range(5.2, 6.0), down: r.range(28.0, 36.0), height }
    }),
    (1.5, |r| Shape::Step { up: r.range(18.0, 22.0), top: r.range(40.0, 80.0), down: r.range(50.0, 80.0), height: r.range(6.0, 9.0) }),
    (1.5, |r| Shape::Rollers { count: r.range(3.0, 5.99) as u32, spacing: r.range(26.0, 34.0), height: r.range(3.5, 5.0) }),
    (1.2, |r| Shape::Whoops { count: r.range(6.0, 12.99) as u32, spacing: r.range(11.0, 14.0), height: r.range(1.8, 2.8) }),
    (0.8, |r| Shape::Rollers { count: 2, spacing: r.range(22.0, 28.0), height: r.range(2.5, 3.5) }),
    (0.4, |r| Shape::Rollers { count: 1, spacing: r.range(20.0, 26.0), height: r.range(2.5, 3.5) }),
];

const QUARRY_JUMPS: &[(f32, fn(&mut Rng) -> Shape)] = &[
    (3.0, |r| Shape::Gap { up: r.range(28.0, 36.0), gap: r.range(70.0, 120.0), down: r.range(40.0, 55.0), height: r.range(11.0, 16.0) }),
    (2.0, |r| Shape::Table { up: r.range(30.0, 40.0), top: r.range(40.0, 70.0), down: r.range(40.0, 55.0), height: r.range(10.0, 15.0) }),
    (1.0, |r| Shape::Rollers { count: r.range(3.0, 5.99) as u32, spacing: r.range(34.0, 44.0), height: r.range(4.0, 6.0) }),
];

const SX_JUMPS: &[(f32, fn(&mut Rng) -> Shape)] = &[
    (2.5, |r| Shape::Gap { up: r.range(14.0, 16.0), gap: r.range(58.0, 70.0), down: r.range(18.0, 22.0), height: r.range(9.0, 11.0) }),
    (3.0, |r| Shape::Gap { up: r.range(12.0, 14.0), gap: r.range(34.0, 46.0), down: r.range(15.0, 18.0), height: r.range(7.0, 8.5) }),
    // Rhythm rollers spaced to roll or double, and whoops to skim.
    (2.5, |r| Shape::Rollers { count: r.range(3.0, 5.99) as u32, spacing: r.range(22.0, 26.0), height: r.range(2.4, 3.0) }),
    (1.5, |r| Shape::Whoops { count: r.range(8.0, 12.99) as u32, spacing: r.range(9.5, 11.0), height: r.range(1.9, 2.5) }),
    (1.5, |r| Shape::Table { up: r.range(13.0, 15.0), top: r.range(15.0, 30.0), down: r.range(14.0, 17.0), height: r.range(6.0, 8.0) }),
    (1.0, |r| Shape::Step { up: r.range(12.0, 14.0), top: r.range(18.0, 28.0), down: r.range(18.0, 26.0), height: r.range(4.5, 6.0) }),
    (0.8, |r| Shape::Rollers { count: 2, spacing: r.range(20.0, 24.0), height: r.range(2.8, 3.4) }),
];

/// The course's bounding box, ft.
fn bounds(course: &Course) -> ([f32; 2], [f32; 2]) {
    course.points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]))
}

/// Places along the course where it turns tightly: (distance along,
/// point, outward direction, radius).
fn apexes(course: &Course, tighter: f32) -> Vec<(f32, [f32; 2], [f32; 2], f32)> {
    let n = course.points.len();
    let mut out = Vec::new();
    for i in 0..n {
        let c = course.curve[i].abs();
        if c < 1.0 / tighter {
            continue;
        }
        let peak = (1..12).all(|k| course.curve[(i + k) % n].abs() <= c && course.curve[(i + n - k) % n].abs() < c);
        if peak {
            let (p, d) = course.at(course.along[i]);
            // The outside of the turn.
            let left = [-d[1], d[0]];
            let s = -course.curve[i].signum();
            out.push((course.along[i], p, [left[0] * s, left[1] * s], 1.0 / c));
        }
    }
    out
}

fn model(name: &str, land: &Land, x: f32, z: f32, look: [f32; 2]) -> Model {
    Model { name: name.to_string(), at: [x, land.at(x, z), z], look, scale: 1.0 }
}

fn national(def: &Def) -> Made {
    let mut rng = Rng(def.seed.wrapping_mul(7919));
    let mut land = Land::new(Kind::National);
    let span = land.span();
    let seed = def.seed;
    let centre = [span / 2.0, span / 2.0];
    // The layout first, so the land can be shaped round it.
    let cell = rng.range(72.0, 84.0);
    let mut course = None;
    for attempt in 0..400 {
        let nodes = rng.range(7.0, 11.99) as usize;
        let long = cell * rng.range(2.0, 3.0);
        let outline = tree_outline(&mut rng, 4, 6, nodes, 1.5);
        let c = outline_course(&outline, [long, cell], centre, rng.range(0.0, TAU), cell * 0.15, seed + attempt);
        // Folded up, not strung out: the layout's box no longer than
        // twice its width, each side of it the length of a straight.
        let (lo, hi) = bounds(&c);
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        let (a, b) = (w.max(h), w.min(h));
        if a > b * 2.0 || b < 380.0 {
            continue;
        }
        if (3200.0..4000.0).contains(&c.length) && c.tightest() >= cell * 0.34 && !c.crosses(cell * 0.72) {
            course = Some(c);
            break;
        }
    }
    let course = course.expect("a national layout");
    let (lo, hi) = bounds(&course);
    let relief = match def.biome {
        Biome::Alpine => 240.0,
        Biome::Forest | Biome::Autumn => 160.0,
        Biome::DryHills => 150.0,
        _ => 110.0,
    };
    // The track sits on a hillside: a long slope across it, a mound or
    // two, and bigger hills further out.
    let slope_dir = rng.range(0.0, TAU);
    let (sd, cd) = slope_dir.sin_cos();
    let climb = rng.range(35.0, 75.0);
    let mound = [centre[0] + rng.range(-250.0, 250.0), centre[1] + rng.range(-250.0, 250.0)];
    let mound_h = rng.range(15.0, 35.0);
    let n = land.size;
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 * land.scale, z as f32 * land.scale);
            let (dx, dz) = (fx - centre[0], fz - centre[1]);
            let along = (dx * cd + dz * sd) / 700.0;
            let hill = climb * smooth((along + 1.0) / 2.0);
            let m = mound_h * (-((fx - mound[0]).powi(2) + (fz - mound[1]).powi(2)) / (260.0f32 * 260.0)).exp();
            let gentle = (fbm(fx / 520.0, fz / 520.0, seed + 1) - 0.5) * 24.0;
            // Out past the track, real hills.
            let out_x = (lo[0] - 150.0 - fx).max(fx - hi[0] - 150.0).max(0.0);
            let out_z = (lo[1] - 150.0 - fz).max(fz - hi[1] - 150.0).max(0.0);
            let rim = smooth((out_x * out_x + out_z * out_z).sqrt() / 900.0);
            let big = fbm(fx / 1500.0, fz / 1500.0, seed) * 0.7 + ridged(fx / 2100.0, fz / 2100.0, seed + 2) * 0.3;
            land.heights[z * n + x] = hill + m + gentle + big * relief * rim + fbm(fx / 90.0, fz / 90.0, seed + 3) * 3.0 * rim;
        }
    }
    land.valley(&[&course], 26.0, 270.0, relief * 0.7, 20.0, relief * 0.8, seed);
    let raw = land.profile(&course, 120.0);
    let runs = straights(&course, 140.0);
    let (s0, sl) = longest(&runs);
    if std::env::var_os("FT_DEBUG").is_some() {
        eprintln!("runs {:?}", runs.iter().map(|r| r.1 as i32).collect::<Vec<_>>());
    }
    let line = (s0 + sl * 0.3) % course.length;
    let features = fill_straights(&course, &runs, &mut rng, &mut |r, room| pick_shape(r, room, NATIONAL_JUMPS), 20.0, 18.0, (14.0, 40.0), Some(((line - 80.0).rem_euclid(course.length), 80.0 + (sl * 0.7).min(300.0))));
    if std::env::var_os("FT_DEBUG").is_some() {
        eprintln!("{} features: {:?}", features.0.len(), features.0.iter().map(|f| format!("{:.0}:{:?}", f.0, f.1)).collect::<Vec<_>>());
    }
    let half = 17.0;
    {
        let profile = profile_fn(&course, raw);
        land.carve(&course, half, cell / 2.0 - half, &profile, &features, 5.0, 1.0, Some(((line - 80.0).rem_euclid(course.length), 80.0 + (sl * 0.7).min(320.0))));
        land.widen_start(&course, line, half, START_HALF, 12.0, &profile);
        if std::env::var_os("FT_DEBUG").is_some() {
            let (p, _) = course.at(line);
            eprintln!("start {line:.0} at {:.0},{:.0}: near along {:?}", p[0], p[1], course.near(p, 5.0).map(|n| n.along));
        }
    }
    // The pits, off to the side nearest the start.
    let (p0, d0) = course.at(line);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let sides = [[lo[0] - 170.0, p0[1]], [hi[0] + 170.0, p0[1]], [p0[0], lo[1] - 170.0], [p0[0], hi[1] + 170.0]];
    let pits = sides
        .into_iter()
        .filter(|p| course.near(*p, 140.0).is_none())
        .min_by(|a, b| {
            let da = (a[0] - p0[0]).powi(2) + (a[1] - p0[1]).powi(2);
            let db = (b[0] - p0[0]).powi(2) + (b[1] - p0[1]).powi(2);
            da.total_cmp(&db)
        })
        .unwrap_or([mid[0], lo[1] - 200.0]);
    flatten(&mut land, pits, 95.0, 45.0, LOOSE as u8);
    land.compute_steep();
    let mut models = Vec::new();
    // Fences down the start straight, banners by the gate.
    let mut s = line - 40.0;
    let mut panel = 0usize;
    const FENCES: [&str; 4] = ["fp_fence", "fp_fence_2", "fp_fence_3", "fp_fence_4"];
    while s < line + (sl * 0.7).min(320.0) {
        let (p, d) = course.at(s);
        let left = [-d[1], d[0]];
        // Out round the wide start, then in along the track.
        let ahead = (s - line - 15.0).max(0.0);
        let off = half + (START_HALF - half) * (1.0 - smooth(ahead / 60.0)) + 6.0;
        // A run of one sponsor's mesh, then the next.
        for (k, sign) in [-1.0f32, 1.0].into_iter().enumerate() {
            models.push(model(FENCES[(panel / 6 + k * 2) % 4], &land, p[0] + left[0] * off * sign, p[1] + left[1] * off * sign, d));
        }
        panel += 1;
        s += 6.5;
    }
    for sign in [-1.0f32, 1.0] {
        let left = [-d0[1], d0[0]];
        let (x, z) = (p0[0] + left[0] * (START_HALF + 14.0) * sign, p0[1] + left[1] * (START_HALF + 14.0) * sign);
        models.push(model("fp_banner", &land, x, z, [left[0] * -sign, left[1] * -sign]));
    }
    // Bales round the outside of the tight turns.
    for (_, p, out, radius) in apexes(&course, 70.0) {
        let across = [-out[1], out[0]];
        for k in -1..=1 {
            let off = half + 4.0 + radius.min(40.0) * 0.05;
            let (x, z) = (p[0] + out[0] * off + across[0] * k as f32 * 9.0, p[1] + out[1] * off + across[1] * k as f32 * 9.0);
            let name = format!("triplebale0{}", 1 + (rng.next() * 8.0) as u32 % 8);
            models.push(model(&name, &land, x, z, across));
        }
    }
    // A flagman beside each big jump's take-off.
    for (at, shape) in &features.0 {
        if matches!(shape, Shape::Gap { .. } | Shape::Table { .. }) {
            let (p, d) = course.at(*at);
            let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
            let left = [-d[1] * side, d[0] * side];
            models.push(model("fp_flagman_stand", &land, p[0] + left[0] * (half + 10.0), p[1] + left[1] * (half + 10.0), [-left[0], -left[1]]));
        }
    }
    // The podium stands in the middle of the pits: the tents keep clear of it.
    scatter_props(&land, pits, 85.0, PODIUM_CLEAR, &mut rng, &["tent01", "tent02", "nattrailer01", "nattrailer01", "toilet", "fp_banner"], 22, &mut models);
    national_dressing(&course, &land, half, line, &features, pits, centre, &mut rng, &mut models);
    let start = start_at(&course, &land, line);
    // The finish arch stands up the straight, clear of the wide start.
    let finish = (line + (sl * 0.7 - 20.0).clamp(0.0, 115.0)) % course.length;
    let infield = ([lo[0] - 50.0, lo[1] - 50.0], [hi[0] + 50.0, hi[1] + 50.0]);
    Made { land, courses: vec![(course, half + 9.0, Vec::new(), String::new())], lap_lines: Some((finish, line, START_HALF)), start, models, keep: vec![(pits, 120.0)], podium: Some(([pits[0], 0.0, pits[1]], d0)), infield: Some(infield) }
}

/// The Free Pack's race-day banners, each its own sponsor.
const RACE_BANNERS: [&str; 6] = ["fp_banner_2", "fp_banner_3", "fp_banner_4", "fp_banner_5", "fp_banner_6", "fp_banner_7"];

/// What makes a national look like race day, as the game's own tracks do:
/// sponsor banners down the straights and round the turns, speaker and
/// light poles by the start, a TV platform at the big jumps, the teams'
/// trucks behind the pits, a blimp overhead and a line of pylons across
/// the hills. All of it well clear of the track.
fn national_dressing(course: &Course, land: &Land, half: f32, line: f32, features: &Features, pits: [f32; 2], centre: [f32; 2], rng: &mut Rng, models: &mut Vec<Model>) {
    let span = land.span();
    let clear = |p: [f32; 2], room: f32| course.near(p, room).is_none() && p[0] > 60.0 && p[1] > 60.0 && p[0] < span - 60.0 && p[1] < span - 60.0;
    // Banners: on the outside of each turn and every so often down the
    // straights, facing the riders.
    let mut s = 0.0;
    let mut last_side = 1.0f32;
    while s < course.length {
        let (p, d) = course.at(s);
        let near = course.near(p, 1.0).map_or(0, |n| n.index);
        let curve = course.curve[near];
        // Outside of a turn, else alternate sides.
        let side = if curve.abs() > 0.004 { -curve.signum() } else { -last_side };
        last_side = side;
        let left = [-d[1] * side, d[0] * side];
        let off = half + 16.0 + rng.range(0.0, 6.0);
        let at = [p[0] + left[0] * off, p[1] + left[1] * off];
        if clear(at, off - 3.0) && ((at[0] - pits[0]).powi(2) + (at[1] - pits[1]).powi(2)).sqrt() > 130.0 {
            models.push(model(RACE_BANNERS[(rng.next() * 6.0) as usize % 6], land, at[0], at[1], [-left[0], -left[1]]));
            // A sponsor's feather flags either side of its banner.
            let flag = 1 + (rng.next() * 4.0) as usize % 4;
            for along in [-12.0f32, 12.0] {
                let q = [at[0] + d[0] * along + left[0] * 2.0, at[1] + d[1] * along + left[1] * 2.0];
                if clear(q, off - 2.0) {
                    models.push(model(&format!("fp_flag_{flag}"), land, q[0], q[1], d));
                }
            }
        }
        s += rng.range(60.0, 100.0);
    }
    // An inflatable sponsor arch over the track on a quiet straight,
    // half a lap from the start, as race day puts up.
    let mut s = line + course.length * 0.45;
    for _ in 0..40 {
        let at = s.rem_euclid(course.length);
        let near = course.near(course.at(at).0, 1.0).map_or(0, |n| n.index);
        let quiet = [-40.0f32, 0.0, 40.0].iter().all(|o| features.height((at + o).rem_euclid(course.length), course.length) < 0.05);
        if quiet && course.curve[near].abs() < 0.003 {
            let (p, d) = course.at(at);
            let mut arch = model("waypnt", land, p[0], p[1], d);
            arch.scale = (half + 6.0) / 18.5;
            models.push(arch);
            break;
        }
        s += 25.0;
    }
    // Speaker and light poles down the start straight, behind the fences.
    let mut s = line - 30.0;
    for k in 0..4 {
        let (p, d) = course.at(s);
        let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
        let left = [-d[1] * sign, d[0] * sign];
        let off = START_HALF + 22.0;
        let at = [p[0] + left[0] * off, p[1] + left[1] * off];
        if clear(at, off - 4.0) {
            models.push(model(if k < 2 { "speakerpole" } else { "lightpole" }, land, at[0], at[1], [-left[0], -left[1]]));
        }
        s += 55.0;
    }
    // Feather flags behind the fences down the start straight.
    let mut s = line - 20.0;
    for k in 0..6 {
        let (p, d) = course.at(s);
        let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
        let left = [-d[1] * sign, d[0] * sign];
        let off = START_HALF + 12.0;
        let at = [p[0] + left[0] * off, p[1] + left[1] * off];
        if clear(at, off - 4.0) {
            models.push(model(&format!("fp_flag_{}", 1 + k % 4), land, at[0], at[1], d));
        }
        s += 24.0;
    }
    // A TV camera platform beside the biggest jump.
    if let Some((at, _)) = features.0.iter().find(|(_, sh)| matches!(sh, Shape::Gap { .. } | Shape::Table { .. })) {
        let (p, d) = course.at(*at + 25.0);
        for sign in [1.0f32, -1.0] {
            let left = [-d[1] * sign, d[0] * sign];
            let off = half + 34.0;
            let q = [p[0] + left[0] * off, p[1] + left[1] * off];
            if clear(q, off - 6.0) {
                models.push(model("cameraplatform01", land, q[0], q[1], [-left[0], -left[1]]));
                break;
            }
        }
    }
    // The teams' trucks lined up behind the pits, away from the track.
    let away = {
        let (dx, dz) = (pits[0] - centre[0], pits[1] - centre[1]);
        let l = (dx * dx + dz * dz).sqrt().max(1.0);
        [dx / l, dz / l]
    };
    let across = [-away[1], away[0]];
    for k in -2i32..=2 {
        let at = [pits[0] + away[0] * 120.0 + across[0] * k as f32 * 26.0, pits[1] + away[1] * 120.0 + across[1] * k as f32 * 26.0];
        if clear(at, 60.0) {
            let name = ["semitrailer", "pit_trailer", "watertruck", "pit_trailer", "companytruck"][(k + 2) as usize];
            models.push(model(name, land, at[0], at[1], [-away[0], -away[1]]));
        }
    }
    // A blimp over the track, and a line of pylons across the far hills.
    let a = rng.range(0.0, TAU);
    let b = [centre[0] + a.cos() * 350.0, centre[1] + a.sin() * 350.0];
    let mut blimp = model("blimp", land, b[0], b[1], [a.sin(), -a.cos()]);
    blimp.at[1] = land.at(centre[0], centre[1]) + 320.0;
    models.push(blimp);
    let a = rng.range(0.0, TAU);
    let (dir, out) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let base = [centre[0] + out[0] * span * 0.36, centre[1] + out[1] * span * 0.36];
    for k in -4i32..=4 {
        let at = [base[0] + dir[0] * k as f32 * 420.0, base[1] + dir[1] * k as f32 * 420.0];
        if clear(at, 250.0) {
            models.push(model(if k % 3 == 0 { "ht_tower02" } else { "ht_tower" }, land, at[0], at[1], dir));
        }
    }
}

/// Half the start's width, ft: eight bikes 7.75 ft apart, set 3 ft left of
/// the middle, with room either side.
const START_HALF: f32 = 36.0;

/// The same in a supercross stadium, where the lanes are closer: the gate
/// spills into the gaps between them.
const SX_START_HALF: f32 = 33.0;

fn start_at(course: &Course, land: &Land, s: f32) -> ([f32; 3], [f32; 2]) {
    let (p, d) = course.at(s);
    ([p[0], land.at(p[0], p[1]) + 4.0, p[1]], d)
}

/// Levels a round patch of `radius` ft to its middle's height, with a
/// `blend` ft edge, and paints it.
fn flatten(land: &mut Land, at: [f32; 2], radius: f32, blend: f32, material: u8) {
    let h = land.at(at[0], at[1]);
    let n = land.size;
    let r = radius + blend;
    let (x0, x1) = (((at[0] - r) / land.scale).max(0.0) as usize, (((at[0] + r) / land.scale) as usize).min(n - 1));
    let (z0, z1) = (((at[1] - r) / land.scale).max(0.0) as usize, (((at[1] + r) / land.scale) as usize).min(n - 1));
    for z in z0..=z1 {
        for x in x0..=x1 {
            let d = ((x as f32 * land.scale - at[0]).powi(2) + (z as f32 * land.scale - at[1]).powi(2)).sqrt();
            let t = 1.0 - smooth((d - radius) / blend);
            let i = z * n + x;
            if land.track[i] > 0.0 {
                continue;
            }
            land.heights[i] = lerp(land.heights[i], h, t);
            if t > land.patch[i].1 {
                land.patch[i] = (material, t * 0.9);
            }
        }
    }
}

/// Props strewn round `at` out to `radius`, none nearer than `clear` (ft).
fn scatter_props(land: &Land, at: [f32; 2], radius: f32, clear: f32, rng: &mut Rng, names: &[&str], count: usize, models: &mut Vec<Model>) {
    for _ in 0..count {
        let a = rng.range(0.0, TAU);
        let r = clear + rng.range(0.15, 0.9) * (radius - clear);
        let (x, z) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
        let look = rng.range(0.0, TAU);
        models.push(Model { name: rng.pick(names).to_string(), at: [x, land.at(x, z), z], look: [look.cos(), look.sin()], scale: 1.0 });
    }
}

/// How far the pits' tents and trailers keep from the podium in their
/// middle (its stage is 26 ft wide, a tent's roof about 20).
const PODIUM_CLEAR: f32 = 36.0;

/// How much bigger than the art's own the free stadium is drawn.
const ARENA_SCALE: f32 = 1.25;
/// The stands' front wall round the stadium floor, half sizes in ft.
/// (fp_arena is built at full size: its front wall's LED ribbon stands
/// at 388.7 by 201.2 ft.)
const SX_APRON: [f32; 2] = [388.0, 200.0];

/// A stadium floor: the track folds over it in lanes.
fn supercross(def: &Def) -> Made {
    let mut rng = Rng(def.seed.wrapping_mul(104_729));
    let mut land = Land::new(Kind::Supercross);
    let span = land.span();
    let (cx, cz) = (span / 2.0, span / 2.0);
    let floor = 4.0;
    // The floor inside the stadium's ad wall.
    let (hx, hz) = (300.0 * ARENA_SCALE - 6.0, 150.0 * ARENA_SCALE - 6.0);
    let n = land.size;
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 * land.scale - cx, z as f32 * land.scale - cz);
            let out = ((fx.abs() - hx).max(fz.abs() - hz)).max(0.0);
            // A flat concrete apron from the padded wall out to the
            // stands' front wall, as a real stadium floor, then down out
            // of sight under the stands.
            let behind = ((fx.abs() - SX_APRON[0]).max(fz.abs() - SX_APRON[1])).max(0.0);
            land.heights[z * n + x] = floor - if behind > 0.0 { 12.0 * smooth(behind / 4.0) } else { 0.0 };
            if out > 0.0 {
                land.patch[z * n + x] = (CONCRETE as u8, 1.0);
            }
        }
    }
    let cell = 54.0;
    let long = cell * 2.0;
    let fine_x = ((2.0 * (hx - 16.0)) / long) as i32;
    let fine_z = ((2.0 * (hz - 16.0)) / cell) as i32;
    let (nx, nz) = ((fine_x + 1) / 2, (fine_z + 1) / 2);
    let mut course = None;
    for attempt in 0..400 {
        let nodes = rng.range((nx * nz) as f32 * 0.6, (nx * nz) as f32 + 0.99) as usize;
        let outline = tree_outline(&mut rng, nx, nz, nodes, 2.0);
        let turn = if rng.next() < 0.5 { 0.0 } else { PI };
        let c = outline_course(&outline, [long, cell], [cx, cz], turn, 0.0, def.seed + attempt);
        if (3200.0..4800.0).contains(&c.length) && c.tightest() >= cell * 0.4 {
            course = Some(c);
            break;
        }
    }
    let course = course.expect("a supercross layout");
    let runs = straights(&course, 120.0);
    // The start: on the longest straight with room for a full gate across
    // it, clear of the walls (the original's starts are 140 ft wide).
    let inside = |s: f32| {
        let (p, d) = course.at(s);
        let left = [-d[1], d[0]];
        [-1.0f32, 1.0].iter().all(|&k| {
            let (x, z) = (p[0] + left[0] * SX_START_HALF * k, p[1] + left[1] * SX_START_HALF * k);
            (x - cx).abs() < hx - 2.0 && (z - cz).abs() < hz - 2.0
        })
    };
    let mut by_length = runs.clone();
    by_length.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (line, run) = by_length
        .iter()
        .map(|&(s0, len)| ((s0 + 20.0) % course.length, len))
        .find(|&(l, _)| (-20..=2).all(|k| inside(l + k as f32 * 4.0)))
        .unwrap_or_else(|| {
            let (s0, len) = longest(&runs);
            ((s0 + 20.0) % course.length, len)
        });
    // The finish arch up the straight, past the gate.
    let finish = (line + (run - 60.0).clamp(30.0, 120.0)) % course.length;
    let features = fill_straights(&course, &runs, &mut rng, &mut |r, room| pick_shape(r, room, SX_JUMPS), 24.0, 20.0, (8.0, 22.0), Some(((line - 40.0).rem_euclid(course.length), 200.0)));
    let half = 12.0;
    land.carve(&course, half, 6.0, &|_| floor + 0.3, &features, 3.0, 0.0, None);
    land.widen_start(&course, line, half, SX_START_HALF, 4.0, &|_| floor + 0.3);
    land.compute_steep();
    let mut models = vec![Model { name: "fp_arena".into(), at: [cx, floor, cz], look: [0.0, 1.0], scale: 1.0 }];
    // Tuff blocks down the middle between lanes, and along the outer
    // edges.
    let mut placed: Vec<[f32; 2]> = Vec::new();
    let mut s = 0.0;
    while s < course.length {
        let (p, d) = course.at(s);
        let left = [-d[1], d[0]];
        for sign in [-1.0f32, 1.0] {
            for off in [cell / 2.0, half + 7.0] {
                let (x, z) = (p[0] + left[0] * off * sign, p[1] + left[1] * off * sign);
                if (x - cx).abs() > hx - 4.0 || (z - cz).abs() > hz - 4.0 {
                    continue;
                }
                let i = (z / land.scale) as usize * n + (x / land.scale) as usize;
                if land.track[i] < 0.02 && land.heights[i] < floor + 1.0 && !placed.iter().any(|q| (q[0] - x).powi(2) + (q[1] - z).powi(2) < 64.0) {
                    placed.push([x, z]);
                    // Each stretch in one sponsor's colours.
                    let name = ["fp_tuffblock", "fp_tuffblock", "fp_tuffblock_b", "fp_tuffblock", "fp_tuffblock_c"][(s / 180.0) as usize % 5];
                    models.push(Model { name: name.into(), at: [x, floor, z], look: d, scale: 1.0 });
                }
                break;
            }
        }
        s += 9.0;
    }
    sx_dressing(&course, &land, &features, line, [cx, cz], [hx, hz], floor, half, &mut rng, &mut models);
    sx_banners(&course, line, [cx, cz], [hx, hz], floor, &mut models);
    let start = start_at(&course, &land, line);
    Made { land, courses: vec![(course, half + 6.0, Vec::new(), String::new())], lap_lines: Some((finish, line, SX_START_HALF)), start, models, keep: Vec::new(), podium: Some(([cx, floor, cz + hz - 40.0], [0.0, -1.0])), infield: None }
}

/// A supercross night's trimmings: the padded wall round the floor, the
/// start gate, a flagger at each big jump, TV cameras and their towers on
/// the apron, the mechanics' area along one side, and the video board
/// hung over the middle. The kit's models face their +z, which the
/// viewer turns away from `look`, so each is given the way its back
/// faces; the flagger faces his -z, and is given the way he looks.
#[allow(clippy::too_many_arguments)]
fn sx_dressing(course: &Course, land: &Land, features: &Features, line: f32, centre: [f32; 2], inner: [f32; 2], floor: f32, half: f32, rng: &mut Rng, models: &mut Vec<Model>) {
    let [cx, cz] = centre;
    let [hx, hz] = inner;
    let at = |name: &str, x: f32, y: f32, z: f32, front: [f32; 2]| Model { name: name.into(), at: [x, y, z], look: [-front[0], -front[1]], scale: 1.0 };
    // The padded wall, in 40 ft pieces facing in.
    const PAD: f32 = 40.0;
    for (len, fixed, along_x) in [(hx, hz, true), (hx, -hz, true), (hz, hx, false), (hz, -hx, false)] {
        let count = (2.0 * len / PAD).round().max(1.0) as i32;
        let step = 2.0 * len / count as f32;
        let inward = -fixed.signum();
        for k in 0..count {
            let t = -len + step * (k as f32 + 0.5);
            let (x, z, look) = if along_x { (cx + t, cz + fixed + 1.0 * -inward, [0.0, inward]) } else { (cx + fixed + 1.0 * -inward, cz + t, [inward, 0.0]) };
            // Runs of one sponsor's livery, as a real wall is hung.
            let livery = ["fp_wall_pad", "fp_wall_pad_b", "fp_wall_pad_c", "fp_wall_pad_d"][((k / 3) as u32 + if along_x { 0 } else { 2 }) as usize % 4];
            models.push(at(livery, x, floor, z, look));
        }
    }
    // The start gate across the start straight, its drop bars just ahead
    // of the riders' front wheels and its pad under them (the model's
    // front faces the way they ride).
    let (p, d) = course.at(line);
    models.push(at("fp_startgate", p[0] + d[0] * 3.5, floor + 0.3, p[1] + d[1] * 3.5, d));
    // A flagger beside each big jump's take-off.
    for (s, shape) in &features.0 {
        if matches!(shape, Shape::Gap { .. } | Shape::Table { .. }) {
            let (p, d) = course.at(*s);
            let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
            let left = [-d[1] * side, d[0] * side];
            let (x, z) = (p[0] + left[0] * (half + 9.0), p[1] + left[1] * (half + 9.0));
            if (x - cx).abs() < hx - 3.0 && (z - cz).abs() < hz - 3.0 && land.sample(&land.track, x, z) < 0.05 {
                models.push(Model { name: "fp_flagger".into(), at: [x, land.at(x, z), z], look: [-left[0], -left[1]], scale: 1.0 });
            }
        }
    }
    // TV cameras round the apron looking in, and two camera towers at
    // the ends.
    let apron = [hx + 10.0, hz + 10.0];
    for k in 0..10 {
        let a = k as f32 / 10.0 * TAU + 0.3;
        let (dx, dz) = (a.cos(), a.sin());
        let r = (apron[0] / dx.abs().max(1e-3)).min(apron[1] / dz.abs().max(1e-3));
        models.push(at("fp_tv_camera", cx + dx * r, floor, cz + dz * r, [-dx, -dz]));
    }
    for sign in [-1.0f32, 1.0] {
        models.push(at("fp_camera_platform", cx + sign * (hx + 12.0), floor, cz, [-sign, 0.0]));
    }
    // The mechanics' area: canopies and toolboxes down the long side
    // away from the start.
    let side = if p[1] < cz { 1.0 } else { -1.0 };
    let z = cz + side * (hz + 12.0);
    let mut x = cx - hx + 60.0;
    while x < cx + hx - 60.0 {
        models.push(at("fp_mechanics_canopy", x, floor, z, [0.0, -side]));
        models.push(at("fp_toolbox", x + rng.range(-6.0, 6.0), floor, z - side * 5.0, [0.0, -side]));
        x += rng.range(34.0, 46.0);
    }
    // The lap board beside the finish, and light masts in the floor's
    // corners shining in.
    let left = [-d[1], d[0]];
    let (bx, bz) = (p[0] + left[0] * (SX_START_HALF + 6.0), p[1] + left[1] * (SX_START_HALF + 6.0));
    if (bx - cx).abs() < hx - 2.0 && (bz - cz).abs() < hz - 2.0 {
        models.push(at("fp_number_board", bx, floor, bz, [-left[0], -left[1]]));
    }
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let (x, z) = (cx + sx * (hx + 9.0), cz + sz * (hz + 9.0));
        let look = [-sx * std::f32::consts::FRAC_1_SQRT_2, -sz * std::f32::consts::FRAC_1_SQRT_2];
        models.push(at("fp_lightrig", x, floor, z, look));
    }
    // The video board over the middle of the floor.
    models.push(at("fp_videoboard", cx, floor, cz, [0.0, 1.0]));
}

/// Big sponsor banners on the apron in front of the stands, facing the
/// floor, so the stadium has colour round it as a real one hung for race
/// night does: down the long side across from the mechanics, and across
/// both ends, clear of the corner light masts.
fn sx_banners(course: &Course, line: f32, centre: [f32; 2], inner: [f32; 2], floor: f32, models: &mut Vec<Model>) {
    let [cx, cz] = centre;
    let [hx, hz] = inner;
    let (p, _) = course.at(line);
    // The mechanics take the long side away from the start.
    let side = if p[1] < cz { -1.0 } else { 1.0 };
    let mut k = 0usize;
    let mut banner = |x: f32, z: f32, look: [f32; 2]| {
        // Runs of two of a sponsor, then the next.
        k += 1;
        Model { name: RACE_BANNERS[(k / 2) % 6].into(), at: [x, floor, z], look, scale: 1.7 }
    };
    let mut x = -hx + 50.0;
    while x <= hx - 50.0 {
        models.push(banner(cx + x, cz + side * (hz + 15.0), [0.0, -side]));
        x += 46.0;
    }
    for end in [-1.0f32, 1.0] {
        let mut z = -hz + 45.0;
        while z <= hz - 45.0 {
            models.push(banner(cx + end * (hx + 15.0), cz + z, [-end, 0.0]));
            z += 46.0;
        }
    }
}

/// The arena's lamps for night races (`<track>.lights`, beside the
/// `.ENV`): one per line, `x y z  dx dy dz  power  beam`, game feet, the
/// way it points, 1.0 a typical stadium bank, cone half-angle in degrees.
/// The four corner masts (SX only, as sx_dressing places them) and the
/// roof's catwalk ring, all aimed at the floor.
fn arena_lights(land: &Land, kind: Kind) -> String {
    use std::fmt::Write;
    let span = land.span();
    let (cx, cz) = (span / 2.0, span / 2.0);
    let floor = 4.0;
    let (hx, hz) = (300.0 * ARENA_SCALE - 6.0, 150.0 * ARENA_SCALE - 6.0);
    let mut o = String::from("# Free Pack arena lamps: x y z  dx dy dz  power  beam (mm2-tools freetracks)\n");
    let mut lamp = |o: &mut String, at: [f32; 3], aim: [f32; 3], power: f32, beam: f32| {
        let d = [aim[0] - at[0], aim[1] - at[1], aim[2] - at[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
        let _ = writeln!(o, "{:.1} {:.1} {:.1}  {:.3} {:.3} {:.3}  {power:.2}  {beam:.0}", at[0], at[1], at[2], d[0] / l, d[1] / l, d[2] / l);
    };
    if kind == Kind::Supercross {
        for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let at = [cx + sx * (hx + 9.0), floor + 28.0, cz + sz * (hz + 9.0)];
            lamp(&mut o, at, [cx + sx * hx * 0.45, floor, cz + sz * hz * 0.45], 0.6, 35.0);
        }
    }
    // The catwalk ring in fp_arena's roof: lamp heads every 16 ft in two
    // rows round a rectangle, 151 ft up. 20 banks, each on one of the
    // inner row's heads (game x and z from the arena's middle, read off
    // the model), aimed at the floor in from below it.
    const RING: [[f32; 2]; 20] = [[400.5, 8.5], [400.5, -119.5], [365.0, -213.0], [237.0, -213.0], [109.0, -213.0], [-3.0, -213.0], [-131.0, -213.0], [-259.0, -213.0], [-371.0, -213.0], [-400.5, -120.5], [-400.5, 7.5], [-400.5, 119.5], [-365.0, 213.0], [-253.0, 213.0], [-125.0, 213.0], [3.0, 213.0], [115.0, 213.0], [243.0, 213.0], [371.0, 213.0], [400.5, 120.5]];
    for [x, z] in RING {
        let at = [cx + x, floor + 151.0, cz + z];
        let aim = [cx + x * 0.55, floor, cz + z * 0.55];
        lamp(&mut o, at, aim, 1.0, 40.0);
    }
    o
}

/// Open country with long loops through it: Baja (desert roads) and
/// enduro (trails through the woods).
fn open_country(def: &Def) -> Made {
    let mut rng = Rng(def.seed.wrapping_mul(15_485_863));
    let mut land = Land::new(def.kind);
    let span = land.span();
    let seed = def.seed;
    let n = land.size;
    let desert = def.biome == Biome::Desert;
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 * land.scale, z as f32 * land.scale);
            let big = fbm(fx / 3000.0, fz / 3000.0, seed);
            let mid = fbm(fx / 900.0, fz / 900.0, seed + 1);
            let h = if desert {
                // Mesas: flat tops and steps, and dry washes cut in.
                let m = fbm(fx / 1800.0, fz / 1800.0, seed + 3);
                let steps = (m * 5.0).floor() + smooth((m * 5.0).fract() * 3.0 - 2.0);
                let wash = ridged(fx / 2500.0, fz / 2500.0, seed + 4);
                // The originals' deserts rise about 230 ft.
                steps * 36.0 + big * 100.0 - smooth((wash - 0.7) / 0.2) * 18.0 + mid * 12.0
            } else {
                let ridge = ridged(fx / 2600.0, fz / 2600.0, seed + 2);
                big * 220.0 + ridge * 140.0 + mid * 40.0
            };
            land.heights[z * n + x] = h;
        }
    }
    // Three courses, of different sizes, round the middle.
    let names = race_names(def);
    let mut courses = Vec::new();
    let half = if desert { 16.0 } else { 9.0 };
    for (k, name) in names.into_iter().enumerate() {
        let off = [rng.range(-0.08, 0.08) * span, rng.range(-0.08, 0.08) * span];
        let size = [0.24, 0.31, 0.37][k] * span * rng.range(0.9, 1.05);
        let squash = rng.range(0.75, 0.95);
        let base = star_loop(&mut rng, [span / 2.0 + off[0], span / 2.0 + off[1]], [size, size * squash], if desert { 90.0 } else { 55.0 }, 120.0);
        // Bend the loop so it wanders the way a trail does, round hills
        // and along washes, keeping its turns rideable.
        let base_points = base.points.clone();
        let mut course = base;
        for attempt in 0..40u32 {
            let amp = if desert { 150.0 } else { 120.0 } * (1.0 - attempt as f32 / 40.0);
            let wave = if desert { 800.0 } else { 600.0 };
            let s2 = seed * 31 + k as u32 * 7 + attempt;
            let bent: Vec<[f32; 2]> = base_points
                .iter()
                .map(|p| [p[0] + (fbm(p[0] / wave, p[1] / wave, s2) - 0.5) * 2.0 * amp, p[1] + (fbm(p[0] / wave, p[1] / wave, s2 + 1) - 0.5) * 2.0 * amp])
                .collect();
            let c = Course::new(&bent, 4.0);
            if c.tightest() >= if desert { 55.0 } else { 40.0 } && !c.crosses(110.0) {
                course = c;
                break;
            }
        }
        let gates_n = (course.length / if desert { 1100.0 } else { 800.0 }).round().clamp(6.0, 14.0) as usize;
        let gates: Vec<f32> = (1..=gates_n).map(|g| course.length * g as f32 / gates_n as f32 - 1.0).collect();
        courses.push((course, half, gates, name));
    }
    {
        let lines: Vec<&Course> = courses.iter().map(|c| &c.0).collect();
        if desert {
            land.valley(&lines, 40.0, 330.0, 110.0, 18.0, 160.0, seed);
        } else {
            land.valley(&lines, 30.0, 300.0, 100.0, 20.0, 200.0, seed);
        }
        // Open country worth riding off the trail: rolls, dips and dry
        // gullies across the ground round it, all smooth enough to ride
        // over, coming in from just past the trail.
        let dist = land.course_distance(&lines);
        let n = land.size;
        for z in 0..n {
            for x in 0..n {
                let i = z * n + x;
                let (fx, fz) = (x as f32 * land.scale, z as f32 * land.scale);
                let away = smooth((dist[i] - 55.0) / 90.0);
                if away <= 0.0 {
                    continue;
                }
                let rolls = (fbm(fx / 85.0, fz / 85.0, seed + 51) - 0.5) * 11.0;
                let g = (noise(fx / 240.0, fz / 240.0, seed + 52) * 2.0 - 1.0).abs();
                let gully = -(1.0 - smooth(g / 0.18)) * 6.0;
                let humps = (fbm(fx / 28.0, fz / 28.0, seed + 53) - 0.5) * 2.6;
                land.heights[i] += (rolls + gully + humps) * away * if desert { 0.8 } else { 1.0 };
            }
        }
    }
    for (course, half, gates, _) in &courses {
        let raw = land.profile(course, if desert { 240.0 } else { 140.0 });
        let profile = profile_fn(course, raw);
        let mut make = |r: &mut Rng| match (r.next() * 3.0) as u32 {
            0 => Shape::Rollers { count: r.range(3.0, 6.0) as u32, spacing: r.range(40.0, 60.0), height: r.range(2.5, 4.0) },
            1 => Shape::Whoops { count: r.range(6.0, 12.0) as u32, spacing: r.range(14.0, 20.0), height: r.range(1.5, 2.5) },
            _ => Shape::Table { up: r.range(30.0, 40.0), top: r.range(30.0, 50.0), down: r.range(40.0, 60.0), height: r.range(6.0, 10.0) },
        };
        let features = place_features(course, &mut rng, &mut make, 400.0, 300.0, 600.0);
        land.carve(course, *half, if desert { 40.0 } else { 26.0 }, &profile, &features, 3.0, 0.0, None);
        let start_s = *gates.last().unwrap_or(&0.0);
        land.widen_start(course, start_s, *half, START_HALF, if desert { 40.0 } else { 20.0 }, &profile);
    }
    land.compute_steep();
    let (course, _, gates, _) = &courses[0];
    let start_s = *gates.last().unwrap_or(&0.0);
    let start = start_at(course, &land, start_s);
    let mut models = Vec::new();
    let camp = {
        let (p, d) = course.at(start_s);
        [p[0] - d[1] * 90.0, p[1] + d[0] * 90.0]
    };
    flatten(&mut land, camp, 60.0, 30.0, LOOSE as u8);
    scatter_props(&land, camp, 55.0, 0.0, &mut rng, &["tent01", "nattrailer01", "toilet"], 6, &mut models);
    Made { land, courses, lap_lines: None, start, models, keep: vec![(camp, 80.0)], podium: None, infield: None }
}

pub fn race_names(def: &Def) -> Vec<String> {
    let parts: [&str; 3] = match def.kind {
        Kind::Baja => ["Short Run", "Long Haul", "Outer Loop"],
        _ => ["Trail One", "Trail Two", "The Big Loop"],
    };
    parts.iter().map(|p| format!("{} - {p}", def.name)).collect()
}

/// A quarry: a deep pit with benches cut in its walls, ramps and big
/// landings on its floor, and banks to ride up.
fn quarry(def: &Def) -> Made {
    let mut rng = Rng(def.seed.wrapping_mul(32_452_843));
    let mut land = Land::new(Kind::Quarry);
    let span = land.span();
    let seed = def.seed;
    let n = land.size;
    let (cx, cz) = (span / 2.0, span / 2.0);
    let radius = rng.range(900.0, 1300.0);
    let depth = rng.range(90.0, 150.0);
    let benches = rng.range(4.0, 7.0);
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 * land.scale, z as f32 * land.scale);
            let (dx, dz) = ((fx - cx) / radius, (fz - cz) / (radius * 0.75));
            let wob = (fbm(fx / 700.0, fz / 700.0, seed) - 0.5) * 0.35;
            let r = (dx * dx + dz * dz).sqrt() + wob;
            // Down in steps from the rim to the floor.
            let t = smooth((r - 0.35) / 0.7);
            let stepped = (t * benches).floor() + smooth(((t * benches).fract() - 0.75) / 0.25);
            let pit = -depth * (1.0 - stepped / benches);
            let outside = fbm(fx / 1500.0, fz / 1500.0, seed + 1) * 120.0;
            let floor_noise = fbm(fx / 150.0, fz / 150.0, seed + 2) * 6.0;
            land.heights[z * n + x] = if r < 1.05 { pit + floor_noise * (1.0 - t) } else { pit } + outside * smooth((r - 0.9) / 0.6);
        }
    }
    // A line round the floor for the computer riders, with the big
    // jumps on it; more ramps off it to find.
    let course = star_loop(&mut rng, [cx, cz], [radius * 0.42, radius * 0.42 * 0.75], 80.0, 150.0);
    let runs = straights(&course, 260.0);
    let features = fill_straights(&course, &runs, &mut rng, &mut |r, room| pick_shape(r, room, QUARRY_JUMPS), 40.0, 40.0, (40.0, 120.0), None);
    {
        let raw = land.profile(&course, 200.0);
        let profile = profile_fn(&course, raw);
        land.carve(&course, 16.0, 30.0, &profile, &features, 4.0, 0.6, None);
    }
    let mut models = Vec::new();
    for k in 0..rng.range(4.0, 8.0) as usize {
        let a = rng.range(0.0, TAU);
        let r = rng.range(0.0, 0.4) * radius;
        let at = [cx + a.cos() * r, cz + a.sin() * r * 0.75];
        if course.near(at, 160.0).is_some() {
            continue;
        }
        let heading = rng.range(0.0, TAU);
        let d = [heading.cos(), heading.sin()];
        let shape = if k % 3 == 0 {
            Shape::Table { up: rng.range(30.0, 45.0), top: rng.range(40.0, 80.0), down: rng.range(40.0, 60.0), height: rng.range(10.0, 18.0) }
        } else {
            Shape::Gap { up: rng.range(26.0, 36.0), gap: rng.range(70.0, 130.0), down: rng.range(40.0, 60.0), height: rng.range(9.0, 16.0) }
        };
        stamp(&mut land, at, d, rng.range(20.0, 36.0), &shape);
    }
    land.compute_steep();
    // Dirt where the ground is worked; gravel and rock elsewhere.
    for i in 0..n * n {
        let z = (i / n) as f32 * land.scale;
        let x = (i % n) as f32 * land.scale;
        let (dx, dz) = ((x - cx) / radius, (z - cz) / (radius * 0.75));
        if (dx * dx + dz * dz).sqrt() < 0.55 {
            land.patch[i] = (LOOSE as u8, 0.6);
        }
    }
    let start = start_at(&course, &land, 0.0);
    scatter_props(&land, [cx, cz], 60.0, 0.0, &mut rng, &["nattrailer01", "gianttire01", "triplebale02"], 8, &mut models);
    Made { land, courses: vec![(course, 22.0, Vec::new(), String::new())], lap_lines: None, start, models, keep: vec![([cx, cz], radius * 0.6)], podium: None, infield: None }
}

/// Adds a jump shape to the land at `at`, facing `d`, `half` ft wide.
fn stamp(land: &mut Land, at: [f32; 2], d: [f32; 2], half: f32, shape: &Shape) {
    let shape = &shape.tamed();
    let len = shape.length();
    let n = land.size;
    let side = [-d[1], d[0]];
    let r = len + half + 20.0;
    let (x0, x1) = (((at[0] - r) / land.scale).max(0.0) as usize, (((at[0] + r) / land.scale) as usize).min(n - 1));
    let (z0, z1) = (((at[1] - r) / land.scale).max(0.0) as usize, (((at[1] + r) / land.scale) as usize).min(n - 1));
    for z in z0..=z1 {
        for x in x0..=x1 {
            let (px, pz) = (x as f32 * land.scale - at[0], z as f32 * land.scale - at[1]);
            let s = px * d[0] + pz * d[1];
            let w = (px * side[0] + pz * side[1]).abs();
            let fall = 1.0 - smooth((w - half) / 10.0);
            if fall <= 0.0 {
                continue;
            }
            let h = shape.height(s) * fall;
            let i = z * n + x;
            land.heights[i] += h;
            if h > 0.3 {
                land.track[i] = land.track[i].max(fall);
                land.wear[i] = land.wear[i].max(0.4 * fall);
            }
        }
    }
}

/// A tag arena: the stadium's floor, rolling, with mounds to chase over
/// and a loop round it that the computer riders follow.
fn tag(def: &Def) -> Made {
    let mut rng = Rng(def.seed.wrapping_mul(49_979_687));
    let mut land = Land::new(Kind::Tag);
    let span = land.span();
    let (cx, cz) = (span / 2.0, span / 2.0);
    let floor = 4.0;
    let (hx, hz) = (300.0 * ARENA_SCALE - 6.0, 150.0 * ARENA_SCALE - 6.0);
    let n = land.size;
    let seed = def.seed;
    for z in 0..n {
        for x in 0..n {
            let (fx, fz) = (x as f32 * land.scale - cx, z as f32 * land.scale - cz);
            let out = ((fx.abs() - hx).max(fz.abs() - hz)).max(0.0);
            let i = z * n + x;
            if out > 0.0 {
                let behind = ((fx.abs() - SX_APRON[0]).max(fz.abs() - SX_APRON[1])).max(0.0);
                land.heights[i] = floor - if behind > 0.0 { 12.0 * smooth(behind / 4.0) } else { 0.0 };
                land.patch[i] = (CONCRETE as u8, 1.0);
            } else {
                let b = fbm((fx + cx) / 110.0, (fz + cz) / 110.0, seed) - 0.5;
                let edge = smooth((hx - fx.abs()).min(hz - fz.abs()) / 40.0);
                land.heights[i] = floor + b * 10.0 * edge;
                land.track[i] = 1.0;
                land.wear[i] = (fbm((fx + cx) / 40.0, (fz + cz) / 40.0, seed + 9) - 0.3).max(0.0);
            }
        }
    }
    // The riders' loop, round the floor.
    let (ix, iz) = (hx - 55.0, hz - 50.0);
    let corners = [([cx - ix, cz - iz], 60.0), ([cx + ix, cz - iz], 60.0), ([cx + ix, cz + iz], 60.0), ([cx - ix, cz + iz], 60.0)];
    let course = Course::new(&rounded(&corners), 3.0);
    for _ in 0..rng.range(5.0, 9.0) as usize {
        let at = [cx + rng.range(-hx + 70.0, hx - 70.0), cz + rng.range(-hz + 60.0, hz - 60.0)];
        let heading = rng.range(0.0, TAU);
        let shape = if rng.next() < 0.6 {
            Shape::Table { up: 16.0, top: rng.range(10.0, 30.0), down: 18.0, height: rng.range(4.0, 7.0) }
        } else {
            Shape::Rollers { count: 3, spacing: 18.0, height: 3.0 }
        };
        stamp(&mut land, at, [heading.cos(), heading.sin()], rng.range(14.0, 26.0), &shape);
    }
    land.compute_steep();
    let mut models = vec![Model { name: "fp_arena".into(), at: [cx, floor, cz], look: [0.0, 1.0], scale: 1.0 }];
    // Tyre walls in the corners to bounce off.
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (x, z) = (cx + sx * (hx - 14.0), cz + sz * (hz - 14.0));
        let look = [-sx * 0.707, -sz * 0.707];
        models.push(Model { name: "fp_tirewall".into(), at: [x, land.at(x, z), z], look, scale: 1.0 });
    }
    let start = start_at(&course, &land, 20.0);
    Made { land, courses: vec![(course, 30.0, Vec::new(), String::new())], lap_lines: None, start, models, keep: Vec::new(), podium: Some(([cx, floor, cz + hz - 40.0], [0.0, -1.0])), infield: None }
}

// ---------------------------------------------------------------------

/// Makes one free track: its `.ENV`, and for arenas its `.lights`
/// (`(file name, bytes)`). `ground` reads one of the pack's ground
/// pictures (`grass.png`) by name; without them the ground is painted flat.
pub fn make(def: &Def, ground: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Vec<(String, Vec<u8>)>, Box<dyn std::error::Error>> {
    let mut made = match def.kind {
        Kind::National => national(def),
        Kind::Supercross => supercross(def),
        Kind::Baja | Kind::Enduro => open_country(def),
        Kind::Quarry => quarry(def),
        Kind::Tag => tag(def),
    };
    let land = &mut made.land;
    // Everything above sea level: the format's heights are unsigned.
    let low = land.heights.iter().copied().fold(f32::MAX, f32::min);
    if low < 1.0 {
        for h in &mut land.heights {
            *h += 1.0 - low;
        }
        for m in &mut made.models {
            m.at[1] += 1.0 - low;
        }
        made.start.0[1] += 1.0 - low;
        if let Some(p) = &mut made.podium {
            p.0[1] += 1.0 - low;
        }
    }
    let land = &made.land;
    let surfaces: Vec<u8> = (0..land.size * land.size)
        .map(|i| {
            if land.track[i] > 0.5 {
                0
            } else if land.steep[i] > 0.5 || land.patch[i].0 as usize == CONCRETE && land.patch[i].1 > 0.5 {
                2
            } else {
                1
            }
        })
        .collect();
    let ground = Ground::load(ground, def.kind.texture(), 64.0 * land.scale);
    let (textures, tiles) = paint(land, def.kind, def.biome, &ground, def.seed);
    let terrain = trn::Build { size: land.size, heights: &land.heights, surfaces: &surfaces, scale: land.scale, textures: &textures, tiles: &tiles }.write()?;

    let species = flora(def.biome);
    let keep: Vec<([f32; 2], f32)> = made.keep.iter().copied().chain(made.models.iter().map(|m| ([m.at[0], m.at[2]], 12.0))).collect();
    let plants = scatter(land, &species, plant_density(def.kind, def.biome), def.seed, &keep, if def.kind == Kind::Enduro { 10.0 } else { 24.0 }, made.infield);

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let stem = def.file;
    files.push((format!("{stem}.trn"), terrain));
    files.push((format!("{stem}.gmt"), ground_map(land, def.kind, def.biome, def.seed)));
    let multi = matches!(def.kind, Kind::Baja | Kind::Enduro);
    for (k, (course, half, gates, _)) in made.courses.iter().enumerate() {
        let name = if multi { format!("{stem}{:02}", k + 1) } else { stem.to_string() };
        let flags = if multi { 0x0780_0001 } else { 1 };
        files.push((format!("{name}.tdf"), write_tdf(course, *half, made.lap_lines, land, flags)));
        if multi {
            files.push((format!("{name}.wpt"), write_wpt(course, gates).into_bytes()));
        }
    }
    if !plants.is_empty() {
        files.push((format!("{stem}.esb"), write_esb(&species, &plants, land)));
    }
    let races = if multi { made.courses.iter().map(|c| c.3.clone()).collect() } else { Vec::new() };
    let scene = write_scene(&Scene { def, land, start: made.start, models: std::mem::take(&mut made.models), races, podium: made.podium });
    files.push((format!("{stem}.scn"), scene.into_bytes()));
    // Pictures for the event page, as the game's own tracks carry.
    crate::preview::add(stem, &mut files);
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    let mut out = vec![(format!("{stem}.ENV"), rs2::build(&refs))];
    if matches!(def.kind, Kind::Supercross | Kind::Tag) {
        out.push((format!("{stem}.lights"), arena_lights(land, def.kind).into_bytes()));
    }
    out.push((format!("{stem}.made"), MADE.as_bytes().to_vec()));
    Ok(out)
}

/// Which making of the recipes a built track is from (its `.made` file):
/// a track built by an older one is built again. Change it whenever a
/// change here moves anything on an existing track.
pub const MADE: &str = "4";

/// The free track whose file is `stem` (`FPNAT01`), any case.
pub fn find(stem: &str) -> Option<&'static Def> {
    SET.iter().find(|d| d.file.eq_ignore_ascii_case(stem))
}

/// Makes the free track `def` into `<game>/TERAFORM/<event>/`.
pub fn build_one(def: &Def, game: &Path, ground_dir: Option<&Path>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    eprint!("{} ({}): ", def.file, def.name);
    let read = |name: &str| std::fs::read(ground_dir?.join(name)).ok();
    let dir = game.join("TERAFORM").join(def.kind.folder());
    std::fs::create_dir_all(&dir)?;
    let mut env = None;
    for (name, bytes) in make(def, &read)? {
        let path = dir.join(&name);
        std::fs::write(&path, &bytes)?;
        if name.ends_with(".ENV") {
            eprintln!(" {} MB", bytes.len() / 1_000_000);
            env = Some(path);
        }
    }
    env.ok_or_else(|| "no .ENV made".into())
}

pub fn build(game: &Path, ground: Option<&Path>, only: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    for def in SET {
        if only.is_some_and(|o| !def.file.eq_ignore_ascii_case(o) && !def.kind.folder().eq_ignore_ascii_case(o)) {
            continue;
        }
        build_one(def, game, ground)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Noise helpers.

pub fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (z as u32).wrapping_mul(0x85eb_ca77) ^ seed.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

pub fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - xi as f32, z - zi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sz) = (s(fx), s(fz));
    let a = hash(xi, zi, seed) * (1.0 - sx) + hash(xi + 1, zi, seed) * sx;
    let b = hash(xi, zi + 1, seed) * (1.0 - sx) + hash(xi + 1, zi + 1, seed) * sx;
    a * (1.0 - sz) + b * sz
}

pub fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    (0..5).map(|o| noise(x * (1 << o) as f32, z * (1 << o) as f32, seed + o) / (1 << o) as f32).sum::<f32>() / 1.94
}

pub fn rgb555(r: f32, g: f32, b: f32) -> u16 {
    let c = |v: f32| ((v.clamp(0.0, 1.0) * 31.0).round() as u16) & 31;
    c(r) << 10 | c(g) << 5 | c(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_corners_make_a_closed_smooth_loop() {
        let square = [([0.0, 0.0], 20.0), ([200.0, 0.0], 20.0), ([200.0, 100.0], 20.0), ([0.0, 100.0], 20.0)];
        let c = Course::new(&rounded(&square), 3.0);
        assert!((c.length - (2.0 * 200.0 + 2.0 * 100.0 - 8.0 * 20.0 + TAU * 20.0)).abs() < 8.0, "{}", c.length);
        assert!(c.tightest() > 17.0, "{}", c.tightest());
        assert!(!c.crosses(30.0));
    }

    #[test]
    fn star_loops_keep_their_turns_wide() {
        let mut rng = Rng(5);
        let c = star_loop(&mut rng, [2000.0, 2000.0], [900.0, 700.0], 70.0, 90.0);
        assert!(c.tightest() >= 70.0);
        let near = c.near(c.points[10], 5.0).unwrap();
        assert!(near.distance < 0.5);
    }

    #[test]
    fn shapes_start_and_end_flat() {
        for s in [
            Shape::Table { up: 20.0, top: 30.0, down: 30.0, height: 6.0 },
            Shape::Gap { up: 20.0, gap: 50.0, down: 30.0, height: 6.0 },
            Shape::Whoops { count: 8, spacing: 12.0, height: 2.0 },
        ] {
            assert_eq!(s.height(-1.0), 0.0);
            assert!(s.height(s.length() + 1.0).abs() < 1e-3);
            assert!((0..100).map(|k| s.height(s.length() * k as f32 / 100.0)).fold(0.0, f32::max) > 1.5);
        }
    }

    #[test]
    fn eco_system_reads_back() {
        let land = Land::new(Kind::Supercross);
        let species = flora(Biome::Forest);
        let esb = write_esb(&species, &[(0, [100.0, 20.0, 300.0], 0, 255), (3, [500.0, 0.0, 50.0], 128, 10)], &land);
        let eco = mm2_formats::esb::parse(&esb).unwrap();
        assert_eq!(eco.species.len(), species.len());
        assert_eq!(eco.species[0].name, "pine01");
        assert_eq!(eco.objects.len(), 2);
        let unit = land.span() / 65536.0;
        assert!((eco.objects[0].position[0] as f32 * unit - 100.0).abs() < 0.1);
    }
}
