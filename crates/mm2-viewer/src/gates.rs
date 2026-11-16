//! The Baja and Enduro checkpoint gates (`BajaRace.cpp`): each course's
//! `<course>.wpt` lists the gates in order (0x405330 reads it), and the
//! race puts up an arch at each, the last one the finish arch (0x405020).
//! Riders must pass the gates in turn; past the last a lap is done.

use glam::Vec3;
use mm2_formats::scn;
use mm2_physics::race::Line;

/// The game takes at most 34 gates (it gives up on 35 or more).
const MOST: usize = 34;
/// The gates stand this far above the ground (0x5557e0).
pub const RAISE: f32 = 2.0;
/// Half the arch's width and its height, from `Waypnt.slb` (both arches
/// are the same size), cut down as the game cuts its gate box (0.98 of the
/// width at 0x5557e4, 0.67 of the height at 0x5557e8).
const HALF_WIDTH: f32 = 18.5 * 0.98;
const HEIGHT: f32 = 19.4 * 0.67;
/// The arches' models, in `RES/MODELS.RES`.
const GATE: &str = "waypnt.slt";
const FINISH: &str = "Finish.slt";

/// One gate: where it stands and the way through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gate {
    pub position: Vec3,
    pub look: Vec3,
}

/// A course's gates and the arch model it asks for (`DefaultSLT`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Waypoints {
    pub model: String,
    /// Level positions and directions; the heights come from the ground.
    pub points: Vec<([f32; 2], [f32; 2])>,
}

impl Waypoints {
    /// Reads a `.wpt`: `[Waypoints]` with `NumWaypoints` and, from 1,
    /// `Point<n>X`, `Point<n>Z`, `Point<n>LookX` and `Point<n>LookZ`.
    pub fn parse(text: &str) -> Waypoints {
        let ini = scn::Ini::parse(text);
        let Some(s) = ini.section("Waypoints") else {
            return Waypoints::default();
        };
        let num = |key: &str| s.get(key).and_then(|v| v.trim().parse::<f32>().ok());
        let count = num("NumWaypoints").unwrap_or(0.0) as usize;
        if count > MOST {
            return Waypoints::default();
        }
        let points = (1..=count)
            .map(|n| {
                let v = |k: &str| num(&format!("Point{n}{k}")).unwrap_or(0.0);
                ([v("X"), v("Z")], [v("LookX"), v("LookZ")])
            })
            .collect();
        Waypoints {
            model: s.get("DefaultSLT").unwrap_or(GATE).trim().to_string(),
            points,
        }
    }

    /// The gates on the ground `height` gives.
    pub fn gates(&self, height: impl Fn(f32, f32) -> f32) -> Vec<Gate> {
        self.points
            .iter()
            .map(|&([x, z], [lx, lz])| Gate {
                position: Vec3::new(x, height(x, z) + RAISE, z),
                look: Vec3::new(lx, 0.0, lz).normalize_or(Vec3::Z),
            })
            .collect()
    }

    /// The arches to put up, the last one the finish.
    pub fn placements(&self, gates: &[Gate]) -> Vec<scn::Placement> {
        gates
            .iter()
            .enumerate()
            .map(|(i, g)| scn::Placement {
                model: if i + 1 == gates.len() {
                    FINISH.to_string()
                } else {
                    self.model.clone()
                },
                position: g.position.into(),
                look: g.look.into(),
                up: [0.0, 1.0, 0.0],
                scale: [1.0; 3],
                lighting: Default::default(),
            })
            .collect()
    }
}

/// The Single Player event page's "Random Gate #" box (Baja only): how
/// many gates, 10 to 30, and the race number, 0 to 999, that seeds them
/// (game +0x2d9c, +0x2d9d and +0x2da0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomGates {
    pub count: u32,
    pub seed: u32,
}

impl RandomGates {
    pub const FEWEST: u32 = 10;
    pub const MOST: u32 = 30;
    pub const LAST_SEED: u32 = 999;
}

/// The C library's `rand` the game was built with: `srand(seed)`, then
/// 0 to 32767.
struct Rand(u32);

impl Rand {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        ((self.0 >> 16) & 0x7fff) as f32
    }
}

/// How far from the terrain's edges random gates keep, in terrain units
/// (0x555814), and the box that must be clear of solid objects (0x555800).
const EDGE: f32 = 240.0;
pub const CLEAR: f32 = 15.0;
/// Each gate is 538 to 2880 ft from the one before (the squares 290000 and
/// 8294400 at 0x55580c, 0x555808), and the last from the first too; a gate
/// is at least 300 ft from all the earlier ones (90000, 0x555804).
const NEAREST: f32 = 290_000.0;
const FARTHEST: f32 = 8_294_400.0;
const APART: f32 = 90_000.0;
/// A flat spot's ground faces up at least this much (0x405330's 0.8), and
/// the four other spots tried lie 20 ft off along x and z (0x555c14).
const FLAT: f32 = 0.8;
const ASIDE: f32 = 20.0;
/// The trees within 150 ft of a gate are taken out (0x5557ec, 0x460c50).
pub const TREES_CLEARED: f32 = 150.0;

