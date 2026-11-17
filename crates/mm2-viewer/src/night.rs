//! A stadium at night under its own lights (graphics `nightlights`): the
//! lamp banks high round the floor are real lights, each lighting the
//! track, the bikes and the riders by how far off it is and how squarely
//! it meets them, and the banks gathered into a few groups, each casting
//! the riders' shadows across the floor from where it hangs.
//!
//! Where the lamps are: a track may list them in a `<stem>.lights` file
//! beside its `.ENV` (the Free Pack's arenas do); otherwise the near-white
//! lamp texels painted high on a domed stadium's roof (as the ray tracing
//! finds them, objects.rs); otherwise a ring of banks round the floor.

use glam::{Mat4, Vec3};
use mm2_formats::trn::Terrain;

/// The most lamps the shaders light with (enhanced.wgsl `stadium_lamps`).
pub const MOST_LAMPS: usize = 24;
/// The shadow groups: one a cascade layer, as the sun's shadows are not
/// drawn while the lamps light the floor.
pub const GROUPS: usize = crate::enhance::CASCADES;
/// How much light the lamps together put on the floor's middle, against
/// the game's full light (1): a little under the day's, as a lit arena is.
const FLOOR_LIGHT: f32 = 1.4;
/// A lamp's beam, half across, degrees, when its file does not say.
const BEAM: f32 = 40.0;

/// One lamp bank: where it hangs, the way it points (drawing
/// coordinates), how strong it is against the rest, and the cosine of its
/// beam's half-angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub at: Vec3,
    pub way: Vec3,
    pub power: f32,
    pub edge: f32,
}

/// The lamps a `<stem>.lights` file beside `env` lists, if there is one:
/// a lamp a line, `x y z  dx dy dz  power  [beam]` in game feet (the way
/// it points, its strength against a typical bank 1, and its beam's
/// half-angle in degrees), `#` starting a comment.
pub fn read_lights(env: &std::path::Path) -> Option<Vec<Lamp>> {
    let path = env.with_extension("lights");
    let path = if path.exists() { path } else { env.with_extension("LIGHTS") };
    let text = std::fs::read_to_string(&path).ok()?;
    let lamps = parse_lights(&text);
    eprintln!("night lights: {} lamps from {}", lamps.len(), path.display());
    (!lamps.is_empty()).then_some(lamps)
}

/// The lamps a `.lights` file's `text` lists (see [`read_lights`]); lines
/// that do not read as one are passed over.
pub fn parse_lights(text: &str) -> Vec<Lamp> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let v: Vec<f32> = line.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect::<Option<_>>()?;
            if v.len() < 7 {
                return None;
            }
            let at = crate::renderer::from_game(Vec3::new(v[0], v[1], v[2]));
            let way = crate::renderer::from_game(Vec3::new(v[3], v[4], v[5])).try_normalize()?;
            let beam = v.get(7).copied().unwrap_or(BEAM).clamp(5.0, 85.0);
            Some(Lamp { at, way, power: v[6].max(0.0), edge: beam.to_radians().cos() })
        })
        .take(MOST_LAMPS)
        .collect()
}

/// The track's floor: its bounds (drawing coordinates), and its height
/// in their y.
#[derive(Clone, Copy, Debug)]
pub struct Floor {
    pub low: Vec3,
    pub high: Vec3,
}

impl Floor {
    /// Round the course's cross-sections (game coordinates), its edges
    /// included.
    pub fn round_course(points: impl Iterator<Item = [f32; 3]>, terrain: &Terrain) -> Option<Floor> {
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in points {
            let p = crate::renderer::from_game(Vec3::from(p));
            low = low.min(p);
            high = high.max(p);
        }
        // The course's own heights may all be naught: the ground's in its
        // middle.
        let y = ground(terrain, (low.x + high.x) * 0.5, (low.z + high.z) * 0.5);
        (high.x > low.x && high.z > low.z).then(|| Floor { low: Vec3::new(low.x, y, low.z), high: Vec3::new(high.x, y, high.z) })
    }

    /// Without a course, the middle of the land.
    pub fn guess(terrain: &Terrain) -> Floor {
        let side = (terrain.size - 1) as f32 * terrain.scale;
        let (cx, cz) = (side * 0.5, -side * 0.5);
        let y = ground(terrain, cx, cz);
        Floor {
            low: Vec3::new(cx - side * 0.3, y, cz - side * 0.2),
            high: Vec3::new(cx + side * 0.3, y, cz + side * 0.2),
        }
    }

    pub fn centre(&self) -> Vec3 {
        (self.low + self.high) * 0.5
    }

