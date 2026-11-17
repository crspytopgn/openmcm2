//! Turning a [`Course`] into ground: the land, the track carved into it
//! along the line, the brush strokes, then the surfaces.
//!
//! The same bake makes the builder's quick preview (every few samples) and
//! the full terrain the game rides on, so what the builder shows is what
//! the track will be.

use crate::path::{Grid, Path};
use crate::{land, Brush, Course, Mode, Stroke, Surface};

/// Baked ground on a square grid of samples.
#[derive(Debug, Clone)]
pub struct Field {
    /// Samples a side.
    pub n: usize,
    /// Feet between samples.
    pub step: f32,
    /// Heights, ft, row-major with rows along +z.
    pub heights: Vec<f32>,
    /// Surface per sample ([`Surface::zone`]).
    pub surfaces: Vec<u8>,
    /// How far each sample is from the line, in half track widths (1 at
    /// the edge), or a large number away from it.
    pub track: Vec<f32>,
    /// The water's height over each sample in a river, else NaN.
    pub water: Vec<f32>,
    /// Each river's water.
    pub rivers: Vec<crate::nature::Water>,
}

impl Field {
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let fx = (x / self.step).clamp(0.0, (self.n - 1) as f32);
        let fz = (z / self.step).clamp(0.0, (self.n - 1) as f32);
        let (x0, z0) = (fx.floor() as usize, fz.floor() as usize);
        let (x1, z1) = ((x0 + 1).min(self.n - 1), (z0 + 1).min(self.n - 1));
        let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
        let h = |x: usize, z: usize| self.heights[z * self.n + x];
        let a = h(x0, z0) * (1.0 - tx) + h(x1, z0) * tx;
        let b = h(x0, z1) * (1.0 - tx) + h(x1, z1) * tx;
        a * (1.0 - tz) + b * tz
    }

    pub fn surface(&self, x: f32, z: f32) -> Surface {
        let i = ((z / self.step).round().clamp(0.0, (self.n - 1) as f32) as usize) * self.n
            + (x / self.step).round().clamp(0.0, (self.n - 1) as f32) as usize;
        Surface::from_zone(self.surfaces[i])
    }
}

/// Samples a side when baking every `every`th of the mode's own.
pub fn samples(mode: Mode, every: usize) -> usize {
    (mode.samples() - 1) / every.max(1) + 1
}

/// The bare land, before the track: slow to make, so the builder keeps it
/// while the land settings stay the same.
pub fn land(course: &Course, every: usize) -> Vec<f32> {
    let every = every.max(1);
    let n = samples(course.mode, every);
    let step = course.mode.scale() * every as f32;
    let side = course.mode.side();
    let land = if course.mode == Mode::Supercross {
        crate::Land {
            kind: crate::LandKind::Flat,
            ..course.land.clone()
        }
    } else {
        course.land.clone()
    };
    let mut out = vec![0.0; n * n];
    // A copy keeps the track's own ground.
    if let Some(src) = &course.source {
        rows(&mut out, n, |z, row| {
            for (x, h) in row.iter_mut().enumerate() {
                *h = src.height(x as f32 * step, z as f32 * step);
            }
        });
        return out;
    }
    rows(&mut out, n, |z, row| {
        for (x, h) in row.iter_mut().enumerate() {
            *h = land::height(&land, side, x as f32 * step, z as f32 * step);
        }
    });
    out
}

/// How many threads to work on: one in a browser, which has none.
pub(crate) fn threads() -> usize {
    if cfg!(target_arch = "wasm32") {
        return 1;
    }
    std::thread::available_parallelism().map_or(1, |t| t.get()).clamp(1, 16)
}

/// Runs `f` on each row of `out`, on a few threads.
fn rows<T: Send>(out: &mut [T], n: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let threads = threads();
    if threads == 1 {
        for (k, row) in out.chunks_mut(n).enumerate() {
            f(k, row);
        }
        return;
    }
    let per = n.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (chunk, part) in out.chunks_mut(per * n).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (k, row) in part.chunks_mut(n).enumerate() {
                    f(chunk * per + k, row);
                }
            });
        }
    });
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far, ft, the track bed's grade is averaged along the line.
fn grade_window(mode: Mode) -> f32 {
    match mode {
        Mode::Supercross => 0.0,
        Mode::National => 140.0,
        Mode::Enduro => 70.0,
        Mode::Baja => 180.0,
    }
}

/// The tallest a fully banked berm stands on its outside, ft.
fn bank_most(mode: Mode) -> f32 {
    match mode {
        Mode::Supercross => 5.0,
        Mode::National => 9.0,
        Mode::Enduro => 5.0,
        Mode::Baja => 6.0,
    }
}

/// The track bed along the line: its grade and the banking at each sample.
pub struct Bed {
    pub grade: Vec<f32>,
    pub bank: Vec<f32>,
}

