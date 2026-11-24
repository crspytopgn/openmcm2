//! The race's course tracker, as `mm2-viewer`'s `race.rs` has it (the
//! game's 0x4eb170 and 0x51c9c0..0x51d7b0): the course's points from the
//! `.tdf`, and where on them a rider is. Kept the same as the game's, so
//! the riders here go round as they do in a race.

#![allow(dead_code)]

use glam::Vec3;
use mm2_formats::tdf::{Line, TrackDef};
use mm2_physics::race;

/// The course as one list of points to ride through, in order.
#[derive(Debug, Clone)]
pub struct Course {
    pub points: Vec<Vec3>,
    /// Half the course's width at each point, ft.
    widths: Vec<f32>,
    /// Each point's cross-section ends.
    edges: Vec<[Vec3; 2]>,
    /// The point after each one; the last of a node leads to the node it
    /// links to.
    pub next: Vec<usize>,
    /// Points in one lap.
    pub lap: usize,
    finish: Line,
    /// Where every rider's confirmed place starts: the start line's
    /// place, 0.1 on (0x4eb170).
    start: Option<Place>,
}

impl Course {
    /// Each stretch of the course's two sides, as the radar draws them.
    pub fn side_segments(&self) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        self.next
            .iter()
            .enumerate()
            .flat_map(move |(i, &n)| (0..2).map(move |k| (self.edges[i][k], self.edges[n][k])))
    }

    pub fn new(def: &TrackDef) -> Option<Self> {
        let mut first = Vec::new();
        let mut points = Vec::new();
        let mut widths = Vec::new();
        let mut edges = Vec::new();
        for node in &def.nodes {
            first.push(points.len());
            edges.extend(node.sections.iter().map(|s| s.edges.map(Vec3::from)));
            points.extend(node.sections.iter().map(|s| Vec3::from(s.center)));
            widths.extend(
                node.sections
                    .iter()
                    .map(|s| flat(Vec3::from(s.edges[0]) - Vec3::from(s.edges[1])).length() / 2.0),
            );
        }
        if points.len() < 3 {
            return None;
        }
        let mut next = Vec::with_capacity(points.len());
        for (n, node) in def.nodes.iter().enumerate() {
            let count = node.sections.len();
            for i in 0..count {
                let here = first[n] + i;
                next.push(if i + 1 < count {
                    here + 1
                } else {
                    // On to the linked node, or round this one again.
                    let to = node.links.first().map_or(n, |&l| l as usize);
                    first
                        .get(to)
                        .copied()
                        .filter(|&f| f < points.len())
                        .unwrap_or(first[n])
                });
            }
        }
        // Skip a closing point that repeats the first one.
        for i in 0..points.len() {
            let j = next[i];
            if j != i && (points[i] - points[j]).length() < 0.5 {
                next[i] = next[j];
            }
        }
        // Walk well into the loop, then count its length.
        let mut at = 0;
        for _ in 0..points.len() {
            at = next[at];
        }
        let (mut lap, mut walk) = (1, next[at]);
        while walk != at && lap <= points.len() {
            walk = next[walk];
            lap += 1;
        }
        let finish = def.finish.or(def.start_line())?;
        let mut course = Course {
            points,
            widths,
            edges,
            next,
            lap,
            finish,
            start: None,
        };
        if let Some(line) = def.start_line() {
            let way = Vec3::from(line.direction);
            course.start = course
                .places(Vec3::from(line.position))
                .into_iter()
                .max_by(|a, b| course.way(*a).dot(way).total_cmp(&course.way(*b).dot(way)))
                .map(|p| Place {
                    leg: p.leg,
                    t: (p.t + 0.1).min(1.0),
                });
        }
        Some(course)
    }

    /// Where `at` stands on the leg into point `to`, and the way along it,
    /// when it is within the course's sides there (the game's confirmed
    /// course place, 0x51d700, 0x51d7b0).
    fn place(&self, at: Vec3, to: usize) -> Option<(Vec3, Vec3)> {
        let from = self.previous(to);
        let (a, b) = (flat(self.points[from]), flat(self.points[to]));
        let leg = b - a;
        let length = leg.length();
        if length < 1e-3 {
            return None;
        }
        let dir = leg / length;
        let t = ((flat(at) - a).dot(dir) / length).clamp(0.0, 1.0);
        let on = a + leg * t;
        let half = self.widths[from] + (self.widths[to] - self.widths[from]) * t;
        ((flat(at) - on).length() <= half).then_some((on, dir))
    }

    /// The point before `i` along the course.
    pub fn previous(&self, i: usize) -> usize {
        self.next.iter().position(|&n| n == i).unwrap_or(i)
    }

    /// The place on the course's centre line nearest `at`, on the level
    /// (0x51c320).
    pub fn nearest_place(&self, at: Vec3) -> Option<Place> {
        let p = flat(at);
        let mut best: Option<(Place, f32)> = None;
        for leg in 0..self.points.len() {
            let to = self.next[leg];
            if to == leg {
                continue;
            }
            let (a, b) = (flat(self.points[leg]), flat(self.points[to]));
            let d = b - a;
            let l2 = d.length_squared();
            let t = if l2 == 0.0 {
                0.0
            } else {
                ((p - a).dot(d) / l2).clamp(0.0, 1.0)
            };
            let gap = (a + d * t - p).length_squared();
            if best.is_none_or(|(_, g)| gap < g) {
                best = Some((Place { leg, t }, gap));
            }
        }
        best.map(|(p, _)| p)
    }

    /// The point nearest `at` among the first lap's points.
    pub fn nearest(&self, at: Vec3) -> usize {
        (0..self.points.len())
            .min_by(|&a, &b| {
                flat(self.points[a] - at)
                    .length()
                    .total_cmp(&flat(self.points[b] - at).length())
            })
            .unwrap_or(0)
    }

    /// The finish line as the game tests it (0x47c5b0): no height limit,
    /// as the port has no line height to go by.
    pub fn finish_line(&self) -> race::Line {
        let f = &self.finish;
        race::Line {
            centre: Vec3::from(f.position),
            forward: Vec3::from(f.direction),
            half_width: Vec3::from(f.across),
            height: f32::INFINITY,
        }
    }

    /// How far the finish line is ahead of `at` along the course, and how
    /// far `at` is past it (0x51d420 both ways), ft.
    fn to_finish(&self, at: Vec3) -> (f32, f32) {
        let finish = Vec3::from(self.finish.position);
        let (here, line) = (self.nearest(at), self.nearest(finish));
        let along = |from: usize, to: usize| {
            let (mut i, mut length) = (from, 0.0);
            for _ in 0..self.points.len() {
                if i == to {
                    break;
                }
                let j = self.next[i];
                length += flat(self.points[j] - self.points[i]).length();
                i = j;
            }
            length
        };
        (along(here, line), along(line, here))
    }

    /// The point `distance` along the course from `from`, going through
    /// point `to`.
    fn ahead(&self, from: Vec3, mut to: usize, mut distance: f32) -> Vec3 {
        let mut here = flat(from);
        for _ in 0..self.points.len() {
            let p = flat(self.points[to]);
            let leg = (p - here).length();
            if leg >= distance {
                return here + (p - here) * (distance / leg.max(1e-3));
            }
            distance -= leg;
            here = p;
            to = self.next[to];
        }
        here
    }
}

