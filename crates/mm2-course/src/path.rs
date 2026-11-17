//! The racing line as a smooth curve: a centripetal Catmull-Rom spline
//! through the player's points, sampled every [`STEP`] ft, with each
//! sample's direction, width and how sharply the line turns there.

use crate::Point;

/// Feet between samples.
pub const STEP: f32 = 2.0;

#[derive(Debug, Clone, Default)]
pub struct Path {
    /// Positions, ft.
    pub pts: Vec<[f32; 2]>,
    /// Distance along the line, ft.
    pub s: Vec<f32>,
    /// Unit direction of travel.
    pub dir: Vec<[f32; 2]>,
    /// Track width, ft.
    pub width: Vec<f32>,
    /// How the line turns, radians a foot, + towards the left (+side),
    /// smoothed over a few lengths of track.
    pub turn: Vec<f32>,
    pub closed: bool,
    pub length: f32,
    /// The sample nearest each of the player's points.
    pub at_point: Vec<usize>,
}

/// Where a spot on the ground lies against the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Near {
    /// The nearest sample.
    pub i: usize,
    /// Distance along the line, ft.
    pub s: f32,
    /// Sideways from the line, ft: + to the left of the way of travel.
    pub side: f32,
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// One centripetal Catmull-Rom segment from p1 to p2, at t (0 to 1).
fn catmull(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let k = |a: [f32; 2], b: [f32; 2]| dist(a, b).max(1e-3).sqrt();
    let t0 = 0.0;
    let t1 = t0 + k(p0, p1);
    let t2 = t1 + k(p1, p2);
    let t3 = t2 + k(p2, p3);
    let u = t1 + (t2 - t1) * t;
    let a1 = lerp2(p0, p1, (u - t0) / (t1 - t0));
    let a2 = lerp2(p1, p2, (u - t1) / (t2 - t1));
    let a3 = lerp2(p2, p3, (u - t2) / (t3 - t2));
    let b1 = lerp2(a1, a2, (u - t0) / (t2 - t0));
    let b2 = lerp2(a2, a3, (u - t1) / (t3 - t1));
    lerp2(b1, b2, (u - t1) / (t2 - t1))
}

impl Path {
    pub fn new(points: &[Point], closed: bool) -> Path {
        let n = points.len();
        let closed = closed && n >= 3;
        if n < 2 {
            return Path {
                closed,
                ..Path::default()
            };
        }
        let p = |i: isize| -> Point {
            if closed {
                points[i.rem_euclid(n as isize) as usize]
            } else if i < 0 {
                // Mirror the end so the curve leaves straight.
                let (a, b) = (points[0], points[1]);
                Point {
                    x: 2.0 * a.x - b.x,
                    z: 2.0 * a.z - b.z,
                    width: a.width,
                }
            } else if i as usize >= n {
                let (a, b) = (points[n - 1], points[n - 2]);
                Point {
                    x: 2.0 * a.x - b.x,
                    z: 2.0 * a.z - b.z,
                    width: a.width,
                }
            } else {
                points[i as usize]
            }
        };
        let segments = if closed { n } else { n - 1 };
        // Dense first, then even steps.
        let mut dense: Vec<([f32; 2], f32, usize)> = Vec::new();
        for seg in 0..segments {
            let (p0, p1, p2, p3) = (
                p(seg as isize - 1),
                p(seg as isize),
                p(seg as isize + 1),
                p(seg as isize + 2),
            );
            let xy = |q: Point| [q.x, q.z];
            let len = dist(xy(p1), xy(p2));
            let steps = ((len / 0.5).ceil() as usize).clamp(4, 20_000);
            for k in 0..steps {
                let t = k as f32 / steps as f32;
                let at = catmull(xy(p0), xy(p1), xy(p2), xy(p3), t);
                dense.push((at, p1.width + (p2.width - p1.width) * t, seg));
            }
        }
        if !closed {
            let last = points[n - 1];
            dense.push(([last.x, last.z], last.width, n - 1));
        } else {
            dense.push(([points[0].x, points[0].z], points[0].width, n));
        }
        let mut path = Path {
            closed,
            ..Path::default()
        };
        let mut along = 0.0;
        let mut next = 0.0;
        let mut seen_seg = usize::MAX;
        for w in 0..dense.len() {
            let (q, width, seg) = dense[w];
            if w > 0 {
                along += dist(dense[w - 1].0, q);
            }
            if seg != seen_seg && seg < n {
                seen_seg = seg;
                path.at_point.push(path.pts.len());
            }
            if along >= next || w + 1 == dense.len() && !closed {
                path.pts.push(q);
                path.s.push(along);
                path.width.push(width);
                next = along + STEP;
            }
        }
        path.length = along;
        if closed && path.pts.len() > 1 {
            // The last sample sits on the first; keep one.
            if dist(*path.pts.last().unwrap(), path.pts[0]) < STEP * 0.5 {
                path.pts.pop();
                path.s.pop();
                path.width.pop();
            }
        }
        let m = path.pts.len();
        path.at_point.truncate(n);
        path.dir = (0..m)
            .map(|i| {
                let (a, b) = path.neighbours(i);
                let (dx, dz) = (
                    path.pts[b][0] - path.pts[a][0],
                    path.pts[b][1] - path.pts[a][1],
                );
                let l = (dx * dx + dz * dz).sqrt().max(1e-6);
                [dx / l, dz / l]
            })
            .collect();
        // Heading change per foot, smoothed over ~30 ft.
        let raw: Vec<f32> = (0..m)
            .map(|i| {
                let (a, b) = path.neighbours(i);
                let (da, db) = (path.dir[a], path.dir[b]);
                let cross = da[0] * db[1] - da[1] * db[0];
                let dot = da[0] * db[0] + da[1] * db[1];
                // + = turning towards +side (left), with side = (-dz, dx).
                let ang = cross.atan2(dot);
                let mut ds = path.s[b] - path.s[a];
                if ds <= 0.0 {
                    ds += path.length;
                }
                ang / ds.max(STEP)
            })
            .collect();
        path.turn = path.smooth(&raw, 30.0);
        path
    }