impl Bed {
    pub fn new(course: &Course, path: &Path) -> Bed {
        let side = course.mode.side();
        let land = if course.mode == Mode::Supercross {
            crate::Land {
                kind: crate::LandKind::Flat,
                ..course.land.clone()
            }
        } else {
            course.land.clone()
        };
        let raw: Vec<f32> = path
            .pts
            .iter()
            .map(|p| match &course.source {
                Some(src) => src.height(p[0], p[1]),
                None => land::height(&land, side, p[0], p[1]),
            })
            .collect();
        let mut grade = path.smooth(&raw, grade_window(course.mode));
        // Twice, for a gentler ride over the hills.
        grade = path.smooth(&grade, grade_window(course.mode));
        let most = bank_most(course.mode) * course.line.berms.clamp(0.0, 1.0);
        // Full banking on a corner of about a 60 ft radius.
        let bank = path
            .turn
            .iter()
            .map(|t| (t.abs() * 60.0).clamp(0.0, 1.0) * most * t.signum())
            .collect();
        Bed { grade, bank }
    }
}

/// Bakes the course at every `every`th sample, over `land` from [`land`].
pub fn bake(course: &Course, every: usize, land: &[f32]) -> Field {
    let every = every.max(1);
    let mode = course.mode;
    let n = samples(mode, every);
    let step = mode.scale() * every as f32;
    assert_eq!(land.len(), n * n, "land baked at another size");
    let path = course.path();
    let shoulder = mode.shoulder();
    let widest = path
        .width
        .iter()
        .copied()
        .fold(mode.track_width(), f32::max);
    let reach = widest / 2.0 + shoulder + 2.0 * step + 4.0;
    let grid = Grid::new(&path, reach);
    let bed = Bed::new(course, &path);
    let ground = if mode == Mode::Supercross {
        Surface::Dirt
    } else {
        course.land.kind.ground()
    };
    let jumps = &course.jumps;
    let copy = course.source.is_some();
    let shoulder = if copy { shoulder.min(12.0) } else { shoulder };

    let mut heights = land.to_vec();
    let mut track = vec![f32::MAX; n * n];
    // The track, by rows on a few threads.
    {
        let mut both: Vec<(f32, f32)> = heights.iter().map(|&h| (h, f32::MAX)).collect();
        rows(&mut both, n, |z, row| {
            for (x, cell) in row.iter_mut().enumerate() {
                let p = [x as f32 * step, z as f32 * step];
                let Some((near, d)) = grid.nearest(p) else {
                    continue;
                };
                if d > reach {
                    continue;
                }
                let i = near.i;
                let hw = path.width[i] / 2.0;
                let mut top = bed.grade[i];
                for j in jumps {
                    let ds = path.ahead(j.at, near.s);
                    if ds >= 0.0 && ds <= j.length {
                        top += j.height_at(ds);
                    }
                }
                // The outside of a corner (the side away from the turn)
                // rises into a berm.
                let b = bed.bank[i];
                if b != 0.0 {
                    let outer = (-b.signum() * near.side / hw).max(0.0);
                    top += b.abs()
                        * smoothstep(0.05, 1.0, outer.min(1.0))
                        * (1.0 + 0.3 * (outer - 1.0).clamp(0.0, 0.6));
                }
                let t = smoothstep(hw, hw + shoulder, d);
                if copy {
                    // A copy's track is already in its ground: the jumps
                    // and berms go on top of it.
                    cell.0 += (top - bed.grade[i]) * (1.0 - t);
                } else {
                    cell.0 = top * (1.0 - t) + cell.0 * t;
                }
                cell.1 = d / hw;
            }
        });
        for (k, (h, d)) in both.into_iter().enumerate() {
            heights[k] = h;
            track[k] = d;
        }
    }
    let mut surfaces: Vec<u8> = track
        .iter()
        .enumerate()
        .map(|(k, &d)| {
            if let Some(src) = &course.source {
                return src.surface((k % n) as f32 * step, (k / n) as f32 * step);
            }
            if d <= 1.06 {
                Surface::Dirt.zone()
            } else {
                ground.zone()
            }
        })
        .collect();
    for s in &course.strokes {
        apply_stroke(s, n, step, &mut heights);
    }
    let mut water = vec![f32::NAN; n * n];
    let rivers = crate::nature::carve(course, n, step, &mut heights, &mut surfaces, &mut water);
    for p in &course.paint {
        let r = p.radius.max(step);
        for d in &p.dabs {
            each_within(n, step, *d, r, |k, _| surfaces[k] = p.surface.zone());
        }
    }
    for h in &mut heights {
        *h = h.max(0.5);
    }
    Field {
        n,
        step,
        heights,
        surfaces,
        track,
        water,
        rivers,
    }
}

/// Calls `f(sample, falloff)` for each sample within `r` ft of `c`, with
/// falloff 1 at the middle to 0 at the rim.
pub(crate) fn each_within(n: usize, step: f32, c: [f32; 2], r: f32, mut f: impl FnMut(usize, f32)) {
    let lo = |v: f32| ((v - r) / step).floor().max(0.0) as usize;
    let hi = |v: f32| (((v + r) / step).ceil().max(0.0) as usize).min(n - 1);
    for z in lo(c[1])..=hi(c[1]) {
        for x in lo(c[0])..=hi(c[0]) {
            let (dx, dz) = (x as f32 * step - c[0], z as f32 * step - c[1]);
            let d = (dx * dx + dz * dz).sqrt();
            if d < r {
                let t = 1.0 - d / r;
                f(z * n + x, t * t * (3.0 - 2.0 * t));
            }
        }
    }
}