/// A place on the course graph: a leg (from a point to the next) and how
/// far along it, 0 to 1 (the game's node, segment and t).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    leg: usize,
    t: f32,
}

impl Course {
    fn leg_length(&self, leg: usize) -> f32 {
        (self.points[self.next[leg]] - self.points[leg]).length()
    }

    /// The point at `p` (0x51d700).
    pub fn point(&self, p: Place) -> Vec3 {
        self.points[p.leg].lerp(self.points[self.next[p.leg]], p.t.clamp(0.0, 1.0))
    }

    /// Half the course's width at `p`, ft.
    pub fn half_width(&self, p: Place) -> f32 {
        let (a, b) = (self.widths[p.leg], self.widths[self.next[p.leg]]);
        a + (b - a) * p.t
    }

    /// The level way along the leg at `p` (0x51d7b0).
    pub fn way(&self, p: Place) -> Vec3 {
        flat(self.points[self.next[p.leg]] - self.points[p.leg]).normalize_or(Vec3::Z)
    }

    /// The place `distance` ft on from `p` along the course (0x51d520).
    pub fn walk(&self, p: Place, mut distance: f32) -> Place {
        let (mut leg, mut t) = (p.leg, p.t);
        for _ in 0..=self.points.len() {
            let length = self.leg_length(leg);
            let left = (1.0 - t) * length;
            if distance <= left {
                let t = if length > 0.0 {
                    t + distance / length
                } else {
                    1.0
                };
                return Place { leg, t };
            }
            distance -= left;
            leg = self.next[leg];
            t = 0.0;
        }
        Place { leg, t }
    }

