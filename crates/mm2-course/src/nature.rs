//! The living land: plants painted with the plant brush, and rivers.
//!
//! Plants go into the track as the game's own eco-system (`.esb`), the
//! trees, bushes and grass clumps of `RES/ECO.RES` (or the free pack's
//! stand-ins), so they show in the original game as well. Rivers are cut
//! into the ground with a muddy, dragging bed the original game rides too;
//! their water, splashes and sound are OpenMCM2's own, from the track's
//! `.wtr` file (which the original game leaves alone).

use crate::bake::Field;
use crate::{Course, Flora, LandKind, Mode, River, Surface};

/// A plant species of the eco-system.
#[derive(Debug, Clone, PartialEq)]
pub struct Species {
    /// The `ECO.RES` model, without its extension.
    pub name: &'static str,
    /// Height and width, ft: middle, smallest, largest.
    pub height: [f32; 3],
    pub width: [f32; 3],
    /// A tree's trunk radius (solid), or none for a bush.
    pub trunk: Option<f32>,
    /// How common, within its kind.
    pub weight: f32,
    pub flora: Flora,
}

const fn tree(name: &'static str, h: f32, w: f32, trunk: f32, weight: f32) -> Species {
    Species { name, height: [h, h * 0.75, h * 1.3], width: [w, w * 0.75, w * 1.3], trunk: Some(trunk), weight, flora: Flora::Trees }
}

const fn bush(name: &'static str, h: f32, w: f32, weight: f32) -> Species {
    Species { name, height: [h, h * 0.7, h * 1.4], width: [w, w * 0.7, w * 1.4], trunk: None, weight, flora: Flora::Bushes }
}

const fn grass(name: &'static str, h: f32, w: f32, weight: f32) -> Species {
    Species { name, height: [h, h * 0.7, h * 1.4], width: [w, w * 0.7, w * 1.4], trunk: None, weight, flora: Flora::Grass }
}

/// The plants that grow on a land (a stadium's floor takes the meadow's).
pub fn species(course: &Course) -> Vec<Species> {
    let kind = if course.mode == Mode::Supercross { LandKind::Flat } else { course.land.kind };
    match kind {
        LandKind::Desert => vec![
            tree("Saguaro01", 28.0, 7.0, 1.1, 1.5),
            tree("Saguaro03", 22.0, 6.0, 1.0, 1.5),
            tree("Ocotillo01", 12.0, 6.0, 0.4, 1.0),
            tree("DryMesquite01", 12.0, 12.0, 0.5, 2.0),
            bush("DryTumbleweed02", 3.0, 3.0, 2.0),
            bush("Tumbleweed01", 3.0, 3.0, 1.0),
            bush("Shrub03", 3.0, 5.0, 3.0),
            grass("Grass03", 1.8, 4.5, 1.0),
        ],
        LandKind::Mountains => vec![
            tree("Pine02", 46.0, 13.0, 0.9, 4.0),
            tree("Pine03", 52.0, 14.0, 1.0, 3.0),
            tree("Pine01", 44.0, 14.0, 0.9, 3.0),
            tree("Aspen02", 28.0, 11.0, 0.7, 1.5),
            bush("Shrub03", 3.0, 5.0, 3.0),
            bush("WideLeaf02", 4.0, 4.0, 2.0),
            grass("Grass02", 1.5, 4.0, 2.0),
            grass("Grass01", 1.5, 4.0, 1.0),
        ],
        LandKind::Valley => vec![
            tree("Aspen01", 32.0, 13.0, 0.7, 3.0),
            tree("Aspen03", 28.0, 12.0, 0.7, 3.0),
            tree("Tree03", 26.0, 20.0, 0.9, 2.0),
            tree("Pine01", 44.0, 14.0, 0.9, 1.5),
            bush("Shrub02", 4.0, 6.0, 3.0),
            bush("WideLeaf03", 4.0, 4.0, 2.0),
            grass("Grass03", 2.0, 5.0, 2.0),
            grass("Grass01", 1.5, 4.0, 1.0),
        ],
        LandKind::Rolling | LandKind::Flat => vec![
            tree("Aspen01", 30.0, 12.0, 0.7, 2.0),
            tree("Aspen02", 34.0, 13.0, 0.7, 2.0),
            tree("Tree01", 26.0, 20.0, 0.9, 2.0),
            tree("Tree02", 28.0, 20.0, 0.9, 1.5),
            bush("Shrub01", 4.0, 6.0, 3.0),
            bush("WideLeaf01", 4.0, 4.0, 2.0),
            grass("Grass02", 2.0, 5.0, 3.0),
            grass("Grass01", 1.5, 4.0, 2.0),
        ],
    }
}