    fn half(&self) -> Vec3 {
        (self.high - self.low) * 0.5
    }

    /// Where a lamp at `at` aims: toward the floor's middle from beneath
    /// it, halfway, but never past the floor's own edge, so the banks
    /// round about between them light all of it.
    fn aim(&self, at: Vec3) -> Vec3 {
        let c = self.centre();
        let h = self.half() * 0.6;
        Vec3::new(c.x + ((at.x - c.x) * 0.5).clamp(-h.x, h.x), c.y, c.z + ((at.z - c.z) * 0.5).clamp(-h.z, h.z))
    }
}

/// Lamps from the banks painted on a domed stadium's roof (objects.rs
/// `floodlights`: each bank's middle, its spread and how many lamp texels
/// it holds), each pointing at the floor.
pub fn from_banks(banks: &[(Vec3, f32, f32)], floor: &Floor) -> Vec<Lamp> {
    let most = banks.iter().map(|b| b.2).fold(1.0, f32::max);
    banks
        .iter()
        .take(MOST_LAMPS)
        .map(|&(at, _, n)| Lamp {
            at,
            way: (floor.aim(at) - at).normalize_or(Vec3::NEG_Y),
            // A bigger bank is brighter, but not by as much as it has more
            // texels.
            power: (n / most).sqrt().max(0.35),
            edge: 50f32.to_radians().cos(),
        })
        .collect()
}

/// A ring of banks round the floor where a stadium neither lists nor
/// paints its lamps: twelve, 120 ft up, a little out past its edges.
pub fn ring(floor: &Floor) -> Vec<Lamp> {
    let c = floor.centre();
    let h = floor.half();
    (0..12)
        .map(|i| {
            let a = (i as f32 + 0.5) / 12.0 * std::f32::consts::TAU;
            let at = Vec3::new(c.x + (h.x + 40.0) * a.cos(), c.y + 120.0, c.z + (h.z + 40.0) * a.sin());
            Lamp {
                at,
                way: (floor.aim(at) - at).normalize(),
                power: 1.0,
                edge: 55f32.to_radians().cos(),
            }
        })
        .collect()
}

/// The ground's height at drawing (x, z), ft.
fn ground(terrain: &Terrain, x: f32, z: f32) -> f32 {
    let s = terrain.scale;
    let n = terrain.size;
    let c = ((x / s).round().max(0.0) as usize).min(n - 1);
    let r = ((-z / s).round().max(0.0) as usize).min(n - 1);
    terrain.heights[r * n + c] * s
}

/// The lamps ready for the shaders: their strengths set so the floor's
/// middle gets [`FLOOR_LIGHT`], each in one of [`GROUPS`] shadow groups,
/// and each group's view of the floor.
#[derive(Clone)]
pub struct StadiumLights {
    pub lamps: Vec<Lamp>,
    /// Each lamp's shadow group.
    pub group: Vec<usize>,
    /// From the drawing to each group's shadow map, as seen from the
    /// group's middle looking at the floor.
    pub views: [Mat4; GROUPS],
    /// The ft a shadow texel spans on the floor, for each group.
    pub texels: [f32; GROUPS],
    /// The lowest lamp's height, ft: what is painted near-white above it
    /// is a lamp, and glows.
    pub lowest: f32,
}