/// How many feet one dab of Raise or Lower moves the ground at full
/// strength.
const DAB_FT: f32 = 2.0;

pub fn apply_stroke(s: &Stroke, n: usize, step: f32, heights: &mut [f32]) {
    let r = s.radius.max(step * 1.5);
    let k = s.strength.clamp(0.0, 1.0);
    for (di, d) in s.dabs.iter().enumerate() {
        match s.brush {
            Brush::Raise => each_within(n, step, *d, r, |i, f| heights[i] += DAB_FT * k * f),
            Brush::Lower => each_within(n, step, *d, r, |i, f| heights[i] -= DAB_FT * k * f),
            Brush::Flatten => each_within(n, step, *d, r, |i, f| {
                heights[i] += (s.target - heights[i]) * (k * f * 0.5)
            }),
            Brush::Smooth => {
                let (mut sum, mut count) = (0.0, 0.0);
                each_within(n, step, *d, r, |i, f| {
                    sum += heights[i] * f;
                    count += f;
                });
                if count > 0.0 {
                    let mean = sum / count;
                    each_within(n, step, *d, r, |i, f| {
                        heights[i] += (mean - heights[i]) * (k * f * 0.4)
                    });
                }
            }
            Brush::Roughen => each_within(n, step, *d, r, |i, f| {
                let (x, z) = ((i % n) as f32 * step, (i / n) as f32 * step);
                heights[i] += land::noise(x / 7.0, z / 7.0, 77 + di as u32 % 3) * 0.5 * k * f;
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kit, Jump};

    #[test]
    fn the_preview_matches_the_full_bake() {
        let c = Course::new(Mode::Supercross);
        let full = bake(&c, 1, &land(&c, 1));
        let quick = bake(&c, 4, &land(&c, 4));
        assert_eq!(full.n, 769);
        assert_eq!(quick.n, 193);
        for z in (0..quick.n).step_by(7) {
            for x in (0..quick.n).step_by(5) {
                let (a, b) = (
                    quick.heights[z * quick.n + x],
                    full.heights[z * 4 * full.n + x * 4],
                );
                assert!((a - b).abs() < 0.01, "{a} {b} at {x},{z}");
            }
        }
    }

    #[test]
    fn a_jump_stands_on_the_line() {
        let mut c = Course::new(Mode::Supercross);
        c.jumps = vec![Jump {
            kind: kit::Kind::Tabletop,
            at: 100.0,
            length: 60.0,
            height: 7.0,
        }];
        c.line.berms = 0.0;
        let f = bake(&c, 1, &land(&c, 1));
        let p = c.path();
        let top = p.at(130.0);
        let flat = p.at(300.0);
        let (ht, hf) = (f.height(top[0], top[1]), f.height(flat[0], flat[1]));
        assert!((ht - hf - 7.0).abs() < 0.3, "top {ht} flat {hf}");
        assert_eq!(f.surface(top[0], top[1]), Surface::Dirt);
    }

    #[test]
    fn corners_bank_on_their_outside() {
        let mut c = Course::new(Mode::National);
        c.jumps.clear();
        c.line.berms = 1.0;
        let f = bake(&c, 2, &land(&c, 2));
        let p = c.path();
        // The sharpest corner.
        let i = (0..p.turn.len())
            .max_by(|&a, &b| p.turn[a].abs().partial_cmp(&p.turn[b].abs()).unwrap())
            .unwrap();
        let s = p.side(i);
        let hw = p.width[i] / 2.0 * 0.9;
        let out = -p.turn[i].signum();
        let at = |k: f32| f.height(p.pts[i][0] + s[0] * hw * k, p.pts[i][1] + s[1] * hw * k);
        assert!(
            at(out) > at(-out) + 1.0,
            "outside {} inside {}",
            at(out),
            at(-out)
        );
    }

    #[test]
    fn brushes_move_the_ground() {
        let mut c = Course::new(Mode::Supercross);
        let l = land(&c, 4);
        let before = bake(&c, 4, &l);
        c.strokes.push(Stroke {
            brush: Brush::Raise,
            radius: 40.0,
            strength: 1.0,
            target: 0.0,
            dabs: vec![[150.0, 150.0]; 5],
        });
        let after = bake(&c, 4, &l);
        let d = after.height(150.0, 150.0) - before.height(150.0, 150.0);
        assert!(d > 8.0 && d < 10.1, "{d}");
        c.paint.push(crate::Paint {
            surface: Surface::Mud,
            radius: 20.0,
            dabs: vec![[150.0, 150.0]],
        });
        assert_eq!(bake(&c, 4, &l).surface(150.0, 150.0), Surface::Mud);
    }
}