/// Plants a square foot at full density.
fn most(flora: Flora) -> f32 {
    match flora {
        Flora::Trees => 1.0 / 260.0,
        Flora::Bushes => 1.0 / 110.0,
        Flora::Grass => 1.0 / 30.0,
        Flora::Mixed => 0.0,
    }
}

/// The most plants a track holds (the original's Baja deserts hold about
/// 40,000).
pub const MOST_PLANTS: usize = 60_000;

/// Feet a side of the density maps' cells.
const CELL: f32 = 8.0;

/// One plant: its species' slot, place (ft) and size bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plant {
    pub species: u8,
    pub at: [f32; 3],
    pub height: u8,
    pub width: u8,
}

/// How thick each kind grows over the land, from the brush strokes in
/// order: (cells a side, the trees', bushes' and grass's maps).
fn density(course: &Course) -> (usize, [Vec<f32>; 3]) {
    let side = course.mode.side();
    let n = (side / CELL).ceil() as usize + 1;
    let mut maps = [vec![0.0f32; n * n], vec![0.0f32; n * n], vec![0.0f32; n * n]];
    for s in &course.plants {
        let shares: [f32; 3] = match s.flora {
            Flora::Trees => [1.0, 0.0, 0.0],
            Flora::Bushes => [0.0, 1.0, 0.0],
            Flora::Grass => [0.0, 0.0, 1.0],
            Flora::Mixed => [0.55, 0.8, 1.0],
        };
        let d = s.density.clamp(0.0, 1.0);
        let mut put = |k: usize, f: f32| {
            for (m, share) in maps.iter_mut().zip(shares) {
                if share == 0.0 {
                    continue;
                }
                if s.clear {
                    m[k] *= 1.0 - f * share;
                } else {
                    m[k] = m[k].max(d * share * f);
                }
            }
        };
        if s.everywhere {
            for k in 0..n * n {
                put(k, 1.0);
            }
            continue;
        }
        let r = s.radius.max(CELL);
        for c in &s.dabs {
            crate::bake::each_within(n, CELL, *c, r, |k, f| put(k, (f * 1.6).min(1.0)));
        }
    }
    (n, maps)
}

/// A small, steady random number source.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// The plants the brush strokes grow on `field`: off the track, out of
/// the water.
pub fn plants(course: &Course, field: &Field) -> Vec<Plant> {
    if course.plants.is_empty() {
        return Vec::new();
    }
    let all = species(course);
    let (n, maps) = density(course);
    let side = course.mode.side();
    let mut out = Vec::new();
    for (layer, flora) in [Flora::Trees, Flora::Bushes, Flora::Grass].into_iter().enumerate() {
        let map = &maps[layer];
        if map.iter().all(|&d| d <= 0.0) {
            continue;
        }
        let kinds: Vec<(usize, &Species)> = all.iter().enumerate().filter(|(_, s)| s.flora == flora).collect();
        if kinds.is_empty() {
            continue;
        }
        let total: f32 = kinds.iter().map(|(_, s)| s.weight).sum();
        let spacing = 1.0 / most(flora).sqrt();
        let cells = (side / spacing) as usize;
        let mut dice = Dice(0x5eed_0000 + layer as u64 * 7919 + course.land.seed as u64);
        // Trees keep further from the track than grass does.
        let clear = match flora {
            Flora::Trees => 1.9,
            Flora::Bushes => 1.45,
            _ => 1.2,
        };
        for cz in 0..cells {
            for cx in 0..cells {
                let (a, b, keep, pick, h, w) = (dice.next(), dice.next(), dice.next(), dice.next(), dice.next(), dice.next());
                let x = (cx as f32 + a) * spacing;
                let z = (cz as f32 + b) * spacing;
                let k = ((z / CELL).round() as usize).min(n - 1) * n + ((x / CELL).round() as usize).min(n - 1);
                if keep >= map[k] {
                    continue;
                }
                let f = field_index(field, x, z);
                if field.track[f] < clear || !field.water[f].is_nan() {
                    continue;
                }
                let mut at = pick * total;
                let slot = kinds
                    .iter()
                    .find(|(_, s)| {
                        at -= s.weight;
                        at <= 0.0
                    })
                    .map_or(kinds[0].0, |(i, _)| *i);
                out.push(Plant { species: slot as u8, at: [x, field.height(x, z), z], height: (h * 255.0) as u8, width: (w * 255.0) as u8 });
            }
        }
    }
    if out.len() > MOST_PLANTS {
        // Thinned evenly to fit.
        let keep = MOST_PLANTS as f32 / out.len() as f32;
        let mut dice = Dice(77);
        out.retain(|_| dice.next() < keep);
    }
    out
}