    /// How far on along the course `to` is from `from`, when it is no
    /// more than `most` ft on (0x51d420).
    pub fn along(&self, from: Place, to: Place, most: f32) -> Option<f32> {
        if from.leg == to.leg && from.t <= to.t {
            return Some((to.t - from.t) * self.leg_length(from.leg));
        }
        let mut d = (1.0 - from.t) * self.leg_length(from.leg);
        let mut leg = self.next[from.leg];
        for _ in 0..self.points.len() {
            if d > most {
                return None;
            }
            if leg == to.leg {
                let d = d + to.t * self.leg_length(leg);
                return (d <= most).then_some(d);
            }
            d += self.leg_length(leg);
            leg = self.next[leg];
        }
        None
    }

    /// The places whose stretch of course `at` stands on (0x51c9c0): each
    /// pair of cross-sections in a row bounds a stretch, its sides from
    /// edge to edge and its ends the sections' lines through their centres
    /// (0x51c570), so the outside of a bend has no gaps between stretches.
    /// The place is along the centre line, clamped to the stretch.
    fn places(&self, at: Vec3) -> Vec<Place> {
        let p = flat(at);
        let mut found = Vec::new();
        for leg in 0..self.points.len() {
            let to = self.next[leg];
            if to == leg {
                continue;
            }
            let (a, b) = (self.points[leg], self.points[to]);
            let ([a0, a1], [b0, b1]) = (self.edges[leg], self.edges[to]);
            if !inside(p, &[a0, b0, b, b1, a1, a]) {
                continue;
            }
            let d = flat(b - a);
            let l2 = d.length_squared();
            let t = if l2 == 0.0 {
                0.0
            } else {
                ((p - flat(a)).dot(d) / l2).clamp(0.0, 1.0)
            };
            found.push(Place { leg, t });
        }
        found
    }

    /// The place under `probe` where the course runs most the way of
    /// `heading`.
    fn facing(&self, probe: Vec3, heading: Vec3) -> Option<Place> {
        self.places(probe).into_iter().max_by(|a, b| {
            self.way(*a)
                .dot(heading)
                .total_cmp(&self.way(*b).dot(heading))
        })
    }

    /// The race tracker's step for one rider (0x4eb170, 0x51c9c0 with
    /// 100 ft ahead and 50 ft back, 0x578ac8 and 0x578acc): where the probe is on the course near the
    /// confirmed place, and the confirmed place moved on to it when it is
    /// ahead by under 100 ft. None when the probe is off the course.
    pub fn follow(
        &self,
        probe: Vec3,
        heading: Vec3,
        confirmed: &mut Option<Place>,
    ) -> Option<Place> {
        let found = self.places(probe);
        // The game starts every rider from the start line's place, so a
        // rider on the gate behind it aims down the course from the line.
        // (A race picked up part way round starts from where it is.)
        if confirmed.is_none() {
            // A gate slot can stand past the course's side (the eighth on
            // Nationals 1): a rider there still starts from the line, or
            // it never finds the course and rides off across the map.
            *confirmed = self.start.filter(|&s| {
                found
                    .iter()
                    .any(|&p| self.along(s, p, 100.0).is_some() || self.along(p, s, 50.0).is_some())
                    || (found.is_empty() && flat(probe - self.point(s)).length() < 100.0)
            });
        }
        let Some(c) = *confirmed else {
            // No start line on the course: where the course runs the way
            // the bike faces.
            let first = self.facing(probe, heading);
            *confirmed = first;
            return first;
        };
        let mut best: Option<(Place, f32)> = None;
        for p in found {
            let score = match (self.along(c, p, 100.0), self.along(p, c, 50.0)) {
                (Some(f), _) => f,
                (None, Some(b)) => -b,
                (None, None) => continue,
            };
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((p, score));
            }
        }
        let (p, score) = best?;
        if score > 0.0 && score < 100.0 {
            *confirmed = Some(p);
        }
        Some(p)
    }
}

pub(crate) fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Whether `p` is inside the polygon `corners` on the ground (x, z), by
/// counting the sides a line from it crosses (0x51c570).
fn inside(p: Vec3, corners: &[Vec3]) -> bool {
    let mut odd = false;
    for (i, a) in corners.iter().enumerate() {
        let b = corners[(i + 1) % corners.len()];
        if (a.z <= p.z) != (b.z <= p.z) {
            let x = a.x + (p.z - a.z) * (b.x - a.x) / (b.z - a.z);
            if p.x < x {
                odd = !odd;
            }
        }
    }
    odd
}