    pub fn is_empty(&self) -> bool {
        self.pts.len() < 2
    }

    fn neighbours(&self, i: usize) -> (usize, usize) {
        let m = self.pts.len();
        if self.closed {
            ((i + m - 1) % m, (i + 1) % m)
        } else {
            (i.saturating_sub(1), (i + 1).min(m - 1))
        }
    }

    /// The unit vector to the left of the way of travel at sample `i`.
    pub fn side(&self, i: usize) -> [f32; 2] {
        let d = self.dir[i];
        [-d[1], d[0]]
    }

    /// A running average of `values` over `window` ft of line.
    pub fn smooth(&self, values: &[f32], window: f32) -> Vec<f32> {
        let m = values.len();
        let half = ((window / STEP / 2.0) as isize).max(0);
        if m == 0 || half == 0 {
            return values.to_vec();
        }
        // Prefix sums over the samples, wrapping round a loop.
        let get = |i: isize| -> f32 {
            if self.closed {
                values[i.rem_euclid(m as isize) as usize]
            } else {
                values[i.clamp(0, m as isize - 1) as usize]
            }
        };
        let mut out = Vec::with_capacity(m);
        let mut sum: f32 = (-half..=half).map(get).sum();
        let count = (2 * half + 1) as f32;
        for i in 0..m as isize {
            out.push(sum / count);
            sum += get(i + half + 1) - get(i - half);
        }
        out
    }

    /// The sample at `s` ft along (wrapping round a loop).
    pub fn index_at(&self, s: f32) -> usize {
        if self.pts.is_empty() {
            return 0;
        }
        let s = if self.closed {
            s.rem_euclid(self.length.max(1e-3))
        } else {
            s.clamp(0.0, self.length)
        };
        match self
            .s
            .binary_search_by(|v| v.partial_cmp(&s).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i,
            Err(i) => i.min(self.pts.len() - 1),
        }
    }

    /// The position at `s` ft along.
    pub fn at(&self, s: f32) -> [f32; 2] {
        self.pts.get(self.index_at(s)).copied().unwrap_or_default()
    }

    /// The nearest place on the line to `p`, by brute force (for the
    /// builder's clicks; the bake uses a [`Grid`]).
    pub fn nearest(&self, p: [f32; 2]) -> Option<Near> {
        (0..self.pts.len())
            .min_by(|&a, &b| {
                let (da, db) = (dist(self.pts[a], p), dist(self.pts[b], p));
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|i| self.near_at(i, p))
    }

    fn near_at(&self, i: usize, p: [f32; 2]) -> Near {
        let d = self.dir[i];
        let q = self.pts[i];
        let (rx, rz) = (p[0] - q[0], p[1] - q[1]);
        let fwd = rx * d[0] + rz * d[1];
        let side = -rx * d[1] + rz * d[0];
        let mut s = self.s[i] + fwd.clamp(-STEP, STEP);
        if self.closed {
            s = s.rem_euclid(self.length.max(1e-3));
        }
        Near { i, s, side }
    }

    /// The signed distance along the line from `a` to `b` going forward,
    /// round a loop.
    pub fn ahead(&self, a: f32, b: f32) -> f32 {
        if self.closed {
            (b - a).rem_euclid(self.length.max(1e-3))
        } else {
            b - a
        }
    }
}

/// The line's samples sorted into squares, for finding the nearest one to
/// each of millions of terrain samples quickly.
pub struct Grid<'a> {
    path: &'a Path,
    cell: f32,
    cols: usize,
    cells: Vec<Vec<u32>>,
    origin: [f32; 2],
}