fn field_index(field: &Field, x: f32, z: f32) -> usize {
    let i = |v: f32| ((v / field.step).round().max(0.0) as usize).min(field.n - 1);
    i(z) * field.n + i(x)
}

/// The eco-system file (`.esb`, method 1), `species` in slots from
/// `first` (the slots before it left empty, or `keep`'s), the plants
/// after `keep`'s own.
pub fn esb(side: f32, species: &[Species], plants: &[Plant], keep: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    let string = |out: &mut Vec<u8>, s: &str| {
        out.push(s.len() as u8 + 1);
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    };
    // A track's own eco-system: its species and plants go first, ours in
    // the slots it leaves free.
    let kept = keep.and_then(|b| Kept::read(b));
    let free: Vec<u8> = (0..=255u8).filter(|s| kept.as_ref().is_none_or(|k| k.species[*s as usize].is_none())).collect();
    let slots: Vec<u8> = free.iter().copied().take(species.len()).collect();
    out.extend_from_slice(&kept.as_ref().map_or(1u32, |k| k.method).to_le_bytes());
    for slot in 0..256usize {
        if let Some(bytes) = kept.as_ref().and_then(|k| k.species[slot].as_ref()) {
            out.extend_from_slice(bytes);
            continue;
        }
        let Some(sp) = slots.iter().position(|&s| s as usize == slot).map(|i| &species[i]) else {
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
        if kept.as_ref().is_some_and(|k| k.method == 2) {
            // The authored kind's extra numbers, and no picture.
            for _ in 0..9 {
                out.extend_from_slice(&0f32.to_le_bytes());
            }
            out.push(0);
        }
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
    match &kept {
        Some(k) => out.extend_from_slice(&k.maps),
        None => out.push(0),
    }
    let own = kept.as_ref().map_or(0, |k| k.objects.len() / 9);
    out.extend_from_slice(&((own + plants.len()) as u32).to_le_bytes());
    if let Some(k) = &kept {
        out.extend_from_slice(&k.objects);
    }
    let unit = side / 65536.0;
    for p in plants {
        out.push(slots.get(p.species as usize).copied().unwrap_or(0));
        for v in p.at {
            out.extend_from_slice(&((v / unit).round().clamp(0.0, 65535.0) as u16).to_le_bytes());
        }
        out.push(p.height);
        out.push(p.width);
    }
    out
}

/// A track's own `.esb`, kept as its bytes: each species' record, the
/// maps' names and the objects.
struct Kept {
    method: u32,
    species: Vec<Option<Vec<u8>>>,
    maps: Vec<u8>,
    objects: Vec<u8>,
}

impl Kept {
    fn read(b: &[u8]) -> Option<Kept> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Option<&[u8]> {
            let s = b.get(*at..*at + n)?;
            *at += n;
            Some(s)
        };
        let u32_at = |at: &mut usize| -> Option<u32> { Some(u32::from_le_bytes(take(at, 4)?.try_into().ok()?)) };
        let string = |at: &mut usize| -> Option<()> {
            let n = *take(at, 1)?.first()? as usize;
            take(at, n)?;
            Some(())
        };
        let method = u32_at(&mut at)?;
        let mut species = Vec::with_capacity(256);
        for _ in 0..256 {
            let from = at;
            if take(&mut at, 1)?[0] == 0 {
                species.push(None);
                continue;
            }
            string(&mut at)?;
            string(&mut at)?;
            take(&mut at, 13 * 4 + 4)?;
            if method == 2 {
                take(&mut at, 9 * 4)?;
                if take(&mut at, 1)?[0] != 0 {
                    string(&mut at)?;
                }
            }
            let shapes = u32_at(&mut at)? as usize;
            take(&mut at, shapes * 36)?;
            species.push(Some(b[from..at].to_vec()));
        }
        let from = at;
        if take(&mut at, 1)?[0] != 0 {
            string(&mut at)?;
        }
        if method == 2 {
            take(&mut at, 4)?;
            if take(&mut at, 1)?[0] != 0 {
                string(&mut at)?;
            }
        }
        let maps = b[from..at].to_vec();
        let count = u32_at(&mut at)? as usize;
        let objects = take(&mut at, count * 9)?.to_vec();
        Some(Kept { method, species, maps, objects })
    }
}

// ---- rivers ----

/// A river's middle line every few feet, with its water's height there.
#[derive(Debug, Clone, PartialEq)]
pub struct Water {
    pub width: f32,
    /// x, water height, z (ft).
    pub points: Vec<[f32; 3]>,
}

/// Feet between a river's samples.
const RIVER_STEP: f32 = 4.0;

/// A river's middle line, rounded through its points (Catmull-Rom).
pub fn river_line(r: &River) -> Vec<[f32; 2]> {
    let p = &r.points;
    if p.len() < 2 {
        return p.clone();
    }
    let mut out = Vec::new();
    for i in 0..p.len() - 1 {
        let a = p[i.saturating_sub(1)];
        let (b, c) = (p[i], p[i + 1]);
        let d = p[(i + 2).min(p.len() - 1)];
        let len = ((c[0] - b[0]).powi(2) + (c[1] - b[1]).powi(2)).sqrt();
        let steps = (len / RIVER_STEP).ceil().max(1.0) as usize;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            out.push([f(a[0], b[0], c[0], d[0]), f(a[1], b[1], c[1], d[1])]);
        }
    }
    out.push(*p.last().unwrap());
    out
}

