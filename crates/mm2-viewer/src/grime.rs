//! Dirt that builds up on the bikes and riders through a race (the
//! enhanced drawing's "grime" switch; looks only, none of it the game's).
//!
//! Each bike keeps a coat of four parts, all 0 to 1 and only ever growing:
//! a thin film of dust that starts on the wheels and lower bike and climbs
//! slowly up the legs as the laps go by; clumps of caked mud (mostly on wet
//! ground); spatter on the front from riding in the roost of a bike ahead;
//! and scuffs from going down, which reach up the rider's side. The coat
//! takes the colour of the ground the bike has been riding on.
//!
//! The shader's side is in objects.wgsl (`fs_enhanced`); the coat goes to
//! it in the unused corners of each model's node matrices (objects.rs).

use crate::dust::Kick;
use glam::Vec3;

/// How fast the film builds riding on dirt at about 40 ft/s: the part of
/// what is left to cover, a second. A dry three-lap race (about four
/// minutes) leaves the bikes about half covered, low down.
const FILM: f32 = 0.0035;
/// And the caked mud, riding in the dry; far faster on wet ground.
const CAKED_DRY: f32 = 0.0006;
const CAKED_WET: f32 = 0.005;
/// The most mud the dry ground cakes on by itself.
const CAKED_DRY_MOST: f32 = 0.3;
/// How far behind a bike its roost reaches, ft, and how wide it spreads
/// at its far end, ft either side.
const ROOST_REACH: f32 = 45.0;
const ROOST_WIDE: f32 = 7.0;
/// How fast riding in the thick of a bike's roost spatters the front.
const SPATTER: f32 = 0.03;

/// The coat on one bike and its rider.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coat {
    pub film: f32,
    pub caked: f32,
    pub spatter: f32,
    pub scuffs: f32,
    /// The dirt's colour (the ground's painted colour, as its texture
    /// stores it), browned a little.
    pub colour: Vec3,
}

impl Coat {
    /// The coat packed for the shader: the caked mud, spatter and scuffs
    /// in one number, and the film and colour (as a negative number, so
    /// the model still reads as moving) in another, each a whole number a
    /// float holds exactly.
    pub fn packed(&self) -> (f32, f32) {
        let q = |v: f32, most: f32| (v.clamp(0.0, 1.0) * most).round();
        let a = q(self.caked, 255.0) + q(self.spatter, 255.0) * 256.0 + q(self.scuffs, 63.0) * 65536.0;
        let c = self.colour;
        let rgb = q(c.x, 31.0) * 1024.0 + q(c.y, 31.0) * 32.0 + q(c.z, 31.0);
        let b = -(1.0 + q(self.film, 255.0) * 32768.0 + rgb);
        (a, b)
    }
}

/// What is kept of a bike from the step before.
#[derive(Clone, Copy, Default)]
struct Seen {
    /// The rear tyre (drawing coordinates) and the way it was riding.
    rear: Option<Vec3>,
    heading: Vec3,
    speed: f32,
    crashed: bool,
    /// The colour taken up yet (none before it first touches ground).
    coloured: bool,
}

/// Every bike's coat.
#[derive(Default)]
pub struct Grime {
    coats: Vec<Coat>,
    seen: Vec<Seen>,
    /// How dirty the bikes start (`--enhanced=mud=`).
    start: f32,
}

/// A plain dirt colour, for a bike that hasn't touched the ground yet.
pub const DIRT: Vec3 = Vec3::new(0.45, 0.36, 0.27);

/// The ground's painted colour turned into the dirt it would leave: grass
/// and sand alike browned toward plain dirt, and neither too pale nor too
/// dark, so the coat stays soft.
pub fn dirt_from(ground: Vec3) -> Vec3 {
    let lum = ground.dot(Vec3::new(0.3, 0.59, 0.11)).clamp(0.2, 0.6);
    let brown = DIRT * (lum / 0.37);
    ground.lerp(brown, 0.35).clamp(Vec3::splat(0.08), Vec3::splat(0.7))
}