/// The flattest of five spots round (`x`, `z`) and the way from it to the
/// one before it in flatness (0x4e66e0): the spot, then 20 ft along +x,
/// -x, +z and -z. The first two start the reckoning; each later one that
/// faces up more than the flattest so far, and past [`FLAT`], takes its
/// place and the old flattest comes second. A spot that does not can
/// still come second if it faces up past [`FLAT`] and more than the second,
/// and, as the game compares them, its ground height is under the
/// flattest one's facing (so seldom, on ground above 1 ft). None without
/// a second spot.
fn flat_spot(
    x: f32,
    z: f32,
    ground: &impl Fn(f32, f32) -> Vec3,
    height: &impl Fn(f32, f32) -> f32,
) -> Option<([f32; 2], Vec3)> {
    let spots = [
        [x, z],
        [x + ASIDE, z],
        [x - ASIDE, z],
        [x, z + ASIDE],
        [x, z - ASIDE],
    ];
    let up: Vec<f32> = spots.iter().map(|p| ground(p[0], p[1]).y).collect();
    let h: Vec<f32> = spots.iter().map(|p| height(p[0], p[1])).collect();
    // The flattest and the second, and the facing a new second must beat
    // (the flattest's while there is no second).
    let (mut best, mut second, mut beat) = if up[0] <= up[1] {
        if h[0] < up[1] && FLAT < up[0] { (1, Some(0), up[0]) } else { (1, None, up[1]) }
    } else if h[1] < up[0] && FLAT < up[1] {
        (0, Some(1), up[1])
    } else {
        (0, None, up[0])
    };
    for i in 2..spots.len() {
        if up[i] <= up[best] || up[i] <= FLAT {
            if beat < up[i] && h[i] < up[best] && FLAT < up[i] {
                second = Some(i);
                beat = up[i];
            }
        } else {
            second = Some(best);
            beat = up[best];
            best = i;
        }
    }
    let second = second?;
    let way = Vec3::new(spots[second][0] - spots[best][0], 0.0, spots[second][1] - spots[best][1]);
    Some((spots[best], way.normalize_or(Vec3::Z)))
}

impl Waypoints {
    /// Random gates (0x405330 with the Random Gate box ticked): spots
    /// tried at random over the terrain, `span` ft across with `unit` ft a
    /// terrain unit, until each is far enough from the gates before it, has
    /// no solid object within 15 ft (`blocked`), and stands on flat
    /// ground. Each gate faces the way the flat ground runs, turned to go
    /// on from the gate before (the first, towards the middle of the
    /// terrain); `ground` gives the ground's normal. The last gate is the
    /// finish. After 200 tries at one gate the turn rules are dropped.
    pub fn random(
        r: RandomGates,
        span: f32,
        unit: f32,
        ground: impl Fn(f32, f32) -> Vec3,
        height: impl Fn(f32, f32) -> f32,
        blocked: impl Fn(f32, f32) -> bool,
    ) -> Waypoints {
        let count = r.count.clamp(RandomGates::FEWEST, RandomGates::MOST) as usize;
        let mut rand = Rand(r.seed);
        let edge = EDGE * unit;
        let mut placed: Vec<(Vec3, Vec3)> = Vec::with_capacity(count);
        let mut tries = 0;
        // A course that cannot be laid out stops rather than hanging.
        let mut budget = 2_000_000u32;
        let d2 = |a: Vec3, x: f32, z: f32| (a.x - x) * (a.x - x) + (a.z - z) * (a.z - z);
        while placed.len() < count && budget > 0 {
            budget -= 1;
            let x = (span - 2.0 * edge) * rand.next() / 32768.0 + edge;
            let z = (span - 2.0 * edge) * rand.next() / 32768.0 + edge;
            if let Some(&(prev, _)) = placed.last() {
                let ok = |a: Vec3| (NEAREST..=FARTHEST).contains(&d2(a, x, z));
                if !ok(prev) || (placed.len() == count - 1 && !ok(placed[0].0)) {
                    continue;
                }
                if placed[..placed.len() - 1].iter().any(|&(g, _)| d2(g, x, z) < APART) {
                    continue;
                }
            }
            if blocked(x, z) {
                continue;
            }
            let Some(([x, z], mut look)) = flat_spot(x, z, &ground, &height) else {
                continue;
            };
            let at = Vec3::new(x, height(x, z), z);
            match placed.last() {
                None => {
                    let middle = Vec3::new(span / 2.0, 0.0, span / 2.0);
                    if (middle - at).normalize_or_zero().dot(look) < 0.0 {
                        look = -look;
                    }
                }
                Some(&(prev, prev_look)) => {
                    let on = (at - prev).normalize_or_zero();
                    tries += 1;
                    // No sharp turn back (0x5557fc's -0.5), and the gate
                    // faces well along the way from the last (0.6).
                    if tries < 200 && on.dot(prev_look) < -0.5 {
                        continue;
                    }
                    let along = on.dot(look);
                    if tries < 200 && along.abs() < 0.6 && along >= -0.5 {
                        continue;
                    }
                    if along < 0.0 {
                        look = -look;
                    }
                }
            }
            tries = 0;
            placed.push((at, look));
        }
        Waypoints {
            model: GATE.to_string(),
            points: placed.iter().map(|(p, l)| ([p.x, p.z], [l.x, l.z])).collect(),
        }
    }
}