impl StadiumLights {
    pub fn new(mut lamps: Vec<Lamp>, floor: &Floor) -> Option<StadiumLights> {
        lamps.truncate(MOST_LAMPS);
        if lamps.is_empty() {
            return None;
        }
        let centre = floor.centre();
        let half = floor.half();
        // How much they light the floor, as the shader would, over a grid
        // across it, so their strengths can be set to give it its light.
        let mut total = 0.0;
        for i in -2..=2 {
            for k in -2..=2 {
                let p = centre + Vec3::new(i as f32 * half.x, 0.0, k as f32 * half.z) * 0.35;
                total += lamps.iter().map(|l| l.power * beam(l, p, Vec3::Y)).sum::<f32>() / 25.0;
            }
        }
        if total <= 1e-6 {
            return None;
        }
        let floor = centre;
        let reach = lamps.iter().map(|l| Vec3::new(l.at.x - floor.x, 0.0, l.at.z - floor.z).length()).fold(half.x.max(half.z), f32::max);
        let scale = FLOOR_LIGHT / total;
        for l in &mut lamps {
            l.power *= scale;
        }
        // Into groups by the way round the floor each hangs: the
        // brightest lamps' bearings start the groups.
        let bearing = |l: &Lamp| (l.at.z - floor.z).atan2(l.at.x - floor.x);
        let group: Vec<usize> = lamps
            .iter()
            .map(|l| {
                let b = bearing(l).rem_euclid(std::f32::consts::TAU);
                ((b / std::f32::consts::TAU * GROUPS as f32) as usize).min(GROUPS - 1)
            })
            .collect();
        let mut views = [Mat4::IDENTITY; GROUPS];
        let mut texels = [1.0; GROUPS];
        for g in 0..GROUPS {
            let mine: Vec<&Lamp> = lamps.iter().zip(&group).filter(|(_, &k)| k == g).map(|(l, _)| l).collect();
            if mine.is_empty() {
                continue;
            }
            // From the group's power-weighted middle.
            let weight: f32 = mine.iter().map(|l| l.power).sum::<f32>().max(1e-6);
            let eye = mine.iter().map(|l| l.at * l.power).sum::<Vec3>() / weight;
            let to = floor - eye;
            let dist = to.length().max(1.0);
            let up = if to.normalize().y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
            // Wide enough to see the whole floor from there.
            let cover = half.x.hypot(half.z) * 1.1 + 20.0;
            let angle = (cover / dist).atan().min(1.3);
            let view = Mat4::look_at_rh(eye, floor, up);
            // Its depth only over the floor and what stands on it, so a
            // foot is still told apart that far off.
            let proj = Mat4::perspective_rh(angle * 2.0, 1.0, (dist - cover * 1.2).max(10.0), dist + cover * 1.2);
            views[g] = proj * view;
            texels[g] = 2.0 * dist * angle.tan() / crate::enhance::SHADOW_SIDE as f32;
        }
        let lowest = lamps.iter().map(|l| l.at.y).fold(f32::MAX, f32::min);
        eprintln!("night lights: {} lamps in {} groups, floor at {:.0} ft", lamps.len(), GROUPS, floor.y);
        if std::env::var_os("MM2_NIGHT_DEBUG").is_some() {
            eprintln!("  floor {floor:?} reach {reach:.0}");
            for (l, g) in lamps.iter().zip(&group) {
                eprintln!("  lamp at {:?} way {:?} power {:.3} edge {:.2} group {g}", l.at, l.way, l.power, l.edge);
            }
        }
        Some(StadiumLights { lamps, group, views, texels, lowest })
    }

    /// The lamps for enhanced.wgsl: xyz where, w power; xyz the way, w the
    /// beam's edge plus 10 times the shadow group.
    pub fn uniform(&self) -> ([[f32; 4]; MOST_LAMPS], [[f32; 4]; MOST_LAMPS]) {
        let mut at = [[0.0; 4]; MOST_LAMPS];
        let mut way = [[0.0; 4]; MOST_LAMPS];
        for (i, l) in self.lamps.iter().enumerate() {
            at[i] = [l.at.x, l.at.y, l.at.z, l.power];
            way[i] = [l.way.x, l.way.y, l.way.z, l.edge + 10.0 * self.group[i] as f32];
        }
        (at, way)
    }
}

/// The light lamp `l` (at power 1) puts on a surface at `p` facing `n`,
/// as enhanced_fns.wgsl's `stadium_light` gives it, unshadowed.
fn beam(l: &Lamp, p: Vec3, n: Vec3) -> f32 {
    let to = l.at - p;
    let d = to.length().max(1.0);
    let dir = to / d;
    let full = 1.0 - (1.0 - l.edge) * 0.35;
    let t = ((l.way.dot(-dir) - l.edge) / (full - l.edge)).clamp(0.0, 1.0);
    let cone = t * t * (3.0 - 2.0 * t);
    cone * n.dot(dir).max(0.0) * FALLOFF / (d * d + FALLOFF)
}

/// Lamps are compared at this square distance (ft²): only the ratios
/// matter, as the strengths are set by the floor's light.
pub const FALLOFF: f32 = 10000.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_lights_file() {
        let l = parse_lights("# a header\n774.0 32.0 961.5  0.884 -0.117 0.454  0.60  35\nnonsense\n1 2 3 0 -1 0 1\n");
        assert_eq!(l.len(), 2);
        // Game z is drawn as -z.
        assert_eq!(l[0].at, Vec3::new(774.0, 32.0, -961.5));
        assert!((l[0].way.length() - 1.0).abs() < 1e-4);
        assert!((l[0].edge - 35f32.to_radians().cos()).abs() < 1e-5);
        assert!((l[1].edge - BEAM.to_radians().cos()).abs() < 1e-5);
    }
}