/// The painted ground's colour tile by tile (the terrain's tile map), as
/// its textures store it.
pub struct GroundColours {
    pub cells: Vec<Vec3>,
    /// Tiles a side, and ft each spans.
    pub side: u32,
    pub span: f32,
}

impl GroundColours {
    /// The colour under drawing point `at`.
    pub fn at(&self, at: Vec3) -> Vec3 {
        let (x, z) = (at.x / self.span, -at.z / self.span);
        if !(x >= 0.0 && z >= 0.0) || self.side == 0 {
            return DIRT;
        }
        let (x, z) = ((x as u32).min(self.side - 1), (z as u32).min(self.side - 1));
        self.cells.get((z * self.side + x) as usize).copied().unwrap_or(DIRT)
    }
}

/// A texture's average colour, from one of its smaller levels.
pub fn average_colour(image: &crate::hires::Image) -> Vec3 {
    let level = image.levels.iter().find(|l| l.len() <= 64 * 64 * 4 && l.len() >= 4).or(image.levels.last());
    let Some(px) = level else { return DIRT };
    let (mut sum, mut weight) = (Vec3::ZERO, 0.0);
    for p in px.chunks_exact(4) {
        let a = p[3] as f32 / 255.0;
        sum += Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0 * a;
        weight += a;
    }
    if weight > 0.0 {
        sum / weight
    } else {
        DIRT
    }
}

impl Grime {
    pub fn new(start: f32) -> Self {
        Grime { start: start.clamp(0.0, 1.0), ..Default::default() }
    }

    /// Bike `bike`'s coat as it stands.
    pub fn coat(&self, bike: usize) -> Coat {
        self.coats.get(bike).copied().unwrap_or_else(|| self.fresh())
    }

    fn fresh(&self) -> Coat {
        Coat { film: self.start, caked: self.start * 0.5, colour: DIRT, ..Default::default() }
    }