fn sample(n: usize, step: f32, heights: &[f32], x: f32, z: f32) -> f32 {
    let i = |v: f32| ((v / step).round().max(0.0) as usize).min(n - 1);
    heights[i(z) * n + i(x)]
}

/// Cuts the rivers into the ground: a bed `depth` deep under water that
/// only ever runs downhill, banks that meet it, and mud in the bed. Marks
/// each sample under water with the water's height in `water`, and
/// returns each river's water.
pub fn carve(course: &Course, n: usize, step: f32, heights: &mut [f32], surfaces: &mut [u8], water: &mut [f32]) -> Vec<Water> {
    let mut out = Vec::new();
    for r in &course.rivers {
        let line = river_line(r);
        if line.len() < 2 {
            continue;
        }
        let hw = (r.width / 2.0).max(3.0);
        let depth = r.depth.clamp(0.5, 12.0);
        let bank = (r.width * 0.6).max(12.0);
        // The water: the lowest ground along the line so far, a little
        // under it.
        let mut level = f32::MAX;
        let mut points = Vec::with_capacity(line.len());
        for p in &line {
            // The lowest across the bed here.
            let mut low = sample(n, step, heights, p[0], p[1]);
            for (dx, dz) in [(hw * 0.5, 0.0), (-hw * 0.5, 0.0), (0.0, hw * 0.5), (0.0, -hw * 0.5)] {
                low = low.min(sample(n, step, heights, p[0] + dx, p[1] + dz));
            }
            level = level.min(low - 0.6);
            points.push([p[0], level, p[1]]);
        }
        // Each sample's nearest point of the line, within reach.
        let reach = hw + bank;
        let mut near = vec![(f32::MAX, 0.0f32); 0];
        let (lo, hi) = line.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let cell = |v: f32| ((v / step).floor().max(0.0) as usize).min(n - 1);
        let (x0, z0, x1, z1) = (cell(lo[0] - reach), cell(lo[1] - reach), cell(hi[0] + reach) + 1, cell(hi[1] + reach) + 1);
        let (x1, z1) = (x1.min(n - 1), z1.min(n - 1));
        let w = x1 - x0 + 1;
        near.resize(w * (z1 - z0 + 1), (f32::MAX, 0.0));
        for (k, q) in points.iter().enumerate() {
            let _ = k;
            let (qx, qz) = (q[0], q[2]);
            for z in cell(qz - reach)..=cell(qz + reach).min(z1) {
                if z < z0 {
                    continue;
                }
                for x in cell(qx - reach)..=cell(qx + reach).min(x1) {
                    if x < x0 {
                        continue;
                    }
                    let d = ((x as f32 * step - qx).powi(2) + (z as f32 * step - qz).powi(2)).sqrt();
                    let c = &mut near[(z - z0) * w + (x - x0)];
                    if d < c.0 {
                        *c = (d, q[1]);
                    }
                }
            }
        }
        for z in z0..=z1 {
            for x in x0..=x1 {
                let (d, level) = near[(z - z0) * w + (x - x0)];
                if d >= reach {
                    continue;
                }
                let k = z * n + x;
                let h = &mut heights[k];
                if d < hw {
                    let t = d / hw;
                    let bed = level - depth * (1.0 - t * t).max(0.0).powf(0.7);
                    *h = h.min(bed).min(level - 0.15);
                    surfaces[k] = Surface::Mud.zone();
                    water[k] = if water[k].is_nan() { level } else { water[k].max(level) };
                } else {
                    // The bank: pulled to just over the water at the edge,
                    // the land as it was further out.
                    let u = ((d - hw) / bank).clamp(0.0, 1.0);
                    let s = u * u * (3.0 - 2.0 * u);
                    let edge = level + 0.5;
                    *h = edge + (*h - edge) * s;
                    if u < 0.12 && water[k].is_nan() {
                        water[k] = level;
                    }
                }
            }
        }
        out.push(Water { width: r.width, points });
    }
    out
}

