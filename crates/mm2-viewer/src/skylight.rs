//! The sky's light over the land, baked once when a track loads: for each
//! of the terrain's samples, how much of the sky the hills round it leave
//! open (enhanced_fns.wgsl `sky_seen`). Open ground sees all of it; the
//! floor of a dip, the foot of a bank or a gully between hills less, so
//! the ambient light there is a little dimmer, as under a real sky.
//!
//! Each sample looks along eight ways over the heights, out to
//! [`REACH`] ft, for the highest the land rises; a level surface under a
//! horizon raised by an angle `h` sees cos²(h) of the sky's light that way,
//! the angle taken above the ground's own slope there.

use mm2_formats::trn::Terrain;

/// The ways each sample looks.
const WAYS: usize = 8;
/// How far it looks, ft, in steps growing by [`GROWTH`] from [`FIRST`].
const REACH: f32 = 400.0;
const FIRST: f32 = 2.0;
const GROWTH: f32 = 1.35;

/// The baked map: per sample, how much sky it sees (r) and its height (g,
/// from `low` across `range` ft), and what places it over the drawing.
pub struct SkyLight {
    pub texels: Vec<[u8; 4]>,
    /// Samples a side, and ft between them.
    pub side: u32,
    pub spacing: f32,
    pub low: f32,
    pub range: f32,
}

