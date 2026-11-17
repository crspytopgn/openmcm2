//! One-click help for the builder: filling a line with jumps where they
//! fit, and a whole new track to start from.

use crate::kit::Kind;
use crate::{Course, Jump, Mode, Point};

/// A small seeded random source.
struct Dice(u64);

impl Dice {
    fn new(seed: u32) -> Dice {
        Dice((seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0x2545_F491_4F6C_DD1D))
    }
    /// 0 to 1.
    fn roll(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        from[((self.roll() * from.len() as f32) as usize).min(from.len() - 1)]
    }
}

/// The jumps a kind of race is built from, the more common listed twice.
fn pool(mode: Mode) -> &'static [Kind] {
    use Kind::*;
    match mode {
        Mode::Supercross => &[Tabletop, Double, Double, Triple, Rhythm, Rhythm, Whoops, StepUp, StepDown, Kicker],
        Mode::National => &[Tabletop, Tabletop, Double, Triple, StepUp, StepDown, Roller, Roller, Whoops, Kicker],
        Mode::Baja => &[Roller, Roller, Whoops, Dip, Dip, Kicker, Tabletop],
        Mode::Enduro => &[Roller, Roller, Dip, Whoops, Kicker, Tabletop, StepUp],
    }
}

/// Fills the straights of the line with jumps, leaving the jumps already
/// there and room round each to ride in and land. Returns how many it
/// added.
pub fn fill_jumps(course: &mut Course, seed: u32) -> usize {
    let path = course.path();
    if path.is_empty() || path.length < 200.0 {
        return 0;
    }
    let mut dice = Dice::new(seed);
    let sx = course.mode == Mode::Supercross;
    // Room before a jump and after it, and the tightest turn (radians a
    // foot) a jump may sit on.
    let (before, after, tight) = if sx { (35.0, 30.0, 0.012) } else { (70.0, 70.0, 0.006) };
    let spread = match course.mode {
        Mode::Supercross => 0.0,
        Mode::National => 130.0,
        Mode::Baja | Mode::Enduro => 400.0,
    };
    // Leave the start straight clear for the gate drop.
    let first = if sx { 120.0 } else { 220.0 };
    let last = path.length - if path.closed { 60.0 } else { 150.0 };
    let taken: Vec<(f32, f32)> = course.jumps.iter().map(|j| (j.at - before, j.at + j.length + after)).collect();
    let free = |a: f32, b: f32, extra: &[(f32, f32)]| {
        taken.iter().chain(extra).all(|&(x, y)| b < x || a > y)
    };
    let straight = |a: f32, b: f32| {
        let mut s = a;
        while s <= b {
            if path.turn[path.index_at(s)].abs() > tight {
                return false;
            }
            s += 5.0;
        }
        true
    };
    let mut added = Vec::new();
    let mut s = first;
    while s < last {
        let kind = dice.pick(pool(course.mode));
        let mut jump: Jump = kind.make(course.mode, s);
        jump.height *= 0.85 + dice.roll() * 0.3;
        jump.length *= 0.9 + dice.roll() * 0.2;
        let (a, b) = (s - before, s + jump.length + after);
        if b < last && free(a, b, &added) && straight(s - before * 0.5, s + jump.length + after * 0.5) {
            added.push((a, b));
            course.jumps.push(jump);
            s = b + spread * (0.5 + dice.roll());
        } else {
            s += 10.0;
        }
    }
    course.jumps.sort_by(|a, b| a.at.total_cmp(&b.at));
    added.len()
}

/// A whole new track of `mode`: a fresh line, land and jumps.
pub fn surprise(mode: Mode, seed: u32) -> Course {
    let mut c = Course::new(mode);
    let mut dice = Dice::new(seed ^ 0x5A5A);
    c.land.seed = seed;
    if mode != Mode::Supercross {
        let mid = mode.side() / 2.0;
        let (rx, rz) = match mode {
            Mode::National => (950.0, 650.0),
            Mode::Baja => (2300.0, 1900.0),
            _ => (1200.0, 900.0),
        };
        let n = 10 + (dice.roll() * 6.0) as usize;
        let (k1, k2) = (2.0 + (dice.roll() * 2.0).floor(), 3.0 + (dice.roll() * 3.0).floor());
        let (p1, p2) = (dice.roll() * 6.3, dice.roll() * 6.3);
        let (a1, a2) = (0.10 + dice.roll() * 0.12, 0.05 + dice.roll() * 0.10);
        let width = mode.track_width();
        c.line.points = (0..n)
            .map(|i| {
                let a = (i as f32 + (dice.roll() - 0.5) * 0.5) / n as f32 * std::f32::consts::TAU;
                let r = 1.0 + a1 * (k1 * a + p1).sin() + a2 * (k2 * a + p2).sin() + (dice.roll() - 0.5) * 0.12;
                Point { x: mid + rx * a.cos() * r, z: mid + rz * a.sin() * r, width }
            })
            .collect();
        c.land.hills = (c.land.hills * (0.7 + dice.roll() * 0.6)).clamp(0.0, 1.0);
    }
    fill_jumps(&mut c, seed);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_gets_jumps_that_do_not_overlap() {
        for mode in Mode::ALL {
            for seed in 1..4 {
                let c = surprise(mode, seed);
                assert!(c.jumps.len() >= 2, "{mode:?} {seed}: {}", c.jumps.len());
                for w in c.jumps.windows(2) {
                    assert!(w[0].at + w[0].length < w[1].at, "{mode:?} overlap");
                }
            }
        }
    }
}