    /// One step of `dt` s: where each bike's tyres touch the ground
    /// (drawing coordinates) and how it kicks up dirt, whether the ground
    /// is wet, and the ground's colour at a place (drawing coordinates).
    pub fn step(&mut self, feed: &[([Option<Vec3>; 2], Kick)], dt: f32, wet: bool, ground: impl Fn(Vec3) -> Vec3) {
        if dt <= 0.0 {
            return;
        }
        let fresh = self.fresh();
        self.coats.resize(feed.len(), fresh);
        self.seen.resize(feed.len(), Seen::default());
        // Where each bike is now, the way it rides, and how much it throws
        // back off its rear tyre, first: the roost reaches the others.
        let mut now: Vec<Seen> = self.seen.clone();
        for (s, (tyres, kick)) in now.iter_mut().zip(feed) {
            let rear = tyres[1].or(tyres[0]);
            if let (Some(a), Some(b)) = (s.rear, rear) {
                let moved = b - a;
                let speed = moved.length() / dt;
                // A jump back to the track (a reset) is no ride.
                if speed < 200.0 {
                    s.speed = speed;
                    if speed > 3.0 {
                        s.heading = Vec3::new(moved.x, 0.0, moved.z).normalize_or(s.heading);
                    }
                }
            }
            if let [Some(f), Some(r)] = *tyres {
                s.heading = Vec3::new(f.x - r.x, 0.0, f.z - r.z).normalize_or(s.heading);
            }
            if rear.is_none() {
                s.speed = 0.0;
            }
            s.rear = rear.or(s.rear);
            s.crashed = kick.crashed;
        }
        let roost: Vec<Option<(Vec3, Vec3, f32)>> = now
            .iter()
            .zip(feed)
            .map(|(s, (tyres, kick))| {
                let rear = tyres[1]?;
                let throws = if kick.dirt { 1.0 } else if kick.dusty { 0.4 } else { 0.0 };
                let how = throws * ((s.speed / 50.0).min(1.0) * 0.6 + kick.spin * 0.8);
                (how > 0.05).then_some((rear, s.heading, how))
            })
            .collect();
        for (i, (coat, (tyres, kick))) in self.coats.iter_mut().zip(feed).enumerate() {
            let (before, s) = (self.seen[i], now[i]);
            let down = tyres[0].is_some() || tyres[1].is_some();
            let pace = (s.speed / 40.0).min(1.5);
            let pick = if kick.dirt { 1.0 } else if kick.dusty { 0.6 } else { 0.15 };
            let wet_more = if wet { 2.5 } else { 1.0 };
            let grow = |v: &mut f32, rate: f32, most: f32| {
                if *v < most {
                    *v = (*v + (most - *v) * (rate * dt).min(1.0)).min(most);
                }
            };
            if down {
                // The film: from the riding, more as the rear tyre spins.
                grow(&mut coat.film, (FILM * pace + 0.004 * kick.spin) * pick * wet_more, 1.0);
                if wet {
                    grow(&mut coat.caked, CAKED_WET * pace * pick, 1.0);
                } else {
                    grow(&mut coat.caked, CAKED_DRY * pace * pick, CAKED_DRY_MOST);
                }
                // The colour follows the ground ridden on, over a minute or so.
                if let Some(at) = tyres[1].or(tyres[0]) {
                    let c = dirt_from(ground(at));
                    let follow = if s.coloured { (0.02 * pace * pick * dt).min(1.0) } else { 1.0 };
                    coat.colour = coat.colour.lerp(c, follow);
                    now[i].coloured = true;
                }
            }
            // Riding in the roost of a bike ahead: the front spattered.
            if let Some(me) = s.rear.filter(|_| !s.crashed) {
                let mut hit = 0.0f32;
                for (j, r) in roost.iter().enumerate() {
                    let Some((at, way, how)) = *r else { continue };
                    if j == i {
                        continue;
                    }
                    let off = me - at;
                    let behind = -off.dot(way);
                    if !(1.5..ROOST_REACH).contains(&behind) || off.y.abs() > 8.0 {
                        continue;
                    }
                    let side = (off + way * behind).length();
                    let wide = 1.5 + ROOST_WIDE * behind / ROOST_REACH;
                    if side < wide {
                        hit += how * (1.0 - behind / ROOST_REACH) * (1.0 - side / wide);
                    }
                }
                let hit = hit.min(1.5);
                if hit > 0.0 {
                    grow(&mut coat.spatter, SPATTER * hit * wet_more, 1.0);
                    grow(&mut coat.film, FILM * 0.6 * hit, 1.0);
                    let most = if wet { 1.0 } else { CAKED_DRY_MOST };
                    grow(&mut coat.caked, if wet { 0.01 } else { 0.0015 } * hit, most);
                }
            }
            // Going down: a scuff of dirt up the side, more caked on.
            if s.crashed && !before.crashed {
                let hard = 0.5 + 0.5 * (before.speed / 50.0).min(1.0);
                coat.scuffs = (coat.scuffs + (1.0 - coat.scuffs) * 0.45 * hard).min(1.0);
                coat.film = (coat.film + (1.0 - coat.film) * 0.12 * hard).min(1.0);
                let most = if wet { 1.0 } else { 0.6 };
                coat.caked = coat.caked.max((coat.caked + if wet { 0.25 } else { 0.1 } * hard).min(most));
                coat.spatter = (coat.spatter + 0.15 * hard).min(1.0);
            }
        }
        self.seen = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bike riding straight along x at `speed` ft/s, its rear tyre at `x`.
    fn riding(x: f32, z: f32) -> [Option<Vec3>; 2] {
        [Some(Vec3::new(x + 5.0, 0.0, z)), Some(Vec3::new(x, 0.0, z))]
    }

    fn dirt() -> Kick {
        Kick { dusty: true, dirt: true, ..Default::default() }
    }

    #[test]
    fn builds_up_slowly_and_never_shrinks() {
        let mut g = Grime::new(0.0);
        let mut last = Coat::default();
        for f in 0..(240 * 30) {
            let x = f as f32 * 40.0 / 30.0;
            g.step(&[(riding(x, 0.0), dirt())], 1.0 / 30.0, false, |_| Vec3::new(0.5, 0.4, 0.3));
            let c = g.coat(0);
            assert!(c.film >= last.film && c.caked >= last.caked);
            if f == 30 * 30 {
                assert!(c.film < 0.15, "half a minute in: {c:?}");
            }
            last = c;
        }
        // Four minutes in the dry: about half the film, little mud.
        assert!((0.4..0.75).contains(&last.film), "{last:?}");
        assert!(last.caked < 0.2, "{last:?}");
        assert_eq!(last.spatter, 0.0);
    }

    #[test]
    fn wet_ground_cakes_on_faster() {
        let ride = |wet| {
            let mut g = Grime::new(0.0);
            for f in 0..(120 * 30) {
                let x = f as f32 * 40.0 / 30.0;
                g.step(&[(riding(x, 0.0), dirt())], 1.0 / 30.0, wet, |_| DIRT);
            }
            g.coat(0)
        };
        let (dry, wet) = (ride(false), ride(true));
        assert!(wet.caked > dry.caked * 4.0, "{dry:?} {wet:?}");
        assert!(wet.film > dry.film);
    }

    #[test]
    fn riding_in_roost_spatters_the_front() {
        let mut g = Grime::new(0.0);
        let spin = Kick { spin: 0.6, ..dirt() };
        for f in 0..(30 * 30) {
            let x = f as f32 * 40.0 / 30.0;
            // Bike 0 leads, bike 1 follows 15 ft behind it, bike 2 rides
            // alongside 30 ft off the line.
            g.step(&[(riding(x, 0.0), spin), (riding(x - 20.0, 0.0), dirt()), (riding(x, 30.0), dirt())], 1.0 / 30.0, false, |_| DIRT);
        }
        assert!(g.coat(1).spatter > 0.3, "{:?}", g.coat(1));
        assert_eq!(g.coat(0).spatter, 0.0);
        assert_eq!(g.coat(2).spatter, 0.0);
        assert!(g.coat(1).film > g.coat(2).film);
    }

    #[test]
    fn a_crash_scuffs_once() {
        let mut g = Grime::new(0.0);
        let crashed = Kick { crashed: true, ..Default::default() };
        g.step(&[(riding(0.0, 0.0), dirt())], 0.1, false, |_| DIRT);
        g.step(&[(riding(4.0, 0.0), dirt())], 0.1, false, |_| DIRT);
        g.step(&[([None; 2], crashed)], 0.1, false, |_| DIRT);
        let once = g.coat(0);
        assert!(once.scuffs > 0.2 && once.caked > 0.05, "{once:?}");
        for _ in 0..10 {
            g.step(&[([None; 2], crashed)], 0.1, false, |_| DIRT);
        }
        assert_eq!(g.coat(0).scuffs, once.scuffs);
    }

    #[test]
    fn takes_the_grounds_colour() {
        let mut g = Grime::new(0.0);
        let red = Vec3::new(0.6, 0.25, 0.15);
        g.step(&[(riding(0.0, 0.0), dirt())], 0.1, false, |_| red);
        let c = g.coat(0).colour;
        assert!(c.x > c.y && c.y > c.z, "{c}");
    }

    #[test]
    fn packs_exactly() {
        let c = Coat { film: 1.0, caked: 1.0, spatter: 1.0, scuffs: 1.0, colour: Vec3::ONE };
        let (a, b) = c.packed();
        assert!(a < 16_777_216.0 && -b < 16_777_216.0);
        assert_eq!(a as u32 & 255, 255);
        assert!(b <= -1.0);
        assert_eq!(Coat::default().packed().1, -1.0);
    }
}