impl SkyLight {
    pub fn bake(terrain: &Terrain) -> SkyLight {
        let n = terrain.size;
        let s = terrain.scale;
        let height = |c: i64, r: i64| -> f32 {
            let c = c.clamp(0, n as i64 - 1) as usize;
            let r = r.clamp(0, n as i64 - 1) as usize;
            terrain.heights[r * n + c] * s
        };
        let mut steps = Vec::new();
        let mut d = FIRST;
        while d < REACH {
            steps.push(d);
            d *= GROWTH;
        }
        let ways: Vec<(f32, f32)> = (0..WAYS)
            .map(|i| {
                let a = i as f32 / WAYS as f32 * std::f32::consts::TAU;
                (a.cos(), a.sin())
            })
            .collect();
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for h in &terrain.heights {
            low = low.min(h * s);
            high = high.max(h * s);
        }
        let range = (high - low).max(1.0);
        let mut texels = vec![[255u8, 0, 0, 255]; n * n];
        // Each way's step across the samples, as `wx * d / s` was worked
        // out for every sample (the same bits, so the same map).
        let offsets: Vec<Vec<(f32, f32, f32)>> = ways
            .iter()
            .map(|&(wx, wz)| steps.iter().map(|&d| (wx * d / s, wz * d / s, d)).collect())
            .collect();
        let last = (n - 1) as f32;
        // The highest ground within reach of each sample (ft), so a way's
        // search can stop once nothing further could rise above it.
        let reach = (REACH / s).ceil() as usize + 1;
        let highest = window_max(&terrain.heights, n, reach);
        let row = |r: usize, out: &mut [[u8; 4]]| {
            for (c, texel) in out.iter_mut().enumerate() {
                let here = height(c as i64, r as i64);
                let high = highest[r * n + c] * s;
                // The ground's own slope: land rising only as the slope
                // does hides none of the sky a surface on it faces.
                let (ci, ri) = (c as i64, r as i64);
                let gx = (height(ci + 1, ri) - height(ci - 1, ri)) / (2.0 * s);
                let gz = (height(ci, ri + 1) - height(ci, ri - 1)) / (2.0 * s);
                let mut seen = 0.0;
                for (&(wx, wz), offsets) in ways.iter().zip(&offsets) {
                    let slope = gx * wx + gz * wz;
                    let mut rise: f32 = 0.0;
                    for &(ox, oz, d) in offsets {
                        // Not even the highest ground within reach, this far
                        // off or further, would rise above `rise`: done.
                        // (Each step only rounds down what bounds it, so
                        // this leaves out nothing that would count.)
                        if (high - here) / d - slope <= rise {
                            break;
                        }
                        let (x, z) = (c as f32 + ox, r as f32 + oz);
                        // Past the edge the land is taken as level.
                        if x < 0.0 || z < 0.0 || x > last || z > last {
                            break;
                        }
                        // Inside, the nearest sample, as `round` gives it
                        // (half away from zero) without its call.
                        let near = |v: f32| {
                            let i = v as usize;
                            i + (v - i as f32 >= 0.5) as usize
                        };
                        let h = terrain.heights[near(z) * n + near(x)] * s;
                        rise = rise.max((h - here) / d - slope);
                    }
                    seen += 1.0 / (1.0 + rise * rise);
                }
                let seen = seen / WAYS as f32;
                *texel = [
                    (seen * 255.0).round() as u8,
                    (((here - low) / range) * 255.0).round() as u8,
                    0,
                    255,
                ];
            }
        };
        // A row at a time, the rows shared among the processor's cores
        // (a browser has the one).
        #[cfg(not(target_arch = "wasm32"))]
        {
            let threads = std::thread::available_parallelism().map_or(1, |t| t.get()).min(16);
            let rows_each = n.div_ceil(threads).max(1);
            std::thread::scope(|scope| {
                for (k, chunk) in texels.chunks_mut(rows_each * n).enumerate() {
                    let row = &row;
                    scope.spawn(move || {
                        for (i, out) in chunk.chunks_mut(n).enumerate() {
                            row(k * rows_each + i, out);
                        }
                    });
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        for (r, out) in texels.chunks_mut(n).enumerate() {
            row(r, out);
        }
        SkyLight {
            texels,
            side: n as u32,
            spacing: s,
            low,
            range,
        }
    }
}

/// The highest of `heights` (`n` a side) within `radius` samples of each,
/// across and along (a square): by rows then columns, each a run of
/// blocks with their highest so far from either end (van Herk, Gil and
/// Werman), so it takes the same time whatever the radius.
fn window_max(heights: &[f32], n: usize, radius: usize) -> Vec<f32> {
    // Each line padded `radius` past both ends with nothing, so every
    // window is a whole `w` long and spans at most two blocks.
    let w = 2 * radius + 1;
    let len = n + 2 * radius;
    let line = |get: &dyn Fn(usize) -> f32, out: &mut dyn FnMut(usize, f32)| {
        let at = |i: usize| if i < radius || i >= radius + n { f32::MIN } else { get(i - radius) };
        let (mut ahead, mut behind) = (vec![f32::MIN; len], vec![f32::MIN; len]);
        for i in 0..len {
            ahead[i] = if i % w == 0 { at(i) } else { ahead[i - 1].max(at(i)) };
        }
        for i in (0..len).rev() {
            behind[i] = if i % w == w - 1 || i == len - 1 { at(i) } else { behind[i + 1].max(at(i)) };
        }
        // Sample i's window is i..i + w in the padded line.
        for i in 0..n {
            out(i, behind[i].max(ahead[i + w - 1]));
        }
    };
    let mut rows = vec![0.0; n * n];
    for r in 0..n {
        line(&|i| heights[r * n + i], &mut |i, v| rows[r * n + i] = v);
    }
    let mut out = vec![0.0; n * n];
    for c in 0..n {
        line(&|i| rows[i * n + c], &mut |i, v| out[i * n + c] = v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain(size: usize, height: impl Fn(usize, usize) -> f32) -> Terrain {
        Terrain {
            size,
            heights: (0..size * size)
                .map(|i| height(i % size, i / size))
                .collect(),
            normals: vec![[0, 127, 0]; size * size],
            surfaces: vec![0; size * size],
            scale: 4.0,
            textures: Vec::new(),
            tiles: Vec::new(),
        }
    }

    /// The map as first written, one sample at a time with `round`: the
    /// faster bake must give the very same bytes.
    fn bake_plain(terrain: &Terrain) -> Vec<[u8; 4]> {
        let n = terrain.size;
        let s = terrain.scale;
        let height = |c: i64, r: i64| terrain.heights[r.clamp(0, n as i64 - 1) as usize * n + c.clamp(0, n as i64 - 1) as usize] * s;
        let mut steps = Vec::new();
        let mut d = FIRST;
        while d < REACH {
            steps.push(d);
            d *= GROWTH;
        }
        let ways: Vec<(f32, f32)> = (0..WAYS).map(|i| i as f32 / WAYS as f32 * std::f32::consts::TAU).map(|a| (a.cos(), a.sin())).collect();
        let low = terrain.heights.iter().fold(f32::MAX, |a, h| a.min(h * s));
        let high = terrain.heights.iter().fold(f32::MIN, |a, h| a.max(h * s));
        let range = (high - low).max(1.0);
        let mut out = Vec::new();
        for r in 0..n {
            for c in 0..n {
                let here = height(c as i64, r as i64);
                let (ci, ri) = (c as i64, r as i64);
                let gx = (height(ci + 1, ri) - height(ci - 1, ri)) / (2.0 * s);
                let gz = (height(ci, ri + 1) - height(ci, ri - 1)) / (2.0 * s);
                let mut seen = 0.0;
                for &(wx, wz) in &ways {
                    let slope = gx * wx + gz * wz;
                    let mut rise: f32 = 0.0;
                    for &d in &steps {
                        let (x, z) = (c as f32 + wx * d / s, r as f32 + wz * d / s);
                        if x < 0.0 || z < 0.0 || x > (n - 1) as f32 || z > (n - 1) as f32 {
                            break;
                        }
                        rise = rise.max((height(x.round() as i64, z.round() as i64) - here) / d - slope);
                    }
                    seen += 1.0 / (1.0 + rise * rise);
                }
                let seen = seen / WAYS as f32;
                out.push([(seen * 255.0).round() as u8, (((here - low) / range) * 255.0).round() as u8, 0, 255]);
            }
        }
        out
    }

    #[test]
    fn window_max_is_the_highest_round_about() {
        let n = 23;
        let h: Vec<f32> = (0..n * n).map(|i| ((i * 7919) % 101) as f32).collect();
        for radius in [0, 1, 3, 7, 30] {
            let m = window_max(&h, n, radius);
            for r in 0..n {
                for c in 0..n {
                    let mut best = f32::MIN;
                    for y in r.saturating_sub(radius)..=(r + radius).min(n - 1) {
                        for x in c.saturating_sub(radius)..=(c + radius).min(n - 1) {
                            best = best.max(h[y * n + x]);
                        }
                    }
                    assert_eq!(m[r * n + c], best, "radius {radius} at {c},{r}");
                }
            }
        }
    }

    #[test]
    fn faster_bake_is_the_same() {
        for scale in [4.0, 7.5, 13.0] {
            let mut t = terrain(97, |x, z| ((x as f32 * 0.37).sin() * 9.0 + (z as f32 * 0.21).cos() * 14.0 + ((x * 31 + z * 17) % 7) as f32 * 0.6));
            t.scale = scale;
            assert!(SkyLight::bake(&t).texels == bake_plain(&t), "scale {scale}");
        }
    }

    /// On real tracks from the game's files, when they are there
    /// (`cargo test --release -- --ignored real_tracks`): the same bytes,
    /// and how long each bake takes.
    #[test]
    #[ignore]
    fn real_tracks_bake_the_same() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GAME/TERAFORM");
        for track in ["NATIONAL/NAT05.ENV", "NATIONAL/NAT01.ENV", "BAJA/BAJA01.ENV", "SX/SX01.ENV"] {
            let Ok(bytes) = std::fs::read(root.join(track)) else { continue };
            let name = track.rsplit('/').next().unwrap();
            let bytes = if bytes.get(..4) == Some(mm2_formats::faoe::MAGIC) { mm2_formats::faoe::decode(&bytes, name).unwrap() } else { bytes };
            let archive = mm2_formats::rs2::Archive::parse(&bytes).unwrap();
            let e = archive.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".trn")).unwrap();
            let t = mm2_formats::trn::parse(archive.data(e)).unwrap();
            let start = std::time::Instant::now();
            let fast = SkyLight::bake(&t).texels;
            let fast_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = std::time::Instant::now();
            let plain = bake_plain(&t);
            let plain_ms = start.elapsed().as_secs_f64() * 1000.0;
            eprintln!("{track}: {} samples a side, {plain_ms:.0} ms before, {fast_ms:.0} ms now", t.size);
            assert!(fast == plain, "{track}");
        }
    }

    #[test]
    fn open_ground_sees_the_whole_sky_and_a_ditch_less() {
        let flat = SkyLight::bake(&terrain(32, |_, _| 0.0));
        assert!(flat.texels.iter().all(|t| t[0] == 255));
        // A trench along the middle, 5 units (20 ft) deep, 2 wide.
        let ditch = SkyLight::bake(&terrain(
            32,
            |c, _| if (15..17).contains(&c) { -5.0 } else { 0.0 },
        ));
        let floor = ditch.texels[16 * 32 + 15][0];
        let open = ditch.texels[16 * 32 + 4][0];
        assert!(floor < 200, "{floor}");
        assert!(open > 250, "{open}");
    }
}