impl<'a> Grid<'a> {
    /// Squares of `cell` ft; lookups reach `reach` ft.
    pub fn new(path: &'a Path, reach: f32) -> Grid<'a> {
        let cell = reach.max(8.0);
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &path.pts {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        if path.pts.is_empty() {
            return Grid {
                path,
                cell,
                cols: 0,
                cells: Vec::new(),
                origin: [0.0; 2],
            };
        }
        let origin = [lo[0] - cell, lo[1] - cell];
        let cols = ((hi[0] - origin[0]) / cell) as usize + 2;
        let rows = ((hi[1] - origin[1]) / cell) as usize + 2;
        let mut cells = vec![Vec::new(); cols * rows];
        for (i, p) in path.pts.iter().enumerate() {
            let (cx, cz) = (
                ((p[0] - origin[0]) / cell) as usize,
                ((p[1] - origin[1]) / cell) as usize,
            );
            cells[cz * cols + cx].push(i as u32);
        }
        Grid {
            path,
            cell,
            cols,
            cells,
            origin,
        }
    }

    /// The nearest place on the line to `p`, if any sample lies within the
    /// reach.
    pub fn nearest(&self, p: [f32; 2]) -> Option<(Near, f32)> {
        if self.cols == 0 {
            return None;
        }
        let rows = self.cells.len() / self.cols;
        let fx = (p[0] - self.origin[0]) / self.cell;
        let fz = (p[1] - self.origin[1]) / self.cell;
        if fx < 0.0 || fz < 0.0 || fx >= self.cols as f32 || fz >= rows as f32 {
            return None;
        }
        let (cx, cz) = (fx as usize, fz as usize);
        let mut best = (u32::MAX, f32::MAX);
        for z in cz.saturating_sub(1)..(cz + 2).min(rows) {
            for x in cx.saturating_sub(1)..(cx + 2).min(self.cols) {
                for &i in &self.cells[z * self.cols + x] {
                    let q = self.path.pts[i as usize];
                    let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2);
                    if d < best.1 {
                        best = (i, d);
                    }
                }
            }
        }
        (best.0 != u32::MAX).then(|| {
            let near = self.path.near_at(best.0 as usize, p);
            (near, best.1.sqrt())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(r: f32, n: usize) -> Vec<Point> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                Point {
                    x: r * a.cos(),
                    z: r * a.sin(),
                    width: 30.0,
                }
            })
            .collect()
    }

    #[test]
    fn a_loop_is_about_its_circumference() {
        let p = Path::new(&circle(500.0, 12), true);
        let c = std::f32::consts::TAU * 500.0;
        assert!((p.length - c).abs() / c < 0.01, "{} vs {c}", p.length);
        // Even steps.
        for w in p.s.windows(2) {
            assert!((w[1] - w[0] - STEP).abs() < 0.6);
        }
        // Counter-clockwise (x to z): the turn is towards +side.
        let mid = p.turn[p.turn.len() / 2];
        assert!((mid - 1.0 / 500.0).abs() < 0.0004, "turn {mid}");
        assert_eq!(p.at_point.len(), 12);
    }

    #[test]
    fn the_grid_finds_what_brute_force_finds() {
        let p = Path::new(&circle(300.0, 8), true);
        let g = Grid::new(&p, 60.0);
        for (x, z) in [(310.0, 5.0), (0.0, -290.0), (-250.0, 170.0)] {
            let (a, _) = g.nearest([x, z]).unwrap();
            let b = p.nearest([x, z]).unwrap();
            assert_eq!(a.i, b.i);
        }
        assert!(g.nearest([0.0, 0.0]).is_none());
        // Outside a counter-clockwise loop is to the right (- side).
        let (n, _) = g.nearest([320.0, 0.0]).unwrap();
        assert!(n.side < -15.0, "{}", n.side);
    }

    #[test]
    fn an_open_line_ends_at_its_last_point() {
        let pts = vec![
            Point {
                x: 0.0,
                z: 0.0,
                width: 30.0,
            },
            Point {
                x: 100.0,
                z: 0.0,
                width: 30.0,
            },
            Point {
                x: 200.0,
                z: 50.0,
                width: 40.0,
            },
        ];
        let p = Path::new(&pts, false);
        let last = *p.pts.last().unwrap();
        assert!(dist(last, [200.0, 50.0]) < 0.01);
        assert!((p.width.last().unwrap() - 40.0).abs() < 0.5);
    }
}