/// The rivers' water for the viewer: `OPENMCM2 WATER 1`, then per river
/// `RIVER <width> <points>` and a line `x y z` for each point.
pub fn water_file(rivers: &[Water]) -> String {
    let mut s = String::from("OPENMCM2 WATER 1\r\n");
    for r in rivers {
        // Every other sample is plenty for the water's surface.
        let pts: Vec<&[f32; 3]> = r.points.iter().step_by(2).chain(r.points.last()).collect();
        s += &format!("RIVER {:.1} {}\r\n", r.width, pts.len());
        for p in pts {
            s += &format!("{:.1} {:.2} {:.1}\r\n", p[0], p[1], p[2]);
        }
    }
    s
}

/// Reads [`water_file`]'s text.
pub fn read_water(text: &str) -> Vec<Water> {
    let mut out: Vec<Water> = Vec::new();
    let mut lines = text.lines();
    if !lines.next().is_some_and(|l| l.trim().starts_with("OPENMCM2 WATER")) {
        return out;
    }
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[..] {
            ["RIVER", w, _] => out.push(Water { width: w.parse().unwrap_or(20.0), points: Vec::new() }),
            [x, y, z] => {
                if let (Some(r), Ok(x), Ok(y), Ok(z)) = (out.last_mut(), x.parse(), y.parse(), z.parse()) {
                    r.points.push([x, y, z]);
                }
            }
            _ => {}
        }
    }
    out.retain(|r| r.points.len() >= 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Course, Planting};

    #[test]
    fn plants_grow_off_the_track_and_read_back() {
        let mut c = Course::new(Mode::National);
        c.plants.push(Planting { flora: Flora::Mixed, radius: 300.0, density: 0.8, clear: false, everywhere: true, dabs: Vec::new() });
        let land = crate::bake::land(&c, 4);
        let field = crate::bake::bake(&c, 4, &land);
        let plants = plants(&c, &field);
        assert!(plants.len() > 1000, "{} plants", plants.len());
        assert!(plants.len() <= MOST_PLANTS);
        let sp = species(&c);
        let bytes = esb(c.mode.side(), &sp, &plants, None);
        let eco = mm2_formats::esb::parse(&bytes).unwrap();
        assert_eq!(eco.objects.len(), plants.len());
        assert_eq!(eco.species.len(), sp.len());
        // Laid onto a track's own eco-system, both are kept.
        let again = esb(c.mode.side(), &sp[..2], &plants[..10], Some(&bytes));
        let eco2 = mm2_formats::esb::parse(&again).unwrap();
        assert_eq!(eco2.objects.len(), plants.len() + 10);
        assert_eq!(eco2.species.len(), sp.len() + 2);
    }

    #[test]
    fn a_river_runs_downhill_in_its_bed() {
        let mut c = Course::new(Mode::National);
        let s = c.mode.side();
        c.rivers.push(River { points: vec![[s * 0.1, s * 0.2], [s * 0.5, s * 0.25], [s * 0.9, s * 0.2]], width: 30.0, depth: 2.5 });
        let land = crate::bake::land(&c, 4);
        let field = crate::bake::bake(&c, 4, &land);
        let w = &field.rivers[0];
        assert!(w.points.windows(2).all(|p| p[1][1] <= p[0][1]));
        for p in w.points.iter().step_by(10) {
            assert!(field.height(p[0], p[2]) < p[1], "bed over the water at {p:?}");
        }
        let text = water_file(&field.rivers);
        assert_eq!(read_water(&text).len(), 1);
    }
}
