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

/// How the scenery grows on a land: grass along the edges, bush clumps,
/// groves of trees, and a light scatter of trees (and on a desert, bushes)
/// over the rest. Densities 0 to 1; groves and clumps a hundred feet of
/// line.
struct Growth {
    edge_grass: f32,
    clumps: f32,
    groves: f32,
    grove_density: f32,
    scatter_trees: f32,
    scatter_bushes: f32,
    /// Woods out on the land.
    woods: usize,
}

fn growth(course: &Course) -> Growth {
    match course.land.kind {
        LandKind::Desert => Growth { edge_grass: 0.25, clumps: 0.6, groves: 0.3, grove_density: 0.45, scatter_trees: 0.02, scatter_bushes: 0.02, woods: 0 },
        LandKind::Mountains => Growth { edge_grass: 0.45, clumps: 0.5, groves: 0.75, grove_density: 0.85, scatter_trees: 0.03, scatter_bushes: 0.0, woods: 8 },
        LandKind::Valley => Growth { edge_grass: 0.6, clumps: 0.7, groves: 0.6, grove_density: 0.8, scatter_trees: 0.02, scatter_bushes: 0.0, woods: 6 },
        LandKind::Rolling | LandKind::Flat => Growth { edge_grass: 0.55, clumps: 0.65, groves: 0.5, grove_density: 0.75, scatter_trees: 0.015, scatter_bushes: 0.0, woods: 5 },
    }
}

/// "Dress the scenery": plant strokes that grow the land round the line
/// as a real course looks, tall grass along the edges, bushes in clumps
/// just off the track, groves of trees further out and a few trees over
/// the rest. Every stroke is marked [`Planting::auto`]. Nothing for a
/// stadium. Kept light (about 15,000 plants on a National track) so the
/// track loads fast in the browser too.
pub fn scenery(course: &Course) -> Vec<crate::Planting> {
    use crate::Planting;
    if course.mode == Mode::Supercross {
        return Vec::new();
    }
    let path = course.path();
    if path.is_empty() || path.length < 100.0 {
        return Vec::new();
    }
    let g = growth(course);
    let side = course.mode.side();
    // Different for each track, the same every time for one.
    let mut seed = course.land.seed as u64 ^ 0x9e37_79b9;
    for p in &course.line.points {
        seed = seed.wrapping_mul(31).wrapping_add((p.x as i64 * 7 + p.z as i64 * 13) as u64);
    }
    let mut dice = Dice(seed);
    let stroke = |flora, radius, density, dabs| Planting { flora, radius, density, clear: false, everywhere: false, dabs, auto: true };
    let mut out = Vec::new();
    // Over the rest of the land.
    if g.scatter_trees > 0.0 {
        out.push(Planting { everywhere: true, ..stroke(Flora::Trees, 0.0, g.scatter_trees, Vec::new()) });
    }
    if g.scatter_bushes > 0.0 {
        out.push(Planting { everywhere: true, ..stroke(Flora::Bushes, 0.0, g.scatter_bushes, Vec::new()) });
    }
    let inside = |p: [f32; 2]| p[0] > 20.0 && p[1] > 20.0 && p[0] < side - 20.0 && p[1] < side - 20.0;
    let at = |i: usize, off: f32| {
        let (p, n) = (path.pts[i], path.side(i));
        [p[0] + n[0] * off, p[1] + n[1] * off]
    };
    // Tall grass along both edges, broken here and there.
    let mut grass = Vec::new();
    let mut s = 0.0;
    while s < path.length {
        let i = path.index_at(s);
        let hw = path.width[i] / 2.0;
        for sign in [1.0f32, -1.0] {
            if dice.next() < 0.8 {
                let p = at(i, sign * (hw + 16.0 + dice.next() * 22.0));
                if inside(p) {
                    grass.push(p);
                }
            }
        }
        s += 34.0;
    }
    out.push(stroke(Flora::Grass, 24.0, g.edge_grass, grass));
    // Bush clumps just off the track.
    let mut bushes = Vec::new();
    let mut s = 20.0;
    while s < path.length {
        if dice.next() < g.clumps {
            let i = path.index_at(s);
            let hw = path.width[i] / 2.0;
            let sign = if dice.next() < 0.5 { 1.0 } else { -1.0 };
            let p = at(i, sign * (hw + 26.0 + dice.next() * 50.0));
            if inside(p) {
                bushes.push(stroke(Flora::Bushes, 14.0 + dice.next() * 14.0, 0.7 + dice.next() * 0.3, vec![p]));
            }
        }
        s += 70.0 + dice.next() * 40.0;
    }
    out.extend(bushes);
    // Groves of trees with bushes round them, further out.
    let mut s = 60.0;
    while s < path.length {
        if dice.next() < g.groves {
            let i = path.index_at(s);
            let hw = path.width[i] / 2.0;
            let sign = if dice.next() < 0.5 { 1.0 } else { -1.0 };
            let r = 40.0 + dice.next() * 50.0;
            let p = at(i, sign * (hw + 40.0 + r * 0.5 + dice.next() * 170.0));
            if inside(p) {
                out.push(stroke(Flora::Trees, r, g.grove_density * (0.75 + dice.next() * 0.25), vec![p]));
                out.push(stroke(Flora::Bushes, r * 1.25, 0.45, vec![p]));
            }
        }
        s += 110.0 + dice.next() * 70.0;
    }
    // A few woods out on the land, away from the track, for a backdrop.
    let grid = crate::path::Grid::new(&path, 400.0);
    let mut woods = 0;
    for _ in 0..80 {
        if woods >= g.woods {
            break;
        }
        let p = [side * (0.06 + dice.next() * 0.88), side * (0.06 + dice.next() * 0.88)];
        let r = 120.0 + dice.next() * 160.0;
        if grid.nearest(p).is_some_and(|(_, d)| d < r + 120.0) {
            continue;
        }
        out.push(stroke(Flora::Trees, r, g.grove_density, vec![p]));
        out.push(stroke(Flora::Bushes, r * 1.15, 0.35, vec![p]));
        woods += 1;
    }
    out
}