impl Gate {
    /// The line a rider must cross, going the gate's way.
    pub fn line(&self) -> Line {
        let right = Vec3::new(self.look.z, 0.0, -self.look.x);
        Line {
            centre: self.position - Vec3::Y * RAISE,
            forward: self.look,
            half_width: right * HALF_WIDTH,
            height: HEIGHT + RAISE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_gates() {
        let w = Waypoints::parse(
            "; a comment\n[Waypoints]\nNumWaypoints=2\n\nPoint1X=10\nPoint1Z=20\n\
             Point1LookX=0.0\nPoint1LookZ=1.0\nPoint2X=30\nPoint2Z=40\nPoint2LookX=1\nPoint2LookZ=0\n",
        );
        assert_eq!(w.model, "waypnt.slt");
        assert_eq!(
            w.points,
            [([10.0, 20.0], [0.0, 1.0]), ([30.0, 40.0], [1.0, 0.0])]
        );
        let gates = w.gates(|_, _| 5.0);
        assert_eq!(gates[0].position, Vec3::new(10.0, 7.0, 20.0));
        let p = w.placements(&gates);
        assert_eq!(
            (p[0].model.as_str(), p[1].model.as_str()),
            ("waypnt.slt", "Finish.slt")
        );
        // Through the first gate going +z, not back through it.
        let l = gates[0].line();
        let (a, b) = (Vec3::new(12.0, 5.0, 19.0), Vec3::new(12.0, 5.0, 21.0));
        assert!(l.crossed(a, b, true));
        assert!(!l.crossed(b, a, true));
        assert!(!l.crossed(a + Vec3::X * 20.0, b + Vec3::X * 20.0, true));
    }

    /// With the game's files beside the engine, every Baja course has
    /// its gates.
    #[test]
    fn baja_courses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../GAME/TERAFORM/BAJA/BAJA01.ENV");
        let Ok(bytes) = crate::read_archive(&path) else {
            return;
        };
        let ar = mm2_formats::rs2::Archive::parse(&bytes).unwrap();
        let wpts: Vec<_> = ar
            .entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(".wpt"))
            .collect();
        assert_eq!(wpts.len(), 3);
        for e in wpts {
            let w = Waypoints::parse(&String::from_utf8_lossy(ar.data(e)));
            assert!(w.points.len() > 3, "{}", e.name);
        }
    }

    #[test]
    fn random_gates_keep_their_spacing() {
        let r = RandomGates { count: 12, seed: 7 };
        // Gently rolling ground: every spot faces up a little differently.
        let flat = |x: f32, z: f32| Vec3::new((x * 0.01).sin() * 0.3, 1.0, (z * 0.013).cos() * 0.3).normalize();
        let w = Waypoints::random(r, 8192.0, 8.0, flat, |_, _| 50.0, |_, _| false);
        assert_eq!(w.points.len(), 12);
        let again = Waypoints::random(r, 8192.0, 8.0, flat, |_, _| 50.0, |_, _| false);
        assert_eq!(w, again, "the same race number, the same gates");
        let d = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        for k in 1..w.points.len() {
            let gap = d(w.points[k].0, w.points[k - 1].0);
            assert!((538.0..=2881.0).contains(&gap), "{gap}");
            for j in 0..k - 1 {
                assert!(d(w.points[k].0, w.points[j].0) >= 299.0);
            }
        }
        for &([x, z], _) in &w.points {
            assert!((1900.0..=6292.0).contains(&x) && (1900.0..=6292.0).contains(&z), "a flat spot is up to 20 ft off");
        }
        let other = Waypoints::random(RandomGates { count: 12, seed: 8 }, 8192.0, 8.0, flat, |_, _| 50.0, |_, _| false);
        assert_ne!(w, other);
    }

    /// A steep slope everywhere has no flat spot: no gates, and no hang.
    #[test]
    fn random_gates_need_flat_ground() {
        let steep = |_: f32, _: f32| Vec3::new(0.8, 0.6, 0.0);
        let w = Waypoints::random(RandomGates { count: 10, seed: 1 }, 8192.0, 8.0, steep, |_, _| 0.0, |_, _| false);
        assert!(w.points.is_empty());
    }
}
