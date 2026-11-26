//! What a camera can see, to leave the rest undrawn: the ground and the
//! still objects are cut into blocks, and a block wholly outside the view
//! is skipped. Only what could not reach the screen is left out, so the
//! picture is the same.

use glam::{Mat4, Vec3, Vec4};

/// The six sides of what a view and projection take in (drawing
/// coordinates), each a plane with the inside where it is positive.
#[derive(Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// From a view and projection (wgpu's depth, 0 to 1).
    pub fn new(view_proj: Mat4) -> Frustum {
        let m = view_proj;
        let [r0, r1, r2, r3] = [m.row(0), m.row(1), m.row(2), m.row(3)];
        Frustum {
            planes: [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2],
        }
    }

    /// Whether any of the box from `lo` to `hi` might be seen.
    pub fn sees(&self, lo: Vec3, hi: Vec3) -> bool {
        self.planes.iter().all(|p| {
            // The box's corner furthest inside this side.
            let c = Vec3::select(p.truncate().cmpge(Vec3::ZERO), hi, lo);
            p.truncate().dot(c) + p.w >= 0.0
        })
    }
}

/// Runs of indices to draw, `start..end`, joined where they meet.
pub fn runs(ranges: impl Iterator<Item = std::ops::Range<u32>>, out: &mut Vec<std::ops::Range<u32>>) {
    out.clear();
    for r in ranges {
        match out.last_mut() {
            Some(last) if last.end == r.start => last.end = r.end,
            _ => out.push(r),
        }
    }
}

/// The ground's heights, to tell what a hill hides: a block wholly
/// behind a hill need not be drawn at all, where the frustum alone
/// would keep it (a hill in view hides what is behind it, not only
/// what is off screen). The ground is the only occluder used: what
/// another object hides is left drawn, so nothing is ever hidden that
/// the ground alone would not hide.
pub struct Heightfield {
    heights: Vec<f32>,
    /// Height samples a side.
    size: usize,
    /// Feet a cell.
    scale: f32,
}

impl Heightfield {
    /// From a terrain's heights (terrain units, row-major with rows
    /// along +z in the game's coordinates) and its scale, ft per terrain
    /// unit across and up: drawing coordinates are x along a row and z
    /// down them, so a row is `-z / scale`, and a height is `h * scale`
    /// ft, as the ground's blocks are drawn (renderer.rs).
    pub fn new(heights: &[f32], size: usize, scale: f32) -> Heightfield {
        Heightfield {
            heights: heights.iter().map(|h| h * scale).collect(),
            size,
            scale,
        }
    }

    /// The ground's height at `x, z` (drawing coordinates), on the same
    /// two triangles a cell is drawn and ridden as (ride::ground), or
    /// none outside the ground.
    pub fn height(&self, x: f32, z: f32) -> Option<f32> {
        if self.size < 2 || self.scale <= 0.0 {
            return None;
        }
        let (col, row) = (x / self.scale, -z / self.scale);
        let last = self.size as f32 - 1.0;
        if !(0.0..=last).contains(&col) || !(0.0..=last).contains(&row) {
            return None;
        }
        let edge = (self.size - 2) as f32;
        let (cx, cz) = (col.floor().min(edge), row.floor().min(edge));
        let (fx, fz) = (col - cx, row - cz);
        let (cx, cz) = (cx as usize, cz as usize);
        let at = |dx: usize, dz: usize| self.heights[(cz + dz) * self.size + cx + dx];
        let (h00, h10, h01, h11) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
        Some(if (cx ^ cz) & 1 == 0 {
            if fx <= fz {
                (h11 - h01) * fx + (h00 - h01) * (1.0 - fz) + h01
            } else {
                (h10 - h00) * fx + (h11 - h10) * fz + h00
            }
        } else if fx + fz >= 1.0 {
            (h11 - h01) * fx + (h10 - h11) * (1.0 - fz) + h01
        } else {
            (h10 - h00) * fx + (h01 - h00) * fz + h00
        })
    }