/// Grows the scenery again round the line as it is now: the old
/// "Dress the scenery" strokes give way to new ones, laid under the
/// player's own so their painting and clearing still show.
pub fn redress(course: &mut Course) {
    course.plants.retain(|p| !p.auto);
    let mut fresh = scenery(course);
    fresh.append(&mut course.plants);
    course.plants = fresh;
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
        c.plants.push(Planting { flora: Flora::Mixed, radius: 300.0, density: 0.8, clear: false, everywhere: true, dabs: Vec::new(), auto: false });
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
    fn dressed_tracks_stay_light() {
        for mode in [Mode::National, Mode::Enduro, Mode::Baja, Mode::Supercross] {
            let mut c = Course::new(mode);
            redress(&mut c);
            let land = crate::bake::land(&c, 2);
            let field = crate::bake::bake(&c, 2, &land);
            let plants = plants(&c, &field);
            let sp = species(&c);
            let trees = plants.iter().filter(|p| sp[p.species as usize].flora == Flora::Trees).count();
            let bushes = plants.iter().filter(|p| sp[p.species as usize].flora == Flora::Bushes).count();
            println!("{mode:?}: {} plants ({trees} trees, {bushes} bushes), esb {} KB", plants.len(), esb(c.mode.side(), &sp, &plants, None).len() / 1024);
            if mode == Mode::Supercross {
                assert!(plants.is_empty());
                continue;
            }
            assert!(trees > 300 && bushes > 300, "{mode:?} bare");
            assert!(plants.len() < 30_000, "{mode:?}: {} plants", plants.len());
        }
    }

    #[test]
    fn dressing_again_follows_the_line_and_keeps_own_plants() {
        let mut c = Course::new(Mode::National);
        let own = Planting { flora: Flora::Trees, radius: 50.0, density: 1.0, clear: true, everywhere: false, dabs: vec![[100.0, 100.0]], auto: false };
        c.plants.push(own.clone());
        redress(&mut c);
        let first = c.plants.clone();
        for p in &mut c.line.points {
            p.x += 200.0;
        }
        redress(&mut c);
        assert_ne!(c.plants, first);
        assert_eq!(c.plants.last(), Some(&own));
        assert_eq!(c.plants.iter().filter(|p| !p.auto).count(), 1);
        assert!(c.change_mode(Mode::Supercross));
        assert!(c.plants.iter().all(|p| !p.auto));
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