    /// Whether the ground hides the whole box from `lo` to `hi` as seen
    /// from `eye`: every corner, face middle and centre of the box is
    /// reached only by a ray that dips a foot or more under the ground
    /// first, and the march stops short of the box itself, so a ray
    /// grazing the ground by the box never counts. Where the eye is at
    /// or under the ground, or the box is close or past the far plane,
    /// nothing is hidden: the picture is never left with a hole in it.
    pub fn hides(&self, eye: Vec3, lo: Vec3, hi: Vec3) -> bool {
        if self.height(eye.x, eye.z).is_some_and(|h| eye.y <= h + 1.0) {
            return false;
        }
        // The box's corners, the middle of each face, and its centre:
        // no heap work, this runs for every block every frame.
        let mid = (lo + hi) * 0.5;
        let points = [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, hi.z),
            Vec3::new(mid.x, lo.y, mid.z),
            Vec3::new(mid.x, hi.y, mid.z),
            Vec3::new(lo.x, mid.y, mid.z),
            Vec3::new(hi.x, mid.y, mid.z),
            Vec3::new(mid.x, mid.y, lo.z),
            Vec3::new(mid.x, mid.y, hi.z),
            mid,
        ];
        for p in points {
            let d = eye.distance(p);
            if !(24.0..4000.0).contains(&d) {
                return false;
            }
            let way = (p - eye) / d;
            let step = (d / 64.0).clamp(4.0, 8.0);
            let mut blocked = false;
            let mut t = step;
            while t < d - 8.0 {
                let at = eye + way * t;
                if let Some(h) = self.height(at.x, at.z) {
                    if h > at.y + 1.0 {
                        blocked = true;
                        break;
                    }
                }
                t += step;
            }
            if !blocked {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_ahead_not_behind() {
        let proj = Mat4::perspective_rh(1.0, 1.5, 1.0, 1000.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y);
        let f = Frustum::new(proj * view);
        assert!(f.sees(Vec3::new(-1.0, -1.0, -20.0), Vec3::new(1.0, 1.0, -10.0)));
        assert!(!f.sees(Vec3::new(-1.0, -1.0, 10.0), Vec3::new(1.0, 1.0, 20.0)));
        assert!(!f.sees(Vec3::new(-1.0, -1.0, -2000.0), Vec3::new(1.0, 1.0, -1500.0)));
        assert!(!f.sees(Vec3::new(500.0, -1.0, -20.0), Vec3::new(600.0, 1.0, -10.0)));
    }

    #[test]
    fn joins_runs() {
        let mut out = Vec::new();
        runs([0..3, 3..6, 9..12].into_iter(), &mut out);
        assert_eq!(out, vec![0..6, 9..12]);
    }

    /// A heightfield `size` samples a side, `scale` ft a cell, with
    /// `height` at each (column, row).
    /// (`height` in ft; the terrain keeps terrain units, ft / scale.)
    fn field(size: usize, scale: f32, height: impl Fn(usize, usize) -> f32) -> Heightfield {
        let heights: Vec<f32> = (0..size * size).map(|i| height(i % size, i / size) / scale).collect();
        Heightfield::new(&heights, size, scale)
    }

    #[test]
    fn heights_are_in_feet_like_the_drawn_ground() {
        // A shipped track's scale of 3: a sample of 10 terrain units
        // stands 30 ft, as the ground's blocks are drawn.
        let f = Heightfield::new(&[10.0; 16], 4, 3.0);
        assert_eq!(f.height(4.0, -4.0), Some(30.0));
    }

    #[test]
    fn heights_follow_the_drawn_triangles() {
        // One raised corner (column 1, row 0) of an even cell: on the
        // cell's diagonal from (0, 0) to (1, 1) the ground is flat, as
        // drawn, where blending all four corners would lift it.
        let f = Heightfield::new(&[0.0, 8.0, 0.0, 0.0], 2, 1.0);
        assert_eq!(f.height(0.5, -0.5), Some(0.0));
        assert_eq!(f.height(0.75, -0.25), Some(4.0));
    }

    #[test]
    fn flat_ground_hides_nothing() {
        let f = field(9, 10.0, |_, _| 0.0);
        assert_eq!(f.height(25.0, -35.0), Some(0.0));
        assert_eq!(f.height(1000.0, 0.0), None);
        let eye = Vec3::new(0.0, 10.0, -5.0);
        assert!(!f.hides(eye, Vec3::new(60.0, 0.0, -10.0), Vec3::new(70.0, 5.0, 0.0)));
    }

    #[test]
    fn a_ridge_hides_a_low_box_behind_it() {
        // A ridge 50 ft high at x = 40 ft, flat ground elsewhere.
        let f = field(9, 10.0, |c, _| if c == 4 { 50.0 } else { 0.0 });
        let eye = Vec3::new(0.0, 10.0, -5.0);
        assert!(f.hides(eye, Vec3::new(60.0, 0.0, -10.0), Vec3::new(70.0, 5.0, 0.0)));
    }

    #[test]
    fn a_tall_box_peeking_over_the_ridge_is_kept() {
        // The same ridge, only 30 ft high: a 60 ft box behind it shows
        // over the top.
        let f = field(9, 10.0, |c, _| if c == 4 { 30.0 } else { 0.0 });
        let eye = Vec3::new(0.0, 10.0, -5.0);
        assert!(!f.hides(eye, Vec3::new(60.0, 0.0, -10.0), Vec3::new(70.0, 60.0, 0.0)));
    }

    #[test]
    fn a_valley_hides_nothing_and_low_eyes_hide_nothing() {
        // Low ground between eye and box never hides it.
        let f = field(9, 10.0, |c, _| if c == 4 { -20.0 } else { 0.0 });
        let eye = Vec3::new(0.0, 10.0, -5.0);
        assert!(!f.hides(eye, Vec3::new(60.0, 0.0, -10.0), Vec3::new(70.0, 5.0, 0.0)));
        // Nor does anything when the eye itself is at the ground.
        let flat = field(9, 10.0, |_, _| 10.0);
        assert!(!flat.hides(Vec3::new(0.0, 5.0, -5.0), Vec3::new(60.0, 0.0, -10.0), Vec3::new(70.0, 5.0, 0.0)));
        // Nor a box right by the eye, nor one outside the ground.
        assert!(!f.hides(eye, Vec3::new(1.0, 0.0, -6.0), Vec3::new(2.0, 1.0, -4.0)));
        assert!(!f.hides(Vec3::new(1000.0, 10.0, -5.0), Vec3::new(1060.0, 0.0, -10.0), Vec3::new(1070.0, 5.0, 0.0)));
    }
}
